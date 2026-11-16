use std::{collections::BTreeMap, fs, path::PathBuf, sync::Arc};

use futures_util::{FutureExt, StreamExt};
use nama_plugin_api::{
    PluginId,
    metadata::{ExternalId, ExternalIds, IMDB, TMDB},
    settings::{SettingValue, Settings},
};
use nama_plugin_tmdb::{
    Tmdb,
    testing::{API_KEY, FakeTmdb},
};
use nama_proto::nama::v1::{EventType, ItemMatched, subscribe_response};
use tempfile::TempDir;

use super::Matcher;
use crate::{
    db::Database,
    events::{EventBus, Filter, event},
    images::ImageStore,
    library::artwork::Artwork,
    library::contents::{Change, Contents, Decided, FileItem, Item, Recorded, Show},
    library::custom_images::CustomImages,
    library::libraries::{Kind, Libraries},
    plugins::{ExtensionPoints, Registry},
    scanner::Scanner,
    server_settings::ServerUrl,
};

/// A Movies Library and a Shows Library in scratch folders, a scanner, and a fake TMDB.
pub(super) struct Fixture {
    pub(super) scanner: Scanner,
    pub(super) matcher: Matcher,
    pub(super) contents: Contents,
    pub(super) libraries: Libraries,
    pub(super) plugins: Registry,
    events: EventBus,
    pub(super) tmdb: FakeTmdb,
    pub(super) movies: Library,
    pub(super) shows: Library,
    _media: TempDir,
    _data: TempDir,
}

pub(super) struct Library {
    pub(super) id: i64,
    root: PathBuf,
}

impl Library {
    pub(super) fn write(&self, relative: &str) {
        let path = self.root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"video").unwrap();
    }

    /// Replaces the file at `relative` with one of another size, as an upgrade would.
    pub(super) fn replace(&self, relative: &str) {
        fs::write(self.root.join(relative), b"a better copy of the video").unwrap();
    }
}

impl Fixture {
    pub(super) async fn new() -> Self {
        let fixture = Self::without_tmdb().await;
        fixture.enable_tmdb();
        fixture
    }

    pub(super) async fn without_tmdb() -> Self {
        let data = tempfile::tempdir().unwrap();
        let media = tempfile::tempdir().unwrap();
        let db = Arc::new(Database::open(data.path()).await.unwrap());
        let libraries = Libraries::new(Arc::clone(&db));
        let create = async |name: &str, kind| {
            let root = media.path().canonicalize().unwrap().join(name);
            fs::create_dir(&root).unwrap();
            let library = libraries
                .create(name, kind, vec![root.to_str().unwrap().to_owned()])
                .await
                .unwrap();
            Library {
                id: library.id,
                root,
            }
        };
        let movies = create("Movies", Kind::Movies).await;
        let shows = create("Shows", Kind::Shows).await;
        let plugins = Registry::new();
        let events = EventBus::new(
            Arc::clone(&db),
            ExtensionPoints::default(),
            ServerUrl::default(),
        );
        let images = ImageStore::new(data.path(), Arc::clone(&db));
        let scanner = Scanner::new(
            Arc::clone(&db),
            plugins.extension_points(),
            Artwork::new(images.clone(), plugins.extension_points()),
            CustomImages::new(Arc::clone(&db), images),
            events.clone(),
            None,
            None,
            None,
        );
        Self {
            matcher: scanner.matcher().clone(),
            scanner,
            contents: Contents::new(db),
            libraries,
            plugins,
            events,
            tmdb: FakeTmdb::start().await,
            movies,
            shows,
            _media: media,
            _data: data,
        }
    }

    pub(super) fn enable_tmdb(&self) {
        let settings = Settings::new(BTreeMap::from([(
            "api_key".to_owned(),
            SettingValue::Text(API_KEY.to_owned()),
        )]));
        self.plugins
            .enable_with_settings(
                Arc::new(Tmdb::with_endpoints(self.tmdb.endpoints())),
                settings,
            )
            .unwrap();
    }

    pub(super) async fn scan(&self, library: &Library) {
        self.scanner.scan(library.id).await.unwrap();
    }

