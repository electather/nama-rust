use std::sync::{Arc, Mutex, PoisonError};

use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    contract::{
        Artwork as ListedArtwork, ArtworkAnswer, ArtworkAsked, ArtworkKind, Candidate, Movie,
        MovieAnswer, SearchQuery, SearchResults, Season, SeasonAnswer, SeasonAsked, Show,
        ShowAnswer, TitleAsked, TitleId,
    },
    metadata::{ExternalId, ExternalIds, MetadataError, TitleMethod, TitleSource},
};
use tokio_util::sync::CancellationToken;

use super::season;
use crate::{
    db::Database,
    events::EventBus,
    images::ImageStore,
    library::{
        Artwork, Change, Contents, Item, Libraries, Metadata, Pin, ScanLocks, TitleSources,
        UnmatchedReason, fixture,
    },
    matching::{FoundShow, Matcher, Miss, Summary, TitleKey, recognisers::Recognisers},
    plugins::{ExtensionPoints, Registry},
    server_settings::ServerUrl,
};

/// The Movie `key` names, with no Recogniser enabled.
async fn movie(metadata: &TitleSources, key: &TitleKey) -> Result<Metadata, Miss> {
    super::movie(metadata, &Recognisers::default(), key, &[]).await
}

/// The Show `key` names, with no Recogniser enabled.
async fn show(metadata: &TitleSources, key: &TitleKey) -> Result<FoundShow, Miss> {
    super::show(metadata, &Recognisers::default(), key, &[], &[]).await
}

/// How a [`Fake`] title source answers a search, or a lookup by id.
#[derive(Clone, Copy)]
enum Answers {
    Has,
    NotFound,
    Fails,
}

/// What the sources were asked, as `namespace method id`, in the order asked; a source asked its
/// namespace logs `namespace namespace`, which only [`namespaces_asked`] reads.
type Asked = Arc<Mutex<Vec<String>>>;

/// A title source keeping titles under `namespace`, as the Plugin of that id, and looking them up
/// by `looks_up`. Whatever it has is the Show `Dark`, its id `<namespace>-1`.
struct Fake {
    namespace: &'static str,
    looks_up: &'static [&'static str],
    search: Answers,
    lookup: Answers,
    asked: Asked,
}

impl Fake {
    fn ids(&self) -> Vec<TitleId> {
        vec![TitleId {
            namespace: self.namespace.to_owned(),
            value: "1".to_owned(),
            ..Default::default()
        }]
    }

    fn answer<T: Default>(
        &self,
        asked: &str,
        answers: Answers,
        found: T,
    ) -> Result<T, MetadataError> {
        let mut log = self.asked.lock().unwrap_or_else(PoisonError::into_inner);
        log.push(format!("{} {asked}", self.namespace));
        match answers {
            Answers::Has => Ok(found),
            Answers::NotFound => Ok(T::default()),
            Answers::Fails => Err(MetadataError::Failed("unreachable".into())),
        }
    }
}

/// `id` as `namespace-value`, as an [`ExternalId`] shows.
fn named(id: &TitleId) -> String {
    format!("{}-{}", id.namespace, id.value)
}

impl TitleSource for Fake {
    fn namespace(&self) -> &str {
        let mut log = self.asked.lock().unwrap_or_else(PoisonError::into_inner);
        log.push(format!("{} namespace", self.namespace));
        self.namespace
    }

    fn lookup_namespaces(&self) -> Vec<String> {
        self.looks_up.iter().map(|&n| n.to_owned()).collect()
    }

