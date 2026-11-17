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
    library::{
        Artwork, Change, Contents, Item, Libraries, ScanLocks, TitleSources, UnmatchedReason,
        fixture,
    },
    log_capture::CapturedLog,
    matching::{
        Matcher, Miss, Summary, TitleKey, sources,
        testing::{Fixture, Library, Outcome, episode, matched, movie},
    },
    plugins::{ExtensionPoints, Registry},
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

/// A Movies Library listing the title source `aa` and the Recogniser `rr`, and a Matcher.
struct MoviesLibrary {
    matcher: Matcher,
    locks: ScanLocks,
    library: i64,
    contents: Contents,
    /// What the Recogniser was asked.
    asked: Asked,
    _folders: [tempfile::TempDir; 2],
}

impl MoviesLibrary {
    /// A Library holding `<title>/<title>.mkv` for each of `titles`, with a Recogniser answering
    /// `answers`.
    async fn holding(titles: &[&str], answers: Answers) -> Self {
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
        let folder = media.path().canonicalize().unwrap();
        let files = titles.iter().map(|title| Change::Store {
            path: folder
                .join(format!("{title}/{title}.mkv"))
                .to_str()
                .unwrap()
                .to_owned(),
            size: 10,
            modified_ns: 0,
            items: vec![Item::Movie {
                title: (*title).to_owned(),
                year: None,
                hint: None,
            }],
        });
        contents.apply(library, files.collect()).await.unwrap();
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
        Self {
            matcher,
            locks,
            library,
            contents,
            asked,
            _folders: [data, media],
        }
    }

    async fn match_library(&self) -> Summary {
        let scan = self.locks.lock(self.library).await;
        (self.matcher.match_library(&scan, &CancellationToken::new()))
            .await
            .unwrap()
    }
}

/// Matches a Movies Library holding `Dark/Dark.mkv` with a Recogniser answering `answers`. Returns
/// what Matching decided, the Items still undecided after, and what the Recogniser was asked.
async fn match_dark_file(answers: Answers) -> (Summary, usize, Asked) {
    let movies = MoviesLibrary::holding(&["Dark"], answers).await;

    let summary = movies.match_library().await;

    let undecided = movies.contents.undecided(movies.library).await.unwrap();
    (summary, undecided.len(), movies.asked)
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

#[tokio::test]
async fn a_recogniser_failing_on_several_titles_is_logged_once_per_match_with_the_count() {
    let movies = MoviesLibrary::holding(&["Alien", "Dark", "Dune"], Answers::Fails).await;
    let logged_failures = || async {
        // The test's runtime has one thread, so Matching logs to it.
        let log = CapturedLog::start();
        movies.match_library().await;
        let log = log.finish();
        let failed: Vec<String> = (log.lines())
            .filter(|line| line.contains("plugin=rr"))
            .map(str::to_owned)
            .collect();
        (failed, log)
    };

    for _ in 0..2 {
        let (failed, log) = logged_failures().await;
        assert_eq!(failed.len(), 1, "{log}");
        assert!(failed[0].contains("titles=3"), "{log}");
    }
}

/// TMDB, and a Recogniser enabled and listed in both of `media`'s Libraries that fails for the
/// title `failing`.
async fn failing_for(failing: &'static str) -> Fixture {
    let media = Fixture::new().await;
    let recogniser = FakePlugin(Some((Answers::FailsFor(failing), Asked::default())));
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
    let media = failing_for("Alien").await;
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
    let media = failing_for("Dark").await;
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
    let media = failing_for("Alien").await;
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

    let outdated = media.contents.outdated(media.movies.id).await.unwrap();
    assert!(outdated.is_empty(), "{outdated:?}");
    assert_eq!(
        media.outcomes(&media.movies).await,
        [Outcome::Undecided, matched(438_631, "Dune")]
    );
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
