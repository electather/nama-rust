use std::sync::{Arc, Mutex, PoisonError};

use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    contract::{
        Candidate, Movie, MovieAnswer, SearchQuery, SearchResults, TitleAsked, TitleId,
        TitleRecognised, TitleToRecognise,
    },
    metadata::{ExternalId, MetadataError, TitleSource},
    recogniser::Recogniser,
};
use tokio_util::sync::CancellationToken;

use super::{Recognisers, relative};
use crate::{
    db::Database,
    events::EventBus,
    images::ImageStore,
    library::artwork::Artwork,
    library::contents::{Change, Contents, Item, UnmatchedReason, fixture},
    library::custom_images::CustomImages,
    library::libraries::Libraries,
    library::title_source::TitleSources,
    matching::{Miss, Summary, TitleKey, sources},
    plugins::{ExtensionPoints, Registry},
    scanner::Scanner,
    server_settings::ServerUrl,
};

/// A title source under `aa` whose search finds two Movies called `Dark`, `aa-1` and `aa-2`, which
/// fit equally well, so the search alone leaves `Dark` Ambiguous.
struct Twins;

fn aa(value: &str) -> TitleId {
    TitleId {
        namespace: "aa".to_owned(),
        value: value.to_owned(),
        ..Default::default()
    }
}

impl TitleSource for Twins {
    fn namespace(&self) -> &'static str {
        "aa"
    }

    fn search<'a>(
        &'a self,
        _query: &'a SearchQuery,
    ) -> BoxFuture<'a, Result<SearchResults, MetadataError>> {
        let twin = |value| Candidate {
            id: aa(value).into(),
            ids: vec![aa(value)],
            title: "Dark".to_owned(),
            vote_count: 100,
            ..Default::default()
        };
        let found = SearchResults {
            candidates: vec![twin("1"), twin("2")],
            ..Default::default()
        };
        Box::pin(async { Ok(found) })
    }

    fn movie<'a>(
        &'a self,
        asked: &'a TitleAsked,
    ) -> BoxFuture<'a, Result<MovieAnswer, MetadataError>> {
        let movie = MovieAnswer {
            movie: Movie {
                ids: vec![aa(&asked.id.value)],
                title: format!("Dark {}", asked.id.value),
                ..Default::default()
            }
            .into(),
            ..Default::default()
        };
        Box::pin(async { Ok(movie) })
    }
}