    fn search<'a>(
        &'a self,
        _query: &'a SearchQuery,
    ) -> BoxFuture<'a, Result<SearchResults, MetadataError>> {
        let candidate = Candidate {
            id: self.ids().remove(0).into(),
            ids: self.ids(),
            title: "Dark".to_owned(),
            vote_count: 100,
            ..Default::default()
        };
        let found = SearchResults {
            candidates: vec![candidate],
            ..Default::default()
        };
        let found = self.answer("search", self.search, found);
        Box::pin(async { found })
    }

    fn movie<'a>(
        &'a self,
        asked: &'a TitleAsked,
    ) -> BoxFuture<'a, Result<MovieAnswer, MetadataError>> {
        let movie = MovieAnswer {
            movie: Movie {
                ids: self.ids(),
                title: "Dark".to_owned(),
                ..Default::default()
            }
            .into(),
            ..Default::default()
        };
        let found = self.answer(&format!("movie {}", named(&asked.id)), self.lookup, movie);
        Box::pin(async { found })
    }

    fn show<'a>(
        &'a self,
        asked: &'a TitleAsked,
    ) -> BoxFuture<'a, Result<ShowAnswer, MetadataError>> {
        let show = ShowAnswer {
            show: Show {
                ids: self.ids(),
                title: "Dark".to_owned(),
                seasons: vec![1],
                ..Default::default()
            }
            .into(),
            ..Default::default()
        };
        let found = self.answer(&format!("show {}", named(&asked.id)), self.lookup, show);
        Box::pin(async { found })
    }

    fn season<'a>(
        &'a self,
        asked: &'a SeasonAsked,
    ) -> BoxFuture<'a, Result<SeasonAnswer, MetadataError>> {
        let season = SeasonAnswer {
            season: Season {
                number: asked.season,
                ..Default::default()
            }
            .into(),
            ..Default::default()
        };
        let found = self.answer(
            &format!("season {}", named(&asked.show)),
            self.lookup,
            season,
        );
        Box::pin(async { found })
    }
}

struct FakePlugin(
    &'static str,
    Answers,
    Answers,
    Asked,
    &'static [&'static str],
);

impl Plugin for FakePlugin {
    fn manifest(&self) -> Manifest {
        Manifest {
            version: "0.0.0".to_owned(),
            extension_points: vec![PointKey::provide::<dyn TitleSource>()],
            ..Manifest::new(PluginId::from_static(self.0))
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        let source = Fake {
            namespace: self.0,
            looks_up: self.4,
            search: self.1,
            lookup: self.2,
            asked: Arc::clone(&self.3),
        };
        registrar.provide::<dyn TitleSource>(Priority::DEFAULT, Arc::new(source));
        Ok(())
    }
}

/// How one fake source answers: its searches, then its lookups by id.
type Answering = (Answers, Answers);

/// Title sources under `aa` and `bb`, `bb` also looking titles up by `aa` ids and `aa` by those
/// in `aa_looks_up`, answering as given.
fn looking_up(
    aa: Answering,
    bb: Answering,
    aa_looks_up: &'static [&'static str],
) -> (Registry, Asked) {
    let plugins = Registry::new();
    let asked = Asked::default();
    let fakes = [("aa", aa, aa_looks_up), ("bb", bb, &["aa"][..])];
    for (namespace, (search, lookup), looks_up) in fakes {
        let plugin = FakePlugin(namespace, search, lookup, Arc::clone(&asked), looks_up);
        plugins.enable(Arc::new(plugin)).unwrap();
    }
    (plugins, asked)
}

/// Title sources under `aa` and `bb`, `bb` also looking titles up by `aa` ids, answering as given.
fn aa_and_bb(aa: Answering, bb: Answering) -> (Registry, Asked) {
    looking_up(aa, bb, &[])
}

fn bb_aa() -> Vec<String> {
    vec!["bb".to_owned(), "aa".to_owned()]
}

/// Title sources under `aa` and `bb`, asked in the order `[bb, aa]`, answering as given.
fn bb_then_aa(aa: Answering, bb: Answering) -> (TitleSources, Asked) {
    let (plugins, asked) = aa_and_bb(aa, bb);
    let metadata = TitleSources::new(plugins.extension_points());
    (metadata.in_order(bb_aa()), asked)
}

const HAS: Answering = (Answers::Has, Answers::Has);

fn asked(asked: &Asked) -> Vec<String> {
    let log = asked.lock().unwrap_or_else(PoisonError::into_inner);
    (log.iter())
        .filter(|line| !line.ends_with(" namespace"))
        .cloned()
        .collect()
}

/// The sources asked their namespace, once per ask, in the order asked.
fn namespaces_asked(asked: &Asked) -> Vec<String> {
    let log = asked.lock().unwrap_or_else(PoisonError::into_inner);
    (log.iter())
        .filter_map(|line| line.strip_suffix(" namespace"))
        .map(str::to_owned)
        .collect()
}

/// The Show `Dark`, its folder naming `hint`.
fn dark(hint: Option<ExternalId>) -> TitleKey {
    let item = Item::Episode {
        show: "Dark".to_owned(),
        year: None,
        hint,
        season: 1,
        episode: 1,
    };
    TitleKey {
        name: item.name(),
        pin: None,
    }
}

fn id(namespace: &str) -> ExternalId {
    ExternalId::new(namespace, "1").unwrap()
}