    async fn items(&self, library: &Library) -> Vec<(String, FileItem)> {
        self.contents
            .files(library.id)
            .await
            .unwrap()
            .into_iter()
            .flat_map(|file| {
                let relative = library.relative(&file.path);
                file.items
                    .into_iter()
                    .map(move |item| (relative.clone(), item))
            })
            .collect()
    }

    /// What was stored for each item of `library`, as (path relative to its folder, Season
    /// and Episode, what it Matched).
    async fn stored(&self, library: &Library) -> Vec<(String, Option<(i64, i64)>, Outcome)> {
        self.items(library)
            .await
            .into_iter()
            .map(|(relative, stored)| {
                let episode = match stored.item {
                    Item::Episode {
                        season, episode, ..
                    } => Some((season, episode)),
                    Item::Movie { .. } => None,
                };
                (relative, episode, Outcome::of(stored.recorded.as_ref()))
            })
            .collect()
    }

    pub(super) async fn outcomes(&self, library: &Library) -> Vec<Outcome> {
        self.stored(library)
            .await
            .into_iter()
            .map(|(_, _, outcome)| outcome)
            .collect()
    }

    async fn first_recorded(&self, library: &Library) -> Recorded {
        self.items(library).await.remove(0).1.recorded.unwrap()
    }

    pub(super) async fn item(&self, library: &Library, relative: &str) -> i64 {
        self.items(library)
            .await
            .into_iter()
            .find(|(path, _)| path == relative)
            .unwrap()
            .1
            .id
    }

    /// The Still of each Episode Matched in `library`, by Season and Episode.
    pub(super) async fn stills(&self, library: &Library) -> Vec<Option<String>> {
        let shows = self.contents.shows(library.id).await.unwrap();
        (shows.iter().flat_map(Show::episodes))
            .map(|episode| episode.still.clone())
            .collect()
    }

    /// Enables TMDB with a key it rejects, so every fetch fails.
    pub(super) fn enable_tmdb_rejecting(&self) {
        let settings = Settings::new(BTreeMap::from([(
            "api_key".to_owned(),
            SettingValue::Text("rejected".to_owned()),
        )]));
        self.plugins
            .enable_with_settings(
                Arc::new(Tmdb::with_endpoints(self.tmdb.endpoints())),
                settings,
            )
            .unwrap();
    }

    pub(super) fn searches(&self) -> usize {
        self.tmdb
            .requests()
            .iter()
            .filter(|request| request.starts_with("/3/search/"))
            .count()
    }

    /// How many questions TMDB was asked about titles, leaving out the images a Match
    /// downloads in the background.
    pub(super) fn lookups(&self) -> usize {
        self.tmdb
            .requests()
            .iter()
            .filter(|request| !request.starts_with("/t/p/"))
            .count()
    }
}

