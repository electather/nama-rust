use std::sync::Arc;

use nama_plugin_api::{
    PluginId,
    contract::ArtworkKind,
    metadata::{ExternalId, ExternalIds, TitleMethod},
};
use tempfile::TempDir;
use tokio_util::sync::CancellationToken;

use super::season;
use crate::{
    db::Database,
    events::EventBus,
    images::ImageStore,
    library::{
        Artwork, Change, Contents, Item, Libraries, Metadata, Pin, ScanLocks, TitleSources,
        UnmatchedReason, fixture,
        scripted::{Refusal, ScriptedSource, SourcesAsked},
    },
    matching::{FoundShow, Matcher, Miss, Summary, TitleKey, testing::no_recognisers},
    plugins::{ExtensionPoints, Registry},
    server_settings::ServerUrl,
};

/// The Movie `key` names, with no Recogniser enabled.
async fn movie(metadata: &TitleSources, key: &TitleKey) -> Result<Metadata, Miss> {
    super::movie(metadata, &no_recognisers(), key, &[]).await
}

/// The Show `key` names, with no Recogniser enabled.
async fn show(metadata: &TitleSources, key: &TitleKey) -> Result<FoundShow, Miss> {
    super::show(metadata, &no_recognisers(), key, &[], &[]).await
}

/// How a scripted source answers a search, or a lookup by id.
#[derive(Clone, Copy)]
pub(super) enum Answers {
    Has,
    NotFound,
    Fails,
}

impl Answers {
    /// `source` as `has` scripts it when it has, failing `methods` when it fails.
    pub(super) fn script(
        self,
        source: ScriptedSource,
        methods: &[TitleMethod],
        has: impl FnOnce(ScriptedSource) -> ScriptedSource,
    ) -> ScriptedSource {
        match self {
            Self::Has => has(source),
            Self::NotFound => source,
            Self::Fails => source.refusing(methods, Refusal::Fails),
        }
    }
}

/// How one scripted source answers: its searches, then its lookups by id.
pub(super) type Answering = (Answers, Answers);

pub(super) const HAS: Answering = (Answers::Has, Answers::Has);

/// A source under `namespace` looking titles up by `looks_up`, answering as `answering` says. What
/// it has is the Movie and the Show `Dark`, `<namespace>-1`, with Season 1, also known as `aa-7` if
/// it looks titles up by `aa` ids.
fn dark_source(
    namespace: &'static str,
    (search, lookup): Answering,
    looks_up: &[&str],
    asked: &SourcesAsked,
) -> ScriptedSource {
    let id = format!("{namespace}-1");
    let mut ids = vec![id.as_str()];
    if looks_up.contains(&"aa") {
        ids.push("aa-7");
    }
    let source = ScriptedSource::new(namespace, asked).looking_up(looks_up);
    let source = search.script(source, &[TitleMethod::Search], |s| s.found(&[&id], "Dark"));
    let lookups = [TitleMethod::Movie, TitleMethod::Show, TitleMethod::Season];
    lookup.script(source, &lookups, |s| {
        (s.with_movie(&ids, "Dark").with_show(&ids, "Dark", &[1])).with_season(&id, 1, &[])
    })
}

/// Title sources under `aa`, looking titles up by `aa_looks_up`, and `bb`, looking them up by `aa`
/// ids, answering as given; `aa` also has whatever `aa_has` scripts.
fn looking_up(
    aa: Answering,
    bb: Answering,
    aa_looks_up: &[&str],
    aa_has: impl FnOnce(ScriptedSource) -> ScriptedSource,
) -> (Registry, SourcesAsked) {
    let plugins = Registry::new();
    let asked = SourcesAsked::default();
    let aa = aa_has(dark_source("aa", aa, aa_looks_up, &asked));
    for source in [aa, dark_source("bb", bb, &["aa"], &asked)] {
        plugins.enable(Arc::new(source)).unwrap();
    }
    (plugins, asked)
}

fn bb_aa() -> Vec<String> {
    vec!["bb".to_owned(), "aa".to_owned()]
}

/// Title sources under `aa` and `bb`, `bb` also looking titles up by `aa` ids, asked in the order
/// `[bb, aa]`, answering as given.
fn bb_then_aa(aa: Answering, bb: Answering) -> (TitleSources, SourcesAsked) {
    let (plugins, asked) = looking_up(aa, bb, &[], |aa| aa);
    let metadata = TitleSources::new(plugins.extension_points());
    (metadata.in_order(bb_aa()), asked)
}

/// The Show `Dark`, its folder naming `hint`.
pub(super) fn dark(hint: Option<ExternalId>) -> TitleKey {
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
        nfo: Vec::new(),
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
    assert_eq!(sources_asked.lines(), ["aa show aa-7", "bb show aa-7"]);
}