#[tokio::test]
async fn a_hint_its_source_misses_matches_through_the_next_and_keeps_both_ids() {
    let (metadata, sources_asked) = bb_then_aa((Answers::Has, Answers::NotFound), HAS);
    let hint = ExternalId::new("aa", "7").unwrap();

    let found = show(&metadata, &dark(Some(hint.clone()))).await.unwrap();

    assert_eq!(found.source.namespace, "bb");
    assert_eq!(found.show.ids, ExternalIds::from_iter([hint, id("bb")]));
    assert_eq!(asked(&sources_asked), ["aa show aa-7", "bb show aa-7"]);
}

#[tokio::test]
async fn a_search_bb_finds_nothing_for_falls_to_aa() {
    let (metadata, sources_asked) = bb_then_aa(HAS, (Answers::NotFound, Answers::Has));

    let found = show(&metadata, &dark(None)).await.unwrap();

    assert_eq!(found.source.namespace, "aa");
    assert_eq!(found.id, id("aa"));
    assert_eq!(
        asked(&sources_asked),
        ["bb search", "aa search", "aa show aa-1"]
    );
}

#[tokio::test]
async fn a_failing_bb_leaves_the_title_undecided() {
    let (metadata, sources_asked) = bb_then_aa(HAS, (Answers::Fails, Answers::Has));

    assert_eq!(
        show(&metadata, &dark(None)).await.unwrap_err(),
        Miss::Undecided
    );
    assert_eq!(asked(&sources_asked), ["bb search"]);
}

#[tokio::test]
async fn a_hint_no_source_looks_up_by_is_searched_for_instead() {
    let (metadata, sources_asked) = bb_then_aa(HAS, HAS);
    let tag = ExternalId::new("x264", "GROUP").unwrap();

    let found = show(&metadata, &dark(Some(tag))).await.unwrap();

    assert_eq!(found.source.namespace, "bb");
    assert_eq!(asked(&sources_asked), ["bb search", "bb show bb-1"]);
}

#[tokio::test]
async fn a_hint_every_source_misses_is_blamed() {
    let missing = (Answers::Has, Answers::NotFound);
    let (metadata, _) = bb_then_aa(missing, missing);

    assert_eq!(
        show(&metadata, &dark(Some(id("aa")))).await.unwrap_err(),
        Miss::Unmatched(Some(UnmatchedReason::UnknownIdHint))
    );
}

#[tokio::test]
async fn a_pin_whose_source_is_not_asked_leaves_the_title_undecided() {
    let (metadata, sources_asked) = bb_then_aa(HAS, HAS);
    let pinned = TitleKey {
        pin: Some(Pin::Title(id("cc"))),
        ..dark(None)
    };

    assert_eq!(show(&metadata, &pinned).await.unwrap_err(), Miss::Unserved);
    assert_eq!(asked(&sources_asked), Vec::<String>::new());
}

#[tokio::test]
async fn seasons_come_from_the_source_that_gave_the_show_by_its_id_there() {
    let (metadata, sources_asked) = bb_then_aa((Answers::Has, Answers::NotFound), HAS);
    let hint = ExternalId::new("aa", "7").unwrap();
    let found = show(&metadata, &dark(Some(hint))).await.unwrap();

    season(&metadata, &found, 1).await.unwrap();

    assert_eq!(asked(&sources_asked).last().unwrap(), "bb season bb-1");
}

/// What matching a Movies Library with sources `[bb, aa]`, `aa` looking titles up by
/// `aa_looks_up`, does with the Movies `Aaa`, pinned to `pin`, and `Dark`.
async fn match_with_pin(pin: ExternalId, aa_looks_up: &'static [&'static str]) -> Summary {
    let data = tempfile::tempdir().unwrap();
    let media = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let library = fixture::movies_library(&db, media.path()).await;
    Libraries::new(Arc::clone(&db))
        .set_title_sources(library, bb_aa())
        .await
        .unwrap();
    let contents = Contents::new(Arc::clone(&db));
    // "Aaa" sorts before "Dark", so its pin is asked about first.
    let files = ["Aaa", "Dark"].map(|title| Change::Store {
        path: format!("/media/{title}.mkv"),
        size: 10,
        modified_ns: 0,
        items: vec![Item::Movie {
            title: title.to_owned(),
            year: None,
            hint: None,
        }],
    });
    contents.apply(library, files.into()).await.unwrap();
    let stored = contents.files(library).await.unwrap();
    let aaa = stored
        .iter()
        .find(|file| file.path == "/media/Aaa.mkv")
        .unwrap();
    let named = contents.named_like(aaa.items[0].id).await.unwrap().unwrap();
    let pin = Pin::Title(pin);
    contents
        .pin(library, named.name, pin, vec![])
        .await
        .unwrap();
    let (plugins, _) = looking_up(HAS, HAS, aa_looks_up);
    let images = ImageStore::new(data.path(), Arc::clone(&db));
    let locks = ScanLocks::for_tests();
    let matcher = Matcher::new(
        Arc::clone(&db),
        plugins.extension_points(),
        Artwork::new(images, plugins.extension_points()),
        EventBus::new(db, ExtensionPoints::default(), ServerUrl::default()),
        locks.clone(),
    );

    let scan = locks.lock(library).await;
    matcher
        .match_library(&scan, &CancellationToken::new())
        .await
        .unwrap()
}

