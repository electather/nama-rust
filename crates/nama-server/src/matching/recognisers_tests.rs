use std::sync::{Arc, Mutex, PoisonError};

use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    contract::{TitleId, TitleRecognised, TitleToRecognise},
    metadata::ExternalId,
    recogniser::{Recogniser, RecogniserMethod},
};
use tokio_util::sync::CancellationToken;

use super::relative;
use crate::{
    db::Database,
    events::EventBus,
    images::ImageStore,
    library::{
        Artwork, Change, Contents, Item, Libraries, METADATA_VERSION, Recognisers, ScanLocks,
        TitleSources, UnmatchedReason, fixture,
        scripted::{ScriptedSource, SourcesAsked},
    },
    log_capture::CapturedLog,
    matching::{
        Matcher, Miss, Summary, TitleKey, sources,
        testing::{Fixture, Library, Outcome, episode, matched, movie},
    },
    plugins::{ExtensionPoints, Registry},
    server_settings::ServerUrl,
};

fn aa(value: &str) -> TitleId {
    TitleId {
        namespace: "aa".to_owned(),
        value: value.to_owned(),
        ..Default::default()
    }
}

/// How a [`Fake`] Recogniser answers.
#[derive(Clone)]
enum Answers {
    /// The ids `namespace`, `value`.
    Names(&'static [(&'static str, &'static str)]),
    Fails,
    /// Fails when asked about the title `.0`, and can't tell any other.
    FailsFor(&'static str),
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
            Answers::FailsFor(failing) if title.title == failing => Err("unreachable".into()),
            Answers::FailsFor(_) => Ok(TitleRecognised::default()),
        };
        Box::pin(async { answer })
    }
}

/// The Plugin `rr` serving a [`Fake`].
struct FakePlugin(Answers, Asked);

