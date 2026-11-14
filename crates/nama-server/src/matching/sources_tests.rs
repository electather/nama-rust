use std::sync::{Arc, Mutex, PoisonError};

use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    metadata::{
        Candidate, ExternalId, ExternalIds, MetadataError, Movie, SearchQuery, Season, Show,
        TitleSource,
    },
    settings::SettingsSchema,
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
    library::title_source::MetadataSource,
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

/// A title source keeping titles under `namespace`, as the Plugin of that id. Whatever it has is
/// the Show `Dark`, its id `<namespace>-1`.
struct Fake {
    namespace: &'static str,
    search: Answers,
    lookup: Answers,
    asked: Asked,
}

impl Fake {
    fn ids(&self) -> ExternalIds {
        ExternalIds::from_iter([ExternalId::new(self.namespace, "1").unwrap()])
    }

    fn answer<T>(&self, asked: &str, answers: Answers, found: T) -> Result<T, MetadataError> {
        let mut log = self.asked.lock().unwrap_or_else(PoisonError::into_inner);
        log.push(format!("{} {asked}", self.namespace));
        match answers {
            Answers::Has => Ok(found),
            Answers::NotFound => Err(MetadataError::NotFound),
            Answers::Fails => Err(MetadataError::Failed("unreachable".into())),
        }
    }
}

impl TitleSource for Fake {
    fn namespace(&self) -> &str {
        self.namespace
    }

    fn search<'a>(
        &'a self,
        _query: &'a SearchQuery,
    ) -> BoxFuture<'a, Result<Vec<Candidate>, MetadataError>> {
        let candidate = Candidate {
            id: ExternalId::new(self.namespace, "1").unwrap(),
            ids: self.ids(),
            title: "Dark".to_owned(),
            original_title: None,
            year: None,
            poster: None,
            origin_country: Vec::new(),
            vote_count: 100,
        };
        let found = self.answer("search", self.search, vec![candidate]);
        Box::pin(async { found })
    }

    fn movie<'a>(&'a self, id: &'a ExternalId) -> BoxFuture<'a, Result<Movie, MetadataError>> {
        let movie = Movie {
            ids: self.ids(),
            title: "Dark".to_owned(),
            original_title: None,
            overview: None,
            release_date: None,
            runtime_minutes: None,
            poster: None,
            backdrop: None,
        };
        let found = self.answer(&format!("movie {id}"), self.lookup, movie);
        Box::pin(async { found })
    }

    fn show<'a>(&'a self, id: &'a ExternalId) -> BoxFuture<'a, Result<Show, MetadataError>> {
        let show = Show {
            ids: self.ids(),
            title: "Dark".to_owned(),
            original_title: None,
            overview: None,
            first_air_date: None,
            poster: None,
            backdrop: None,
            seasons: vec![1],
        };
        let found = self.answer(&format!("show {id}"), self.lookup, show);
        Box::pin(async { found })
    }

    fn season<'a>(
        &'a self,
        show: &'a ExternalId,
        number: u32,
    ) -> BoxFuture<'a, Result<Season, MetadataError>> {
        let season = Season {
            ids: ExternalIds::default(),
            number,
            title: String::new(),
            overview: None,
            air_date: None,
            poster: None,
            episodes: Vec::new(),
        };
        let found = self.answer(&format!("season {show}"), self.lookup, season);
        Box::pin(async { found })
    }
}

struct FakePlugin(&'static str, Answers, Answers, Asked);

impl Plugin for FakePlugin {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: PluginId::from_static(self.0),
            name: String::new(),
            description: String::new(),
            version: "0.0.0".to_owned(),
            settings: SettingsSchema::default(),
            extension_points: vec![PointKey::provide::<dyn TitleSource>()],
            logo_svg: None,
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        let source = Fake {
            namespace: self.0,
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

/// Title sources under `aa` and `bb`, answering as given.
fn aa_and_bb(aa: Answering, bb: Answering) -> (Registry, Asked) {
    let plugins = Registry::new();
    let asked = Asked::default();
    for (namespace, (search, lookup)) in [("aa", aa), ("bb", bb)] {
        let plugin = FakePlugin(namespace, search, lookup, Arc::clone(&asked));
        plugins.enable(Arc::new(plugin)).unwrap();
    }
    (plugins, asked)
}

fn bb_aa() -> Vec<String> {
    vec!["bb".to_owned(), "aa".to_owned()]
}

/// Title sources under `aa` and `bb`, asked in the order `[bb, aa]`, answering as given.
fn bb_then_aa(aa: Answering, bb: Answering) -> (MetadataSource, Asked) {
    let (plugins, asked) = aa_and_bb(aa, bb);
    let metadata = MetadataSource::new(plugins.extension_points());
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

#[tokio::test]
async fn a_pin_no_source_looks_up_leaves_the_rest_of_the_library_matching() {
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
    let pin = Pin::Title(id("cc"));
    contents
        .pin(library, named.name, pin, vec![])
        .await
        .unwrap();
    let (plugins, _) = aa_and_bb(HAS, HAS);
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
    let summary = scanner
        .matcher()
        .match_library(&scan, &CancellationToken::new())
        .await
        .unwrap();

    let expected = Summary {
        matched: 1,
        unmatched: 0,
        undecided: 1,
    };
    assert_eq!(summary, expected);
}