/// Only `Dark` Matched; the pinned `Aaa` is neither Matched nor Unmatched.
const PIN_UNDECIDED: Summary = Summary {
    matched: 1,
    unmatched: 0,
    undecided: 1,
};

#[tokio::test]
async fn a_pin_no_source_looks_up_leaves_the_rest_of_the_library_matching() {
    assert_eq!(match_with_pin(id("cc"), &[]).await, PIN_UNDECIDED);
}

#[tokio::test]
async fn an_imdb_pin_no_source_keeps_or_looks_up_stays_undecided() {
    let pin = ExternalId::new("imdb", "tt0078748").unwrap();

    assert_eq!(match_with_pin(pin, &[]).await, PIN_UNDECIDED);
}

#[tokio::test]
async fn an_imdb_pin_matches_through_a_source_looking_up_imdb() {
    let pin = ExternalId::new("imdb", "tt0078748").unwrap();
    let both = Summary {
        matched: 2,
        unmatched: 0,
        undecided: 0,
    };

    assert_eq!(match_with_pin(pin, &["imdb"]).await, both);
}

#[tokio::test]
async fn an_imdb_hint_no_source_looks_up_by_is_searched_for_instead() {
    let (metadata, sources_asked) = bb_then_aa(HAS, HAS);
    let hint = ExternalId::new("imdb", "tt0078748").unwrap();

    let found = show(&metadata, &dark(Some(hint))).await.unwrap();

    assert_eq!(found.source.namespace, "bb");
    assert_eq!(asked(&sources_asked), ["bb search", "bb show bb-1"]);
}

#[tokio::test]
async fn a_source_neither_keeping_nor_looking_up_a_hint_is_never_asked() {
    let (plugins, sources_asked) = looking_up(HAS, (Answers::Has, Answers::NotFound), &[]);
    let metadata = TitleSources::new(plugins.extension_points()).in_order(bb_aa());

    // `bb` looks up `aa` ids but `aa` doesn't look up `bb` ones: a `bb` hint `bb` misses is blamed.
    assert_eq!(
        show(&metadata, &dark(Some(id("bb")))).await.unwrap_err(),
        Miss::Unmatched(Some(UnmatchedReason::UnknownIdHint))
    );
    assert_eq!(asked(&sources_asked), ["bb show bb-1"]);
}

/// A title source like fanart.tv's: it keeps titles under `fanart`, looks them up by `tmdb` and
/// answers only the artwork list, logging any search or Movie asked of it.
struct ArtworkOnly(Asked);

impl TitleSource for ArtworkOnly {
    fn namespace(&self) -> &'static str {
        "fanart"
    }

    fn lookup_namespaces(&self) -> Vec<String> {
        vec!["tmdb".to_owned()]
    }

    fn search<'a>(
        &'a self,
        _query: &'a SearchQuery,
    ) -> BoxFuture<'a, Result<SearchResults, MetadataError>> {
        let mut log = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        log.push("fanart search".to_owned());
        Box::pin(async { Err(MetadataError::Unsupported) })
    }

    fn movie<'a>(
        &'a self,
        _asked: &'a TitleAsked,
    ) -> BoxFuture<'a, Result<MovieAnswer, MetadataError>> {
        let mut log = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        log.push("fanart movie".to_owned());
        Box::pin(async { Err(MetadataError::Unsupported) })
    }

    fn answers(&self, method: TitleMethod) -> bool {
        method == TitleMethod::Artwork
    }
}

struct ArtworkOnlyPlugin(Asked);

