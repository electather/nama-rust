use std::sync::{Arc, Mutex, PoisonError};

use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    contract::{
        Candidate, Movie, MovieAnswer, SearchQuery, SearchResults, Season, SeasonAnswer,
        SeasonAsked, Show, ShowAnswer, TitleAsked, TitleId,
    },
    metadata::{ExternalId, ExternalIds, MetadataError, TitleSource},
};
use tokio_util::sync::CancellationToken;

use super::{season, show};
use crate::{
    db::Database,
    events::EventBus,
    images::ImageStore,
    library::artwork::Artwork,
    library::contents::{Change, Contents, Item, Pin, UnmatchedReason, fixture},
    library::custom_images::CustomImages,
    library::libraries::Libraries,
    library::title_source::TitleSources,
    matching::{Miss, Summary, TitleKey},
    plugins::{ExtensionPoints, Registry},
    scanner::Scanner,
    server_settings::ServerUrl,
};

/// How a [`Fake`] title source answers a search, or a lookup by id.
#[derive(Clone, Copy)]
enum Answers {
    Has,
    NotFound,
    Fails,
}

/// What the sources were asked, as `namespace method id`, in the order asked.
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
    asked.lock().unwrap_or_else(PoisonError::into_inner).clone()
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
    let scanner = Scanner::new(
        Arc::clone(&db),
        plugins.extension_points(),
        Artwork::new(images.clone(), plugins.extension_points()),
        CustomImages::new(Arc::clone(&db), images),
        EventBus::new(db, ExtensionPoints::default(), ServerUrl::default()),
        None,
        None,
        None,
    );

    let scan = scanner.lock(library).await;
    scanner
        .matcher()
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