impl Plugin for FakePlugin {
    fn manifest(&self) -> Manifest {
        Manifest {
            version: "0.0.0".to_owned(),
            extension_points: vec![PointKey::provide::<dyn Recogniser>()],
            ..Manifest::new(PluginId::from_static("rr"))
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        let fake = Fake(self.0.clone(), Arc::clone(&self.1));
        registrar.provide::<dyn Recogniser>(Priority::DEFAULT, Arc::new(fake));
        Ok(())
    }
}

/// A title source under `aa` whose search finds two Movies called `Dark`, `aa-1` and `aa-2`, which
/// fit equally well, so the search alone leaves `Dark` Ambiguous, and a Recogniser answering
/// `answers`, enabled, and what the Recogniser is asked.
fn enabled(answers: Answers) -> (Registry, Asked) {
    let plugins = Registry::new();
    let asked = Asked::default();
    let twins = ScriptedSource::new("aa", &SourcesAsked::default())
        .found(&["aa-1"], "Dark")
        .found(&["aa-2"], "Dark")
        .with_movie(&["aa-1"], "Dark 1")
        .with_movie(&["aa-2"], "Dark 2");
    plugins.enable(Arc::new(twins)).unwrap();
    let recogniser = FakePlugin(answers, Arc::clone(&asked));
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

/// What Matching finds for [`dark`] with a Recogniser answering `answers`, listed.
async fn dark_with(answers: Answers) -> (Result<String, Miss>, Asked) {
    let (plugins, asked) = enabled(answers);
    let metadata = TitleSources::new(plugins.extension_points());
    let recognisers = Recognisers::new(plugins.extension_points(), RecogniserMethod::Title)
        .in_order(&["rr".to_owned()]);
    let found = sources::movie(&metadata, &recognisers, &dark(), &[]).await;
    (found.map(|movie| movie.title), asked)
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
    let locks = ScanLocks::for_tests();
    let matcher = Matcher::new(
        Arc::clone(&db),
        plugins.extension_points(),
        Artwork::new(images, plugins.extension_points()),
        EventBus::new(db, ExtensionPoints::default(), ServerUrl::default()),
        locks.clone(),
    );

    let scan = locks.lock(library).await;
    let summary = matcher
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

/// TMDB, and a Recogniser enabled and listed in both of `media`'s Libraries that answers
/// `answers`.
async fn recognising(answers: Answers) -> Fixture {
    let media = Fixture::new().await;
    let recogniser = FakePlugin(answers, Asked::default());
    media.plugins.enable(Arc::new(recogniser)).unwrap();
    for library in [media.movies.id, media.shows.id] {
        (media
            .libraries
            .set_recognisers(library, vec!["rr".to_owned()]))
        .await
        .unwrap();
    }
    media
}

async fn outcomes_after_scan(media: &Fixture, library: &Library) -> Vec<Outcome> {
    media.scan(library).await;
    media.outcomes(library).await
}

#[tokio::test]
async fn a_recogniser_failing_for_one_movie_leaves_the_next_movie_matched_in_the_same_pass() {
    let media = recognising(Answers::FailsFor("Alien")).await;
    media
        .movies
        .write("Alien (1979)/Alien (1979).mkv", &[movie("Alien", 1979)])
        .await;
    media
        .movies
        .write("Dune (2021)/Dune (2021).mkv", &[movie("Dune", 2021)])
        .await;

    let outcomes = outcomes_after_scan(&media, &media.movies).await;

    assert_eq!(outcomes, [Outcome::Undecided, matched(438_631, "Dune")]);
}

#[tokio::test]
async fn a_recogniser_failing_for_one_show_leaves_another_shows_episodes_matched() {
    let media = recognising(Answers::FailsFor("Dark")).await;
    media
        .shows
        .write(
            "Dark (2017)/Season 01/Dark (2017) - S01E03.mkv",
            &[episode("Dark", 2017, 1, 3)],
        )
        .await;
    media
        .shows
        .write(
            "Euphoria (US)/Season 01/Euphoria (US) - S01E01.mkv",
            &[episode("Euphoria (US)", None, 1, 1)],
        )
        .await;

    let outcomes = outcomes_after_scan(&media, &media.shows).await;

    assert_eq!(outcomes, [Outcome::Undecided, matched(1_734_395, "Pilot")]);
}

#[tokio::test]
async fn a_recogniser_failing_for_one_title_still_lets_the_scan_refresh_outdated_matches() {
    let media = recognising(Answers::FailsFor("Alien")).await;
    media
        .movies
        .write("Dune (2021)/Dune (2021).mkv", &[movie("Dune", 2021)])
        .await;
    media.scan(&media.movies).await;
    media
        .contents
        .stored_by_older_metadata(media.movies.id)
        .await;
    media
        .movies
        .write("Alien (1979)/Alien (1979).mkv", &[movie("Alien", 1979)])
        .await;

    media.scan(&media.movies).await;

    let outdated = (media.contents)
        .matched_below(media.movies.id, METADATA_VERSION)
        .await
        .unwrap();
    assert!(outdated.is_empty(), "{outdated:?}");
    assert_eq!(
        media.outcomes(&media.movies).await,
        [Outcome::Undecided, matched(438_631, "Dune")]
    );
}

#[tokio::test]
async fn a_recogniser_failing_on_several_titles_is_logged_once_per_match_with_the_count() {
    let media = recognising(Answers::Fails).await;
    media
        .movies
        .write("Alien (1979)/Alien (1979).mkv", &[movie("Alien", 1979)])
        .await;
    media
        .movies
        .write("Dune (2021)/Dune (2021).mkv", &[movie("Dune", 2021)])
        .await;

    for _ in 0..2 {
        // The test's runtime has one thread, so Matching logs to it.
        let log = CapturedLog::start();
        media.scan(&media.movies).await;
        let log = log.finish();
        let failed: Vec<&str> = (log.lines())
            .filter(|line| line.contains("plugin=rr"))
            .collect();
        assert_eq!(failed.len(), 1, "{log}");
        assert!(failed[0].contains("titles=2"), "{log}");
    }
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