impl Plugin for ArtworkOnlyPlugin {
    fn manifest(&self) -> Manifest {
        Manifest {
            version: "0.0.0".to_owned(),
            extension_points: vec![PointKey::provide::<dyn TitleSource>()],
            ..Manifest::new(PluginId::from_static("fanart"))
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        let source = ArtworkOnly(Arc::clone(&self.0));
        registrar.provide::<dyn TitleSource>(Priority::DEFAULT, Arc::new(source));
        Ok(())
    }
}

/// The fanart-like [`ArtworkOnly`] source, with a `tmdb` source that has everything if `tmdb`.
fn fanart(tmdb: bool) -> (TitleSources, Asked) {
    let plugins = Registry::new();
    let asked = Asked::default();
    plugins
        .enable(Arc::new(ArtworkOnlyPlugin(Arc::clone(&asked))))
        .unwrap();
    if tmdb {
        let plugin = FakePlugin("tmdb", Answers::Has, Answers::Has, Arc::clone(&asked), &[]);
        plugins.enable(Arc::new(plugin)).unwrap();
    }
    (TitleSources::new(plugins.extension_points()), asked)
}

/// The Movie `Dark`, pinned to `pin`.
fn dark_movie(pin: Option<ExternalId>) -> TitleKey {
    let item = Item::Movie {
        title: "Dark".to_owned(),
        year: None,
        hint: None,
    };
    TitleKey {
        name: item.name(),
        pin: pin.map(Pin::Title),
    }
}

#[tokio::test]
async fn a_tmdb_pin_only_an_artwork_source_looks_up_stays_unserved() {
    let (metadata, sources_asked) = fanart(false);

    let pinned = movie(&metadata, &dark_movie(Some(ExternalId::tmdb(603))))
        .await
        .unwrap_err();

    assert_eq!(pinned, Miss::Unserved);
    assert_eq!(asked(&sources_asked), Vec::<String>::new());
}

#[tokio::test]
async fn a_tmdb_pin_goes_to_tmdb_beside_an_artwork_source() {
    let (metadata, sources_asked) = fanart(true);

    let found = movie(&metadata, &dark_movie(Some(ExternalId::tmdb(1))))
        .await
        .unwrap();

    assert_eq!(found.plugin, PluginId::from_static("tmdb"));
    assert_eq!(asked(&sources_asked), ["tmdb movie tmdb-1"]);
}

#[tokio::test]
async fn a_search_never_asks_an_artwork_source() {
    let (metadata, sources_asked) = fanart(true);

    let found = movie(&metadata, &dark_movie(None)).await.unwrap();

    assert_eq!(found.plugin, PluginId::from_static("tmdb"));
    assert_eq!(asked(&sources_asked), ["tmdb search", "tmdb movie tmdb-1"]);
}

/// A title source keeping titles under `.0` that lists `.2` as a title's artwork, each a kind, a
/// path and a language. It searches and has the Movie `Dark`, with the poster `.1`, unless `.1`
/// is `None`, when it only answers the artwork list and looks titles up by `aa`.
#[derive(Clone, Copy)]
struct Listing(
    &'static str,
    Option<&'static str>,
    &'static [(ArtworkKind, &'static str, &'static str)],
);

impl TitleSource for Listing {
    fn namespace(&self) -> &str {
        self.0
    }

    fn lookup_namespaces(&self) -> Vec<String> {
        vec!["aa".to_owned()]
    }

    fn search<'a>(
        &'a self,
        _query: &'a SearchQuery,
    ) -> BoxFuture<'a, Result<SearchResults, MetadataError>> {
        let id = TitleId {
            namespace: self.0.to_owned(),
            value: "1".to_owned(),
            ..Default::default()
        };
        let candidate = Candidate {
            id: id.clone().into(),
            ids: vec![id],
            title: "Dark".to_owned(),
            vote_count: 100,
            ..Default::default()
        };
        let found = SearchResults {
            candidates: vec![candidate],
            ..Default::default()
        };
        Box::pin(async { Ok(found) })
    }

    fn movie<'a>(
        &'a self,
        asked: &'a TitleAsked,
    ) -> BoxFuture<'a, Result<MovieAnswer, MetadataError>> {
        let movie = Movie {
            ids: asked.id.as_option().cloned().into_iter().collect(),
            title: "Dark".to_owned(),
            poster: self.1.map(str::to_owned),
            ..Default::default()
        };
        let found = MovieAnswer {
            movie: movie.into(),
            ..Default::default()
        };
        Box::pin(async { Ok(found) })
    }

    fn artwork<'a>(
        &'a self,
        _asked: &'a ArtworkAsked,
    ) -> BoxFuture<'a, Result<ArtworkAnswer, MetadataError>> {
        let artwork = (self.2.iter())
            .map(|&(kind, path, language)| ListedArtwork {
                kind: kind.into(),
                path: path.to_owned(),
                language: Some(language.to_owned()),
                ..Default::default()
            })
            .collect();
        let found = ArtworkAnswer {
            artwork,
            ..Default::default()
        };
        Box::pin(async { Ok(found) })
    }

    fn answers(&self, method: TitleMethod) -> bool {
        self.1.is_some() || method == TitleMethod::Artwork
    }
}