/// How a [`Fake`] Recogniser answers.
#[derive(Clone)]
enum Answers {
    /// The ids `namespace`, `value`.
    Names(&'static [(&'static str, &'static str)]),
    Fails,
}

/// What the Recogniser was asked, in the order asked.
type Asked = Arc<Mutex<Vec<TitleToRecognise>>>;

/// A Built-in Recogniser answering as told.
struct Fake(Answers, Asked);

impl Recogniser for Fake {
    fn recognise_title<'a>(
        &'a self,
        title: &'a TitleToRecognise,
    ) -> BoxFuture<'a, Result<TitleRecognised, PluginError>> {
        let mut log = self.1.lock().unwrap_or_else(PoisonError::into_inner);
        log.push(title.clone());
        let answer = match self.0 {
            Answers::Names(ids) => Ok(TitleRecognised {
                ids: (ids.iter())
                    .map(|&(namespace, value)| TitleId {
                        namespace: namespace.to_owned(),
                        value: value.to_owned(),
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            }),
            Answers::Fails => Err("unreachable".into()),
        };
        Box::pin(async { answer })
    }
}

/// The Plugin `aa` serving [`Twins`], and the Plugin `rr` serving a [`Fake`] when given one.
struct FakePlugin(Option<(Answers, Asked)>);

impl Plugin for FakePlugin {
    fn manifest(&self) -> Manifest {
        let (id, point) = match self.0 {
            Some(_) => ("rr", PointKey::provide::<dyn Recogniser>()),
            None => ("aa", PointKey::provide::<dyn TitleSource>()),
        };
        Manifest {
            version: "0.0.0".to_owned(),
            extension_points: vec![point],
            ..Manifest::new(PluginId::from_static(id))
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        match &self.0 {
            Some((answers, asked)) => {
                let fake = Fake(answers.clone(), Arc::clone(asked));
                registrar.provide::<dyn Recogniser>(Priority::DEFAULT, Arc::new(fake));
            }
            None => registrar.provide::<dyn TitleSource>(Priority::DEFAULT, Arc::new(Twins)),
        }
        Ok(())
    }
}

/// [`Twins`] and a Recogniser answering `answers`, enabled, and what the Recogniser is asked.
fn enabled(answers: Answers) -> (Registry, Asked) {
    let plugins = Registry::new();
    let asked = Asked::default();
    plugins.enable(Arc::new(FakePlugin(None))).unwrap();
    let recogniser = FakePlugin(Some((answers, Arc::clone(&asked))));
    plugins.enable(Arc::new(recogniser)).unwrap();
    (plugins, asked)
}

/// The Movie `Dark`, as a file names it.
fn dark() -> TitleKey {
    let item = Item::Movie {
        title: "Dark".to_owned(),
        year: None,
        hint: None,
    };
    TitleKey {
        name: item.name(),
        pin: None,
    }
}

/// What Matching finds for [`dark`] with a Recogniser answering `answers`, asking those of
/// `listed`.
async fn dark_listing(listed: &[&str], answers: Answers) -> (Result<String, Miss>, Asked) {
    let (plugins, asked) = enabled(answers);
    let metadata = TitleSources::new(plugins.extension_points());
    let order: Vec<String> = listed.iter().map(|&id| id.to_owned()).collect();
    let recognisers = Recognisers::new(plugins.extension_points()).in_order(&order);
    let found = sources::movie(&metadata, &recognisers, &dark(), &[]).await;
    (found.map(|movie| movie.title), asked)
}

/// [`dark_listing`] with the Recogniser listed.
async fn dark_with(answers: Answers) -> (Result<String, Miss>, Asked) {
    dark_listing(&["rr"], answers).await
}

#[tokio::test]
async fn a_title_the_search_leaves_ambiguous_matches_the_candidate_a_recogniser_names() {
    let (found, asked) = dark_with(Answers::Names(&[("aa", "2")])).await;

    assert_eq!(found.unwrap(), "Dark 2");
    let asked = asked.lock().unwrap_or_else(PoisonError::into_inner);
    assert_eq!(asked[0].title, "Dark");
    assert_eq!(asked[0].candidates.len(), 2);
}

#[tokio::test]
async fn a_recogniser_naming_an_id_no_source_serves_is_ignored() {
    let (found, _) = dark_with(Answers::Names(&[("zz", "9")])).await;

    assert!(
        matches!(
            found,
            Err(Miss::Unmatched(Some(UnmatchedReason::Ambiguous(_))))
        ),
        "{found:?}"
    );
}

#[tokio::test]
async fn a_recogniser_the_library_doesnt_list_isnt_asked() {
    let (found, asked) = dark_listing(&["gone"], Answers::Names(&[("aa", "1")])).await;

    assert!(
        matches!(
            found,
            Err(Miss::Unmatched(Some(UnmatchedReason::Ambiguous(_))))
        ),
        "{found:?}"
    );
    assert!(
        asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_empty()
    );
}

#[tokio::test]
async fn a_recogniser_that_cant_tell_leaves_the_search_s_decision() {
    let (found, _) = dark_with(Answers::Names(&[])).await;

    assert!(
        matches!(
            found,
            Err(Miss::Unmatched(Some(UnmatchedReason::Ambiguous(_))))
        ),
        "{found:?}"
    );
}

/// Matches a Movies Library holding `Dark/Dark.mkv` with a Recogniser answering `answers`. Returns
/// what Matching decided, the Items still undecided after, and what the Recogniser was asked.
async fn match_dark_file(answers: Answers) -> (Summary, usize, Asked) {
    let data = tempfile::tempdir().unwrap();
    let media = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let library = fixture::movies_library(&db, media.path()).await;
    let libraries = Libraries::new(Arc::clone(&db));
    (libraries.set_title_sources(library, vec!["aa".to_owned()]))
        .await
        .unwrap();
    (libraries.set_recognisers(library, vec!["rr".to_owned()]))
        .await
        .unwrap();
    let contents = Contents::new(Arc::clone(&db));
    let path = media.path().canonicalize().unwrap().join("Dark/Dark.mkv");
    let file = Change::Store {
        path: path.to_str().unwrap().to_owned(),
        size: 10,
        modified_ns: 0,
        items: vec![Item::Movie {
            title: "Dark".to_owned(),
            year: None,
            hint: None,
        }],
    };
    contents.apply(library, vec![file]).await.unwrap();
    let (plugins, asked) = enabled(answers);
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
    let undecided = contents.undecided(library).await.unwrap().len();
    (summary, undecided, asked)
}

#[tokio::test]
async fn a_failing_recogniser_leaves_the_item_undecided_and_stores_nothing() {
    let (summary, undecided, _) = match_dark_file(Answers::Fails).await;

    let left = Summary {
        undecided: 1,
        ..Summary::default()
    };
    assert_eq!(summary, left);
    assert_eq!(undecided, 1);
}

#[tokio::test]
async fn the_recogniser_receives_the_items_path_relative_to_the_library_folder() {
    let (summary, _, asked) = match_dark_file(Answers::Names(&[("aa", "1")])).await;

    assert_eq!(summary.matched, 1);
    let asked = asked.lock().unwrap_or_else(PoisonError::into_inner);
    assert_eq!(asked[0].paths, ["Dark/Dark.mkv"]);
}

#[test]
fn a_path_under_none_of_the_folders_has_no_relative_path() {
    let folders = ["/media/movies".to_owned()];

    assert_eq!(relative("/elsewhere/Dark.mkv", &folders), None);
    assert_eq!(
        relative("/media/movies/Dark/Dark.mkv", &folders).as_deref(),
        Some("Dark/Dark.mkv")
    );
}

#[test]
fn ids_a_recogniser_names_are_well_formed_only() {
    let named = TitleRecognised {
        ids: vec![aa("1"), TitleId::default()],
        ..Default::default()
    };

    let ids = super::well_formed(&PluginId::from_static("rr"), &named);

    assert_eq!(ids, [ExternalId::new("aa", "1").unwrap()]);
}