impl Library {
    /// `path`, relative to the Library's folder, with `/` between its parts.
    fn relative(&self, path: &str) -> String {
        PathBuf::from(path)
            .strip_prefix(&self.root)
            .unwrap()
            .components()
            .map(|part| part.as_os_str().to_str().unwrap().to_owned())
            .collect::<Vec<_>>()
            .join("/")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Outcome {
    /// Matched to the title with this TMDB id and name.
    Matched(i64, String),
    Unmatched,
    Undecided,
}

impl Outcome {
    fn of(recorded: Option<&Recorded>) -> Self {
        match recorded {
            Some(recorded) if recorded.matched => Self::Matched(
                recorded.ids.get(TMDB).unwrap().value().parse().unwrap(),
                recorded.title.clone().unwrap(),
            ),
            Some(_) => Self::Unmatched,
            None => Self::Undecided,
        }
    }
}

pub(super) fn matched(id: i64, title: &str) -> Outcome {
    Outcome::Matched(id, title.to_owned())
}

#[tokio::test]
async fn a_scan_matches_movies_confidently_and_leaves_the_rest_unmatched() {
    let media = Fixture::new().await;
    let movies = &media.movies;
    movies.write("Alien (1979)/Alien (1979).mkv");
    movies.write("Dune (2021)/Dune (2021).mkv");
    // An ID hint is used directly: there is no recorded search for Heat.
    movies.write("Heat (1995) {tmdb-949}/Heat (1995).mkv");
    // Two films called Hamlet came out in 2000; one has over ten times the other's votes.
    movies.write("Hamlet (2000)/Hamlet (2000).mkv");
    // TMDB finds nothing.
    movies.write("Home Movies (2019)/Home Movies (2019).mkv");
    // The ID hint names no title on TMDB.
    movies.write("Lost Reel (1950) {tmdb-1}/Lost Reel (1950).mkv");
    // No title or year to search by: no item, so nothing to Match.
    movies.write("home video.mkv");

    media.scan(movies).await;

    assert_eq!(
        media.stored(movies).await,
        [
            (
                "Alien (1979)/Alien (1979).mkv".to_owned(),
                None,
                matched(348, "Alien")
            ),
            (
                "Dune (2021)/Dune (2021).mkv".to_owned(),
                None,
                matched(438_631, "Dune")
            ),
            (
                "Hamlet (2000)/Hamlet (2000).mkv".to_owned(),
                None,
                matched(10_688, "Hamlet")
            ),
            (
                "Heat (1995) {tmdb-949}/Heat (1995).mkv".to_owned(),
                None,
                matched(949, "Heat")
            ),
            (
                "Home Movies (2019)/Home Movies (2019).mkv".to_owned(),
                None,
                Outcome::Unmatched
            ),
            (
                "Lost Reel (1950) {tmdb-1}/Lost Reel (1950).mkv".to_owned(),
                None,
                Outcome::Unmatched
            ),
        ]
    );
    let alien = media.first_recorded(movies).await;
    assert_eq!(alien.plugin.as_deref(), Some("tmdb"));
    assert_eq!(
        alien.ids,
        ExternalIds::from_iter([
            ExternalId::tmdb(348),
            ExternalId::new(IMDB, "tt0078748").unwrap(),
        ])
    );
    assert_eq!(
        alien.poster.as_deref(),
        Some("/vfrQk5IPloGg1v9Rzbh2Eg3VGyM.jpg")
    );
}

#[tokio::test]
async fn tvdb_and_imdb_id_hints_match_without_a_search() {
    let media = Fixture::new().await;
    let (movies, shows) = (&media.movies, &media.shows);
    movies.write("Alien {imdb-tt0078748}/Alien.mkv");
    movies.write("Alien (1979) tt0078748/Alien (1979).mkv");
    // TMDB knows no title with this TVDB id.
    movies.write("Lost Reel (1950) [tvdbid-1]/Lost Reel (1950).mkv");
    shows.write("Dark [tvdbid-334824]/Season 01/Dark - S01E01.mkv");
    shows.write("Dark {tvdb-334824}/Dark - S01E02.mkv");
    // The IMDb id names a Movie, not a Show.
    shows.write("Alien (imdb=tt0078748)/Season 01/Alien - S01E01.mkv");

    media.scan(movies).await;
    media.scan(shows).await;

    assert_eq!(
        media.outcomes(movies).await,
        [
            matched(348, "Alien"),
            matched(348, "Alien"),
            Outcome::Unmatched
        ]
    );
    assert_eq!(
        media.outcomes(shows).await,
        [
            Outcome::Unmatched,
            matched(1_388_538, "Secrets"),
            matched(1_388_539, "Lies"),
        ]
    );
    assert_eq!(media.searches(), 0, "{:?}", media.tmdb.requests());
}

#[tokio::test]
async fn a_newer_name_parser_matches_an_item_by_its_tvdb_id_hint() {
    let media = Fixture::new().await;
    let shows = &media.shows;
    let pilot = "Dark [tvdbid-334824]/Season 01/Dark - S01E01.mkv";
    shows.write(pilot);
    media.scan(shows).await;
    // Before TVDB ids were read, the folder named a Show with no year and no ID Hint, which
    // a search left Unmatched.
    let (path, file) = media
        .contents
        .stored(shows.id)
        .await
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    let old_name = Item::Episode {
        show: "Dark".to_owned(),
        year: None,
        hint: None,
        season: 1,
        episode: 1,
    };
    let reparse = Change::Reparse {
        file: file.id,
        path,
        items: vec![old_name],
    };
    media.contents.apply(shows.id, vec![reparse]).await.unwrap();
    let item = media.item(shows, pilot).await;
    media
        .contents
        .record(vec![(item, Decided::Unmatched(None))])
        .await
        .unwrap();
    assert_eq!(media.items(shows).await[0].1.item.name().hint, None);

    media.contents.read_by_older_parser(shows.id).await;
    media.scan(shows).await;

    assert_eq!(media.outcomes(shows).await, [matched(1_388_538, "Secrets")]);
}

#[tokio::test]
async fn a_match_is_published_once_stored() {
    let media = Fixture::new().await;
    let movies = &media.movies;
    movies.write("Alien (1979)/Alien (1979).mkv");
    // Unmatched, which publishes nothing.
    movies.write("Home Movies (2019)/Home Movies (2019).mkv");
    let filter = Filter {
        types: vec![EventType::EVENT_TYPE_ITEM_MATCHED],
        ..Filter::default()
    };
    let mut matches = Box::pin(media.events.subscribe_as_admin(filter));

    media.scan(movies).await;

    let alien = media.item(movies, "Alien (1979)/Alien (1979).mkv").await;
    let expected = event(ItemMatched {
        library_id: movies.id,
        item_id: alien,
        title: "Alien".to_owned(),
        ..Default::default()
    });
    assert_eq!(
        matches.next().await.and_then(|response| response.message),
        Some(subscribe_response::Message::from(expected))
    );
    assert!(
        matches.next().now_or_never().is_none(),
        "an Unmatched item was published"
    );
}

#[tokio::test]
async fn a_scan_matches_episodes_through_their_show_and_season() {
    let media = Fixture::new().await;
    let shows = &media.shows;
    shows.write("Dark (2017)/Season 01/Dark (2017) - S01E01-E02.mkv");
    shows.write("Dark (2017)/Season 01/Dark (2017) - S01E03.mkv");
    // Season 1 has no Episode 42.
    shows.write("Dark (2017)/Season 01/Dark (2017) - S01E42.mkv");
    // No year, and both the US and the UK Office fit.
    shows.write("The Office/Season 01/The Office - S01E01.mkv");

    media.scan(shows).await;

    let dark = "Dark (2017)/Season 01/Dark (2017)";
    assert_eq!(
        media.stored(shows).await,
        [
            (
                format!("{dark} - S01E01-E02.mkv"),
                Some((1, 1)),
                matched(1_388_538, "Secrets")
            ),
            (
                format!("{dark} - S01E01-E02.mkv"),
                Some((1, 2)),
                matched(1_388_539, "Lies")
            ),
            (
                format!("{dark} - S01E03.mkv"),
                Some((1, 3)),
                matched(1_388_540, "Past and Present")
            ),
            (
                format!("{dark} - S01E42.mkv"),
                Some((1, 42)),
                Outcome::Unmatched
            ),
            (
                "The Office/Season 01/The Office - S01E01.mkv".to_owned(),
                Some((1, 1)),
                Outcome::Unmatched
            ),
        ]
    );
    let secrets = media.first_recorded(shows).await;
    assert_eq!(secrets.show_title.as_deref(), Some("Dark"));
    assert_eq!(
        secrets.show_ids,
        ExternalIds::from_iter([
            ExternalId::tmdb(70523),
            ExternalId::new(IMDB, "tt5753856").unwrap(),
            ExternalId::tvdb(334_824),
        ])
    );
    // The Show and its Season were each fetched once.
    let requests = media.tmdb.requests();
    let count = |prefix: &str| requests.iter().filter(|r| r.starts_with(prefix)).count();
    assert_eq!(count("/3/tv/70523?"), 1, "{requests:?}");
    assert_eq!(count("/3/tv/70523/season/1?"), 1, "{requests:?}");
}

#[tokio::test]
async fn a_country_tag_on_a_show_is_searched_without_and_keeps_shows_from_that_country() {
    let media = Fixture::new().await;
    let shows = &media.shows;
    // TMDB also has a Euphoria with no country, which is dropped.
    shows.write("Euphoria (US)/Season 01/Euphoria (US) - S01E01.mkv");
    // TheTVDB's UK is TMDB's GB.
    shows.write("The Office (UK)/Season 01/The Office (UK) - S01E01.mkv");
    shows.write("The Office (US)/Season 01/The Office (US) - S01E01.mkv");

    media.scan(shows).await;

    assert_eq!(
        media.outcomes(shows).await,
        [
            matched(1_734_395, "Pilot"),
            matched(2_996_001, "Downsize"),
            matched(2_316_001, "Pilot"),
        ]
    );
}

#[tokio::test]
async fn a_show_left_unmatched_by_the_parser_before_country_tags_matches_once_rescanned() {
    let media = Fixture::without_tmdb().await;
    let shows = &media.shows;
    let pilot = "Euphoria (US)/Season 01/Euphoria (US) - S01E01.mkv";
    shows.write(pilot);
    media.scan(shows).await;
    let item = media.item(shows, pilot).await;
    media
        .contents
        .record(vec![(item, Decided::Unmatched(None))])
        .await
        .unwrap();
    media.contents.read_by_older_parser(shows.id).await;
    media.enable_tmdb();

    media.scan(shows).await;

    assert_eq!(media.outcomes(shows).await, [matched(1_734_395, "Pilot")]);
}

#[tokio::test]
async fn a_bracket_that_is_not_a_country_stays_in_the_shows_title() {
    let media = Fixture::new().await;
    let shows = &media.shows;
    shows.write("The Office (II)/Season 01/The Office (II) - S01E01.mkv");

    media.scan(shows).await;

    let requests = media.tmdb.requests();
    assert!(
        requests
            .iter()
            .any(|r| r.starts_with("/3/search/tv?") && r.contains("query=The+Office+%28II%29")),
        "{requests:?}"
    );
}

#[tokio::test]
async fn a_country_tag_in_a_movies_name_stays_in_its_title() {
    let media = Fixture::new().await;
    let movies = &media.movies;
    movies.write("Euphoria (US) (2019)/Euphoria (US) (2019).mkv");

    media.scan(movies).await;

    let requests = media.tmdb.requests();
    assert!(
        requests
            .iter()
            .any(|r| r.starts_with("/3/search/movie?") && r.contains("query=Euphoria+%28US%29")),
        "{requests:?}"
    );
}

#[tokio::test]
async fn a_pin_on_a_show_named_with_a_country_tag_still_applies_after_a_newer_parser() {
    let media = Fixture::new().await;
    let shows = &media.shows;
    let pilot = "The Office (US)/Season 01/The Office (US) - S01E01.mkv";
    shows.write(pilot);
    media.scan(shows).await;
    let item = media.item(shows, pilot).await;
    media
        .matcher
        .set_match(item, &ExternalId::tmdb(2996))
        .await
        .unwrap();
    let searches = media.searches();

    media.contents.read_by_older_parser(shows.id).await;
    media.scan(shows).await;

    assert_eq!(
        media.outcomes(shows).await,
        [matched(2_996_001, "Downsize")]
    );
    assert_eq!(media.searches(), searches);
}

#[tokio::test]
async fn a_year_off_by_one_still_matches_though_tmdb_filters_by_exact_year() {
    let media = Fixture::new().await;
    // Released in 1979 in the US, but 1980 where this copy came from. TMDB finds no Alien
    // from 1980, so the search is tried again without the year.
    media.movies.write("Alien (1980)/Alien (1980).mkv");

    media.scan(&media.movies).await;

    assert_eq!(
        media.stored(&media.movies).await[0].2,
        matched(348, "Alien")
    );
}

#[tokio::test]
async fn a_failing_metadata_provider_stops_the_pass_and_leaves_items_undecided() {
    let media = Fixture::without_tmdb().await;
    let settings = Settings::new(BTreeMap::from([(
        "api_key".to_owned(),
        SettingValue::Text("rejected".to_owned()),
    )]));
    media
        .plugins
        .enable_with_settings(
            Arc::new(Tmdb::with_endpoints(media.tmdb.endpoints())),
            settings,
        )
        .unwrap();
    media.movies.write("Alien (1979)/Alien (1979).mkv");
    media.movies.write("Dune (2021)/Dune (2021).mkv");

    media.scan(&media.movies).await;

    // TMDB rejected the key once; nothing more was asked.
    assert_eq!(media.tmdb.requests().len(), 1);
    assert_eq!(
        media
            .stored(&media.movies)
            .await
            .into_iter()
            .map(|(_, _, outcome)| outcome)
            .collect::<Vec<_>>(),
        [Outcome::Undecided, Outcome::Undecided]
    );
}

#[tokio::test]
async fn a_rescan_does_not_look_up_items_already_decided() {
    let media = Fixture::new().await;
    media.movies.write("Alien (1979)/Alien (1979).mkv");
    media
        .movies
        .write("Home Movies (2019)/Home Movies (2019).mkv");
    media.scan(&media.movies).await;
    let looked_up = media.lookups();

    media.scan(&media.movies).await;

    assert_eq!(media.lookups(), looked_up);
    assert_eq!(
        media
            .stored(&media.movies)
            .await
            .into_iter()
            .map(|(_, _, outcome)| outcome)
            .collect::<Vec<_>>(),
        [matched(348, "Alien"), Outcome::Unmatched]
    );
}

#[tokio::test]
async fn a_newer_name_parser_retries_unmatched_items_and_keeps_automatic_matches() {
    let media = Fixture::new().await;
    media.movies.write("Alien (1979)/Alien (1979).mkv");
    media
        .movies
        .write("Home Movies (2019)/Home Movies (2019).mkv");
    media.scan(&media.movies).await;
    let alien = media
        .item(&media.movies, "Alien (1979)/Alien (1979).mkv")
        .await;
    let searches = media.searches();

    media.contents.read_by_older_parser(media.movies.id).await;
    media.scan(&media.movies).await;

    // Only Home Movies, left Unmatched, was searched for again: with its year, then without.
    assert_eq!(media.searches(), searches + 2);
    assert_eq!(
        media.outcomes(&media.movies).await,
        [matched(348, "Alien"), Outcome::Unmatched]
    );
    assert_eq!(
        media
            .item(&media.movies, "Alien (1979)/Alien (1979).mkv")
            .await,
        alien
    );
}

#[tokio::test]
async fn a_newer_name_parser_matches_an_item_an_older_one_left_ambiguous() {
    let media = Fixture::new().await;
    let movies = &media.movies;
    let hamlet = "Hamlet (2000)/Hamlet (2000).mkv";
    movies.write(hamlet);
    media.scan(movies).await;
    // Before votes broke a tie, the two Hamlets of 2000 left this file Unmatched.
    let item = media.item(movies, hamlet).await;
    media
        .contents
        .record(vec![(item, Decided::Unmatched(None))])
        .await
        .unwrap();

    media.contents.read_by_older_parser(movies.id).await;
    media.scan(movies).await;

    assert_eq!(media.outcomes(movies).await, [matched(10_688, "Hamlet")]);
}

#[tokio::test]
async fn a_newer_name_parser_keeps_a_pinned_match() {
    let media = Fixture::new().await;
    let movies = &media.movies;
    let home_movies = "Home Movies (2019)/Home Movies (2019).mkv";
    movies.write(home_movies);
    media.scan(movies).await;
    let item = media.item(movies, home_movies).await;
    media
        .matcher
        .set_match(item, &ExternalId::tmdb(949))
        .await
        .unwrap();
    let looked_up = media.lookups();

    media.contents.read_by_older_parser(movies.id).await;
    media.scan(movies).await;

    assert_eq!(media.outcomes(movies).await, [matched(949, "Heat")]);
    assert_eq!(media.lookups(), looked_up);
}

#[tokio::test]
async fn items_stay_undecided_until_a_metadata_provider_can_match_them() {
    let media = Fixture::without_tmdb().await;
    media.movies.write("Alien (1979)/Alien (1979).mkv");

    media.scan(&media.movies).await;
    let before = media.stored(&media.movies).await;
    media.enable_tmdb();
    media.scan(&media.movies).await;

    assert_eq!(before[0].2, Outcome::Undecided);
    assert_eq!(
        media.stored(&media.movies).await[0].2,
        matched(348, "Alien")
    );
    assert!(
        media
            .plugins
            .enabled()
            .contains(&PluginId::from_static("tmdb"))
    );
}