struct ListingPlugin(Listing);

impl Plugin for ListingPlugin {
    fn manifest(&self) -> Manifest {
        Manifest {
            version: "0.0.0".to_owned(),
            extension_points: vec![PointKey::provide::<dyn TitleSource>()],
            ..Manifest::new(PluginId::from_static(self.0.0))
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        registrar.provide::<dyn TitleSource>(Priority::DEFAULT, Arc::new(self.0));
        Ok(())
    }
}

/// Each image, as its Plugin and path, that matching the Movie `Dark` in a Movies Library with
/// the sources `[aa, bb]` stores.
async fn images_matching_dark(aa: Listing, bb: Listing) -> Vec<(String, String)> {
    let data = tempfile::tempdir().unwrap();
    let media = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let library = fixture::movies_library(&db, media.path()).await;
    Libraries::new(Arc::clone(&db))
        .set_title_sources(library, vec!["aa".to_owned(), "bb".to_owned()])
        .await
        .unwrap();
    let contents = Contents::new(Arc::clone(&db));
    let file = Change::Store {
        path: "/media/Dark.mkv".to_owned(),
        size: 10,
        modified_ns: 0,
        items: vec![Item::Movie {
            title: "Dark".to_owned(),
            year: None,
            hint: None,
        }],
    };
    contents.apply(library, vec![file]).await.unwrap();
    let plugins = Registry::new();
    for source in [aa, bb] {
        plugins.enable(Arc::new(ListingPlugin(source))).unwrap();
    }
    let images = ImageStore::new(data.path(), Arc::clone(&db));
    let locks = ScanLocks::for_tests();
    let matcher = Matcher::new(
        Arc::clone(&db),
        plugins.extension_points(),
        Artwork::new(images, plugins.extension_points()),
        EventBus::new(db, ExtensionPoints::default(), ServerUrl::default()),
        locks.clone(),
    );
    let scan = locks.lock(library).await;
    matcher
        .match_library(&scan, &CancellationToken::new())
        .await
        .unwrap();

    let mut stored = contents.images(Some(library)).await.unwrap();
    stored.sort();
    stored
}

fn image(plugin: &str, path: &str) -> (String, String) {
    (plugin.to_owned(), path.to_owned())
}

#[tokio::test]
async fn a_logo_only_a_list_source_has_is_stored_with_its_plugin_beside_the_own_poster() {
    let logo = &[(ArtworkKind::ARTWORK_KIND_LOGO, "/bb-logo.png", "en")];

    let stored = images_matching_dark(
        Listing("aa", Some("/aa-poster.jpg"), &[]),
        Listing("bb", None, logo),
    )
    .await;

    assert_eq!(
        stored,
        [image("aa", "/aa-poster.jpg"), image("bb", "/bb-logo.png")]
    );
}

#[tokio::test]
async fn an_en_poster_of_a_later_source_beats_another_language_of_the_first() {
    let german = &[(ArtworkKind::ARTWORK_KIND_POSTER, "/aa-de.jpg", "de")];
    let english = &[(ArtworkKind::ARTWORK_KIND_POSTER, "/bb-en.jpg", "en")];

    let stored = images_matching_dark(
        Listing("aa", Some("/aa-own.jpg"), german),
        Listing("bb", None, english),
    )
    .await;

    assert_eq!(stored, [image("bb", "/bb-en.jpg")]);
}

#[tokio::test]
async fn matching_a_pinned_movie_asks_each_source_its_namespace_once() {
    let (metadata, sources_asked) = bb_then_aa(HAS, HAS);
    sources_asked
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clear();

    movie(&metadata, &dark_movie(Some(id("aa")))).await.unwrap();

    assert_eq!(namespaces_asked(&sources_asked), ["aa", "bb"]);
}