#[tokio::test]
async fn a_search_bb_finds_nothing_for_falls_to_aa() {
    let (metadata, sources_asked) = bb_then_aa(HAS, (Answers::NotFound, Answers::Has));

    let found = show(&metadata, &dark(None)).await.unwrap();

    assert_eq!(found.source.namespace, "aa");
    assert_eq!(found.id, id("aa"));
    assert_eq!(
        sources_asked.lines(),
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
    assert_eq!(sources_asked.lines(), ["bb search"]);
}

#[tokio::test]
async fn a_hint_no_source_looks_up_by_is_searched_for_instead() {
    let (metadata, sources_asked) = bb_then_aa(HAS, HAS);
    let tag = ExternalId::new("x264", "GROUP").unwrap();

    let found = show(&metadata, &dark(Some(tag))).await.unwrap();

    assert_eq!(found.source.namespace, "bb");
    assert_eq!(sources_asked.lines(), ["bb search", "bb show bb-1"]);
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
    assert_eq!(sources_asked.lines(), Vec::<String>::new());
}

#[tokio::test]
async fn seasons_come_from_the_source_that_gave_the_show_by_its_id_there() {
    let (metadata, sources_asked) = bb_then_aa((Answers::Has, Answers::NotFound), HAS);
    let hint = ExternalId::new("aa", "7").unwrap();
    let found = show(&metadata, &dark(Some(hint))).await.unwrap();

    season(&metadata, &found, 1).await.unwrap();

    assert_eq!(sources_asked.lines().last().unwrap(), "bb season bb-1 1");
}

/// What matching a Movies Library with sources `[bb, aa]`, `aa` looking titles up by
/// `aa_looks_up` and having the Movie `pin` names, does with the Movies `Aaa`, pinned to `pin`,
/// and `Dark`.
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
        edition: None,
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
    let pin_id = pin.to_string();
    let pin = Pin::Title(pin);
    contents
        .pin(library, vec![named.name], pin, vec![])
        .await
        .unwrap();
    let (plugins, _) = looking_up(HAS, HAS, aa_looks_up, |aa| aa.with_movie(&[&pin_id], "Aaa"));
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
async fn a_source_neither_keeping_nor_looking_up_a_hint_is_never_asked() {
    let (plugins, sources_asked) = looking_up(HAS, (Answers::Has, Answers::NotFound), &[], |aa| aa);
    let metadata = TitleSources::new(plugins.extension_points()).in_order(bb_aa());

    // `bb` looks up `aa` ids but `aa` doesn't look up `bb` ones: a `bb` hint `bb` misses is blamed.
    assert_eq!(
        show(&metadata, &dark(Some(id("bb")))).await.unwrap_err(),
        Miss::Unmatched(Some(UnmatchedReason::UnknownIdHint))
    );
    assert_eq!(sources_asked.lines(), ["bb show bb-1"]);
}

/// A source like fanart.tv's, keeping titles under `namespace`, looking them up by `looks_up` and
/// answering only the artwork list.
fn artwork_only(namespace: &'static str, looks_up: &str, asked: &SourcesAsked) -> ScriptedSource {
    let source = ScriptedSource::new(namespace, asked).looking_up(&[looks_up]);
    source.serving(&[TitleMethod::Artwork])
}

/// An artwork-only `fanart` source looking titles up by `tmdb`, with a `tmdb` source that has
/// everything if `tmdb`.
fn fanart(tmdb: bool) -> (TitleSources, SourcesAsked) {
    let plugins = Registry::new();
    let asked = SourcesAsked::default();
    let fanart = artwork_only("fanart", "tmdb", &asked);
    plugins.enable(Arc::new(fanart)).unwrap();
    if tmdb {
        let has_everything = dark_source("tmdb", HAS, &[], &asked);
        plugins.enable(Arc::new(has_everything)).unwrap();
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
        nfo: Vec::new(),
    }
}

#[tokio::test]
async fn a_tmdb_pin_only_an_artwork_source_looks_up_stays_unserved() {
    let (metadata, sources_asked) = fanart(false);

    let pinned = movie(&metadata, &dark_movie(Some(ExternalId::tmdb(603))))
        .await
        .unwrap_err();

    assert_eq!(pinned, Miss::Unserved);
    assert_eq!(sources_asked.lines(), Vec::<String>::new());
}

#[tokio::test]
async fn a_tmdb_pin_goes_to_tmdb_beside_an_artwork_source() {
    let (metadata, sources_asked) = fanart(true);

    let found = movie(&metadata, &dark_movie(Some(ExternalId::tmdb(1))))
        .await
        .unwrap();

    assert_eq!(found.plugin, PluginId::from_static("tmdb"));
    assert_eq!(sources_asked.lines(), ["tmdb movie tmdb-1"]);
}

#[tokio::test]
async fn a_search_never_asks_an_artwork_source() {
    let (metadata, sources_asked) = fanart(true);

    let found = movie(&metadata, &dark_movie(None)).await.unwrap();

    assert_eq!(found.plugin, PluginId::from_static("tmdb"));
    assert_eq!(sources_asked.lines(), ["tmdb search", "tmdb movie tmdb-1"]);
}

/// A source under `aa` that searches and has the Movie `Dark`, `aa-1`, with the poster `poster`.
fn aa_with_poster(poster: &str) -> ScriptedSource {
    let aa = ScriptedSource::new("aa", &SourcesAsked::default());
    (aa.found(&["aa-1"], "Dark").with_movie(&["aa-1"], "Dark")).poster(poster)
}

/// A source under `bb` looking titles up by `aa` that answers only the artwork list.
fn bb_listing() -> ScriptedSource {
    artwork_only("bb", "aa", &SourcesAsked::default())
}

/// The Movie `Dark`, Matched in a Movies Library with the sources `[aa, bb]`.
struct DarkMatched {
    contents: Contents,
    library: i64,
    matcher: Matcher,
    locks: ScanLocks,
    _data: TempDir,
    _media: TempDir,
}

impl DarkMatched {
    async fn new(aa: ScriptedSource, bb: ScriptedSource) -> Self {
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
            edition: None,
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
            plugins.enable(Arc::new(source)).unwrap();
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
        let dark = Self {
            contents,
            library,
            matcher,
            locks,
            _data: data,
            _media: media,
        };
        dark.match_library().await;
        dark
    }

    /// Matches the Library as a scan does.
    async fn match_library(&self) {
        let scan = self.locks.lock(self.library).await;
        (self.matcher)
            .match_library(&scan, &CancellationToken::new())
            .await
            .unwrap();
    }

    /// Each image, as its Plugin and path, that the Match stores.
    async fn images(&self) -> Vec<(String, String)> {
        let mut stored = self.contents.images(Some(self.library)).await.unwrap();
        stored.sort();
        stored
    }

    /// The titled Backdrop the Match stores, as its Plugin and path.
    async fn titled_backdrop(&self) -> Option<(String, String)> {
        let movie = self.contents.movies(self.library, None).await.unwrap();
        let backdrop = movie.into_iter().next().unwrap().titled_backdrop;
        backdrop.map(|image| (image.plugin.to_string(), image.path))
    }
}

/// Each image, as its Plugin and path, that matching the Movie `Dark` in a Movies Library with
/// the sources `[aa, bb]` stores.
async fn images_matching_dark(aa: ScriptedSource, bb: ScriptedSource) -> Vec<(String, String)> {
    DarkMatched::new(aa, bb).await.images().await
}

fn image(plugin: &str, path: &str) -> (String, String) {
    (plugin.to_owned(), path.to_owned())
}

#[tokio::test]
async fn a_logo_only_a_list_source_has_is_stored_with_its_plugin_beside_the_own_poster() {
    let logo = bb_listing().with_artwork(ArtworkKind::ARTWORK_KIND_LOGO, "/bb-logo.png", "en");

    let stored = images_matching_dark(aa_with_poster("/aa-poster.jpg"), logo).await;

    assert_eq!(
        stored,
        [image("aa", "/aa-poster.jpg"), image("bb", "/bb-logo.png")]
    );
}

#[tokio::test]
async fn an_en_poster_of_a_later_source_beats_another_language_of_the_first() {
    let poster = ArtworkKind::ARTWORK_KIND_POSTER;
    let german = aa_with_poster("/aa-own.jpg").with_artwork(poster, "/aa-de.jpg", "de");
    let english = bb_listing().with_artwork(poster, "/bb-en.jpg", "en");

    let stored = images_matching_dark(german, english).await;

    assert_eq!(stored, [image("bb", "/bb-en.jpg")]);
}

#[tokio::test]
async fn a_movie_matched_before_titled_backdrops_gets_its_titled_backdrop_on_the_next_scan() {
    let backdrop = ArtworkKind::ARTWORK_KIND_BACKDROP;
    let titled = bb_listing().with_artwork(backdrop, "/bb-en.jpg", "en");
    let dark = DarkMatched::new(aa_with_poster("/aa-poster.jpg"), titled).await;
    dark.contents.stored_by_older_metadata(dark.library).await;
    let before = dark.titled_backdrop().await;

    dark.match_library().await;

    assert_eq!(before, None);
    assert_eq!(
        dark.titled_backdrop().await,
        Some(image("bb", "/bb-en.jpg"))
    );
}

#[tokio::test]
async fn matching_a_pinned_movie_asks_each_source_its_namespace_once() {
    let (metadata, sources_asked) = bb_then_aa(HAS, HAS);
    let before = sources_asked.namespaces().len();

    movie(&metadata, &dark_movie(Some(id("aa")))).await.unwrap();

    assert_eq!(sources_asked.namespaces()[before..], ["aa", "bb"]);
}
