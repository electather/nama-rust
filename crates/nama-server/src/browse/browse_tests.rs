use std::{sync::Arc, time::Duration};

use nama_plugin_api::metadata::{ExternalId, ExternalIds};
use nama_proto::nama::v1::PageRequest;
use tempfile::TempDir;

use super::{Browse, Error, Started, Target};
use crate::{
    db::{self, Database},
    library::{
        Change, Contents, Decided, Item, Libraries, LibraryKind, Listing, Metadata, MovieOrEpisode,
        NotFound, Progress, Stored, TitleSort, WatchState, WatchStates,
        fixture::{MatchedMovie, decide, tmdb_movie},
    },
    playback::{Probe, StoredTracks, Track, TrackKind, Tracks},
    plugins::{ExtensionPoints, Registry},
    users::{Permissions, User, Users},
};

const ALIEN_TMDB_ID: i64 = 348;
pub(super) const KNIGHT_TMDB_ID: i64 = 224_372;
const KNIGHT_PILOT_TMDB_ID: i64 = 5_000_001;
const KNIGHT_SECOND_TMDB_ID: i64 = 5_000_002;
const KNIGHT_SEASON_2_TMDB_ID: i64 = 5_000_003;
const DARK_TMDB_ID: i64 = 70_523;
const DARK_PILOT_TMDB_ID: i64 = 6_000_001;
const KNIGHT_PILOT_TVDB_ID: i64 = 9_000_001;
/// Alien's IMDb id, which its Match gives it besides its TMDB id.
const ALIEN_IMDB_ID: &str = "tt0078748";

/// Creates a Library of `kind` on `folder`, with its canonical path.
pub(super) async fn library(
    db: &Arc<Database>,
    name: &str,
    kind: LibraryKind,
    folder: &TempDir,
) -> (i64, String) {
    let root = folder.path().canonicalize().unwrap();
    let root = root.to_str().unwrap().to_owned();
    let library = Libraries::new(Arc::clone(db))
        .create(name, kind, vec![root.clone()])
        .await
        .unwrap()
        .id;
    (library, root)
}

/// Stores the file at `path` in `library`, holding each Item of `items` with what was decided
/// about it, and returns its id, the id of its Version.
pub(super) async fn store(
    db: &Arc<Database>,
    library: i64,
    path: String,
    items: Vec<(Item, Decided)>,
) -> i64 {
    let contents = Contents::new(Arc::clone(db));
    let (items, decided): (Vec<_>, Vec<_>) = items.into_iter().unzip();
    let file = Change::Store {
        edition: None,
        path: path.clone(),
        size: 10,
        modified_ns: 0,
        items,
    };
    contents.apply(library, vec![file]).await.unwrap();
    let (version, _items) = decide(&contents, library, &path, decided).await;
    version
}

/// Stores the file at `path` in `library`, holding `item`, and Matches the Item to `matched`.
async fn store_matched(
    db: &Arc<Database>,
    library: i64,
    path: String,
    item: Item,
    matched: Metadata,
) -> i64 {
    let matched = Decided::Matched(Box::new(matched));
    store(db, library, path, vec![(item, matched)]).await
}

/// Ada, a User of `db`.
pub(super) async fn ada(db: &Arc<Database>) -> i64 {
    let root = User {
        id: 0,
        name: "Root".to_owned(),
        permissions: Permissions::ADMIN,
        created_at: 0,
    };
    let plugins = Registry::new();
    plugins
        .enable(Arc::new(nama_plugin_local_auth::LocalAuth))
        .unwrap();
    Users::new(Arc::clone(db), plugins.extension_points())
        .create(&root, "Ada", "correct horse battery staple")
        .await
        .unwrap()
}

/// A Movies Library on `folder` with one file, Matched to Alien.
async fn library_with_alien(db: &Arc<Database>, folder: &TempDir) -> MatchedMovie {
    MatchedMovie::new(db, folder.path(), tmdb_movie(ALIEN_TMDB_ID, "Alien")).await
}

pub(super) fn knight_episode(season: i64, episode: i64) -> Item {
    Item::Episode {
        show: "A Knight of the Seven Kingdoms".to_owned(),
        year: Some(2026),
        hint: None,
        season,
        episode,
    }
}

/// A Match of an Episode of A Knight of the Seven Kingdoms with the TMDB id `tmdb`.
pub(super) fn knight_match(tmdb: i64) -> Decided {
    Decided::Matched(Box::new(Metadata {
        show_ids: ExternalIds::from_iter([ExternalId::tmdb(KNIGHT_TMDB_ID)]),
        show_title: Some("A Knight of the Seven Kingdoms".to_owned()),
        ..tmdb_movie(tmdb, "An Episode")
    }))
}

/// Creates a Shows Library holding A Knight of the Seven Kingdoms' Season 1 Episodes 1 and 2,
/// in one file, and Season 2 Episode 1, and returns the Library and the first file's id.
async fn library_with_knight(db: &Arc<Database>, folder: &TempDir) -> (i64, i64) {
    let (library, root) = library(db, "Shows", LibraryKind::Shows, folder).await;
    let show = format!("{root}/A Knight of the Seven Kingdoms");
    let both = vec![
        (knight_episode(1, 1), knight_match(KNIGHT_PILOT_TMDB_ID)),
        (knight_episode(1, 2), knight_match(KNIGHT_SECOND_TMDB_ID)),
    ];
    let two_episodes = store(
        db,
        library,
        format!("{show}/Season 01/S01E01-E02.mkv"),
        both,
    )
    .await;
    let path = format!("{show}/Season 02/S02E01.mkv");
    let second_season = vec![(knight_episode(2, 1), knight_match(KNIGHT_SEASON_2_TMDB_ID))];
    store(db, library, path, second_season).await;
    (library, two_episodes)
}

/// How far the User `user` has got through each Episode of A Knight of the Seven Kingdoms in
/// `library`, by Season, and whether they've played it: `None` for one they haven't started.
async fn knight_states(browse: &Browse, user: i64, library: i64) -> Vec<Vec<Option<(i64, bool)>>> {
    let mut states = Vec::new();
    for number in [1, 2] {
        let episodes = browse
            .episodes(
                user,
                library,
                &ExternalId::tmdb(KNIGHT_TMDB_ID),
                Some(number),
            )
            .await
            .unwrap();
        let season = episodes
            .into_iter()
            .map(|(_, state)| state.map(|state| (state.position_ms, state.played)));
        states.push(season.collect());
    }
    states
}

async fn only_file(db: &Arc<Database>, library: i64) -> Stored {
    let stored = Contents::new(Arc::clone(db)).stored(library).await.unwrap();
    *stored.values().next().unwrap()
}

fn stereo() -> Track {
    Track {
        codec: "ac3".to_owned(),
        language: None,
        title: None,
        default: false,
        forced: false,
        sdh: false,
        kind: TrackKind::Audio {
            channels: Some(2),
            atmos: false,
            audio_description: false,
        },
    }
}

/// A stereo Track whose language is `language`, so one file's Tracks differ from another's.
fn stereo_in(language: &str) -> Track {
    Track {
        language: Some(language.to_owned()),
        ..stereo()
    }
}

fn probe_of(tracks: Vec<Track>) -> Probe {
    Probe {
        container: "matroska,webm".to_owned(),
        duration_ms: None,
        bitrate: None,
        tracks,
        font_attachments: vec![],
        chapters: vec![],
    }
}

#[tokio::test]
async fn an_episode_has_the_same_tracks_however_it_is_reached() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let (library, root) = library(&db, "Shows", LibraryKind::Shows, &folder).await;
    let pilot_file = Item::Episode {
        show: "A Knight of the Seven Kingdoms".to_owned(),
        year: Some(2026),
        hint: None,
        season: 1,
        episode: 1,
    };
    let matched = Metadata {
        show_ids: ExternalIds::from_iter([ExternalId::tmdb(KNIGHT_TMDB_ID)]),
        show_title: Some("A Knight of the Seven Kingdoms".to_owned()),
        ..tmdb_movie(KNIGHT_PILOT_TMDB_ID, "The Hedge Knight")
    };
    let path = format!("{root}/A Knight of the Seven Kingdoms/Season 01/S01E01.mkv");
    let version = store_matched(&db, library, path, pilot_file, matched).await;
    let file = only_file(&db, library).await;
    Tracks::new(Arc::clone(&db))
        .store(file, probe_of(vec![stereo()]))
        .await
        .unwrap();
    let user = ada(&db).await;
    let browse = Browse::new(db, ExtensionPoints::default());
    let halfway = Progress::new(1_000, 2_000).unwrap();
    browse.report(user, version, halfway).await.unwrap();
    let (show, pilot) = (
        ExternalId::tmdb(KNIGHT_TMDB_ID),
        ExternalId::tmdb(KNIGHT_PILOT_TMDB_ID),
    );

    let (listed, _) = browse
        .episodes(0, library, &show, Some(1))
        .await
        .unwrap()
        .remove(0);
    let in_season = browse
        .season(library, &show, 1)
        .await
        .unwrap()
        .episodes
        .remove(0);
    let Started::Episode {
        episode: started, ..
    } = browse
        .continue_watching(user, &PageRequest::default())
        .await
        .unwrap()
        .0
        .remove(0)
    else {
        panic!("the pilot is an Episode");
    };

    let tracks = [listed, in_season, started].map(|episode| {
        assert_eq!(episode.id, pilot);
        episode.versions[0].tracks.clone()
    });
    assert_eq!(tracks, [[stereo()], [stereo()], [stereo()]]);
}

#[tokio::test]
async fn a_seasons_episodes_have_their_own_tracks_beside_another_shows() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let (library, root) = library(&db, "Shows", LibraryKind::Shows, &folder).await;
    let knight = format!("{root}/A Knight of the Seven Kingdoms/Season 01");
    let pilot = (knight_episode(1, 1), knight_match(KNIGHT_PILOT_TMDB_ID));
    store(&db, library, format!("{knight}/S01E01.mkv"), vec![pilot]).await;
    let second = (knight_episode(1, 2), knight_match(KNIGHT_SECOND_TMDB_ID));
    store(&db, library, format!("{knight}/S01E02.mkv"), vec![second]).await;
    let dark_pilot = Item::Episode {
        show: "Dark".to_owned(),
        year: Some(2017),
        hint: None,
        season: 1,
        episode: 1,
    };
    let dark = Metadata {
        show_ids: ExternalIds::from_iter([ExternalId::tmdb(DARK_TMDB_ID)]),
        show_title: Some("Dark".to_owned()),
        ..tmdb_movie(DARK_PILOT_TMDB_ID, "Secrets")
    };
    let dark_path = format!("{root}/Dark/Season 01/S01E01.mkv");
    store_matched(&db, library, dark_path, dark_pilot, dark).await;
    let stored = Contents::new(Arc::clone(&db))
        .stored(library)
        .await
        .unwrap();
    // Each file's Tracks name its path, so an Episode given another file's shows it.
    for (path, file) in stored {
        Tracks::new(Arc::clone(&db))
            .store(file, probe_of(vec![stereo_in(&path)]))
            .await
            .unwrap();
    }
    let browse = Browse::new(db, ExtensionPoints::default());

    let episodes = browse
        .episodes(0, library, &ExternalId::tmdb(KNIGHT_TMDB_ID), Some(1))
        .await
        .unwrap();

    let tracks: Vec<_> = (episodes.into_iter())
        .map(|(episode, _)| (episode.number, episode.versions[0].tracks.clone()))
        .collect();
    let of_file = |name: &str| vec![stereo_in(&format!("{knight}/{name}"))];
    assert_eq!(
        tracks,
        [(1, of_file("S01E01.mkv")), (2, of_file("S01E02.mkv"))]
    );
}

#[tokio::test]
async fn a_movie_matched_in_two_libraries_is_listed_in_continue_watching_under_the_older_one() {
    let data = tempfile::tempdir().unwrap();
    let (older_folder, newer_folder) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let older = library_with_alien(&db, &older_folder).await;
    library_with_alien(&db, &newer_folder).await;
    let user = ada(&db).await;
    let browse = Browse::new(db, ExtensionPoints::default());
    let halfway = Progress::new(1_000, 2_000).unwrap();
    browse.report(user, older.version, halfway).await.unwrap();

    let (started, _) = browse
        .continue_watching(user, &PageRequest::default())
        .await
        .unwrap();

    let listed: Vec<_> = started
        .iter()
        .map(|started| match started {
            Started::Movie { library, movie, .. } => (*library, movie.id.clone()),
            Started::Episode { .. } => panic!("Alien is a Movie"),
        })
        .collect();
    assert_eq!(listed, [(older.library, ExternalId::tmdb(ALIEN_TMDB_ID))]);
}

#[tokio::test]
async fn continue_watching_reads_no_library_for_a_user_who_started_nothing() {
    let (data, unreadable_data) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let unreadable = Arc::new(Database::open(unreadable_data.path()).await.unwrap());
    unreadable.reader().close().await;
    let browse = Browse {
        libraries: Libraries::new(Arc::clone(&unreadable)),
        contents: Contents::new(Arc::clone(&unreadable)),
        tracks: StoredTracks::new(unreadable),
        watch_states: WatchStates::new(db, ExtensionPoints::default()),
    };

    let (started, _) = browse
        .continue_watching(1, &PageRequest::default())
        .await
        .unwrap();

    assert!(started.is_empty(), "{started:?}");
}

#[tokio::test]
async fn a_version_has_no_tracks_read_from_its_file_before_it_changed() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let library = library_with_alien(&db, &folder).await.library;
    let file = only_file(&db, library).await;
    let before_it_changed = Stored {
        modified_ns: file.modified_ns - 1,
        ..file
    };
    Tracks::new(Arc::clone(&db))
        .store(before_it_changed, probe_of(vec![stereo()]))
        .await
        .unwrap();

    let (movie, _, _) = Browse::new(db, ExtensionPoints::default())
        .movie(0, library, &ExternalId::tmdb(ALIEN_TMDB_ID))
        .await
        .unwrap();

    assert_eq!(movie.versions[0].tracks, []);
}

#[tokio::test]
async fn progress_on_a_movies_version_is_progress_through_the_movie() {
    let (data, folder) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let MatchedMovie {
        library,
        version: alien,
        ..
    } = library_with_alien(&db, &folder).await;
    let user = ada(&db).await;
    let browse = Browse::new(db, ExtensionPoints::default());

    let halfway = Progress::new(1_000, 2_000).unwrap();
    browse.report(user, alien, halfway).await.unwrap();

    let (_, state, _) = browse
        .movie(user, library, &ExternalId::tmdb(ALIEN_TMDB_ID))
        .await
        .unwrap();
    let state = state.unwrap();
    assert_eq!((state.position_ms, state.duration_ms), (1_000, 2_000));
}

#[tokio::test]
async fn progress_on_a_file_holding_two_episodes_is_split_across_them() {
    let (data, folder) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let (library, two_episodes) = library_with_knight(&db, &folder).await;
    let user = ada(&db).await;
    let browse = Browse::new(db, ExtensionPoints::default());

    let three_quarters = Progress::new(1_500, 2_000).unwrap();
    browse
        .report(user, two_episodes, three_quarters)
        .await
        .unwrap();

    assert_eq!(
        knight_states(&browse, user, library).await,
        [vec![Some((1_000, true)), Some((500, false))], vec![None]]
    );
}

#[tokio::test]
async fn progress_on_an_unmatched_item_stores_nothing() {
    let (data, folder) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let (library, root) = library(&db, "Movies", LibraryKind::Movies, &folder).await;
    let unknown = Item::Movie {
        title: "Unknown".to_owned(),
        year: None,
        hint: None,
    };
    let unmatched = vec![(unknown, Decided::Unmatched(None))];
    let file = store(&db, library, format!("{root}/Unknown.mkv"), unmatched).await;
    let user = ada(&db).await;

    let halfway = Progress::new(1_000, 2_000).unwrap();
    Browse::new(Arc::clone(&db), ExtensionPoints::default())
        .report(user, file, halfway)
        .await
        .unwrap();

    // Watch States' own list holds every one of the User's, in a Library or not.
    let states = WatchStates::new(db, ExtensionPoints::default())
        .of_user(user)
        .await
        .unwrap();
    assert!(states.is_empty(), "{states:?}");
}

#[tokio::test]
async fn progress_on_a_version_that_doesnt_exist_is_refused() {
    let data = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let user = ada(&db).await;

    let halfway = Progress::new(1_000, 2_000).unwrap();
    let reported = Browse::new(db, ExtensionPoints::default())
        .report(user, 1, halfway)
        .await;

    assert!(
        matches!(reported, Err(Error::NotFound(NotFound::NoSuchVersion))),
        "{reported:?}"
    );
}

#[tokio::test]
async fn a_movie_marked_played_stays_played_until_marked_unplayed() {
    let (data, folder) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let library = library_with_alien(&db, &folder).await.library;
    let user = ada(&db).await;
    let browse = Browse::new(db, ExtensionPoints::default());
    let alien = ExternalId::tmdb(ALIEN_TMDB_ID);
    let played = async || {
        let (_, state, _) = browse.movie(user, library, &alien).await.unwrap();
        state.map(|state| state.played)
    };

    let target = || Target::Movie(alien.clone());
    browse.mark(user, library, target(), true).await.unwrap();
    assert_eq!(played().await, Some(true));

    browse.mark(user, library, target(), false).await.unwrap();
    assert_eq!(played().await, None);
}

#[tokio::test]
async fn marking_an_episode_a_season_or_a_show_marks_each_of_its_episodes() {
    let (data, folder) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let (library, _) = library_with_knight(&db, &folder).await;
    let user = ada(&db).await;
    let browse = Browse::new(db, ExtensionPoints::default());
    let show = ExternalId::tmdb(KNIGHT_TMDB_ID);
    let played = Some((0, true));

    let pilot = Target::Episode(ExternalId::tmdb(KNIGHT_PILOT_TMDB_ID));
    browse.mark(user, library, pilot, true).await.unwrap();
    assert_eq!(
        knight_states(&browse, user, library).await,
        [vec![played, None], vec![None]]
    );

    let first_season = Target::Season {
        show: show.clone(),
        number: 1,
    };
    browse
        .mark(user, library, first_season, true)
        .await
        .unwrap();
    assert_eq!(
        knight_states(&browse, user, library).await,
        [vec![played, played], vec![None]]
    );

    browse
        .mark(user, library, Target::Show(show.clone()), true)
        .await
        .unwrap();
    assert_eq!(
        knight_states(&browse, user, library).await,
        [vec![played, played], vec![played]]
    );

    browse
        .mark(user, library, Target::Show(show), false)
        .await
        .unwrap();
    assert_eq!(
        knight_states(&browse, user, library).await,
        [vec![None, None], vec![None]]
    );
}

#[tokio::test]
async fn marking_a_title_the_library_doesnt_have_is_refused_as_browsing_to_it_is() {
    let (data, folder) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let (library, _) = library_with_knight(&db, &folder).await;
    let user = ada(&db).await;
    let browse = Browse::new(db, ExtensionPoints::default());
    let (show, elsewhere) = (ExternalId::tmdb(KNIGHT_TMDB_ID), ExternalId::tmdb(1));

    let marks = [
        Target::Episode(elsewhere.clone()),
        Target::Show(elsewhere),
        Target::Season { show, number: 3 },
        Target::Movie(ExternalId::tmdb(ALIEN_TMDB_ID)),
    ];
    let mut refused = Vec::new();
    for target in marks {
        match browse.mark(user, library, target, true).await {
            Err(err) => refused.push(err.to_string()),
            Ok(()) => panic!("marked a title the Library doesn't have"),
        }
    }

    assert_eq!(
        refused,
        [
            NotFound::NothingMatched.to_string(),
            NotFound::NothingMatched.to_string(),
            NotFound::NoSuchSeason.to_string(),
            NotFound::WrongKind {
                wanted: LibraryKind::Movies
            }
            .to_string(),
        ]
    );
    assert_eq!(
        knight_states(&browse, user, library).await,
        [vec![None, None], vec![None]]
    );
}

fn alien_imdb() -> ExternalId {
    ExternalId::new("imdb", ALIEN_IMDB_ID).unwrap()
}

/// A Movies Library on `folder` with one file, Matched to Alien by its TMDB id, which its Library
/// prefers, and its IMDb id.
async fn library_with_alien_by_two_ids(db: &Arc<Database>, folder: &TempDir) -> i64 {
    let alien = Metadata {
        ids: ExternalIds::from_iter([ExternalId::tmdb(ALIEN_TMDB_ID), alien_imdb()]),
        ..tmdb_movie(ALIEN_TMDB_ID, "Alien")
    };
    MatchedMovie::new(db, folder.path(), alien).await.library
}

/// Records that the User `user` has got as far as `progress` through `title`, under the id it
/// names, as Browse would under a Library's preferred id before its title source order changed.
async fn report_under(db: &Arc<Database>, user: i64, title: MovieOrEpisode, progress: Progress) {
    (WatchStates::new(Arc::clone(db), ExtensionPoints::default()).report(
        user,
        vec![Some(title)],
        progress,
    ))
    .await
    .unwrap();
}

fn position(state: Option<WatchState>) -> Option<(i64, bool)> {
    state.map(|state| (state.position_ms, state.played))
}

const BY_TITLE: Listing = Listing {
    genres: Vec::new(),
    min_year: None,
    max_year: None,
    watched: None,
    sort: TitleSort::Title,
    descending: false,
};

#[path = "paging_tests.rs"]
mod paging;

#[tokio::test]
async fn a_movies_watch_state_under_an_id_its_library_doesnt_prefer_is_shown() {
    let (data, folder) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let library = library_with_alien_by_two_ids(&db, &folder).await;
    let user = ada(&db).await;
    let halfway = Progress::new(1_000, 2_000).unwrap();
    report_under(&db, user, MovieOrEpisode::movie(&alien_imdb()), halfway).await;
    let browse = Browse::new(db, ExtensionPoints::default());

    let (movie, state, _) = (browse.movie(user, library, &ExternalId::tmdb(ALIEN_TMDB_ID)))
        .await
        .unwrap();
    let (listed, _) = (browse.movie_page(user, library, &BY_TITLE, &PageRequest::default()))
        .await
        .unwrap();

    assert_eq!(movie.id, ExternalId::tmdb(ALIEN_TMDB_ID));
    assert_eq!(position(state), Some((1_000, false)));
    let listed: Vec<_> = (listed.into_iter())
        .map(|(listed, listed_state)| (listed.id, position(listed_state)))
        .collect();
    assert_eq!(
        listed,
        [(ExternalId::tmdb(ALIEN_TMDB_ID), Some((1_000, false)))]
    );
}

#[tokio::test]
async fn an_episodes_watch_state_under_an_id_its_library_doesnt_prefer_is_shown() {
    let (data, folder) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let (library, root) = library(&db, "Shows", LibraryKind::Shows, &folder).await;
    let tvdb = ExternalId::tvdb(KNIGHT_PILOT_TVDB_ID);
    let Decided::Matched(mut pilot) = knight_match(KNIGHT_PILOT_TMDB_ID) else {
        panic!("an Episode's Match");
    };
    pilot.ids.insert(tvdb.clone());
    let path = format!("{root}/A Knight of the Seven Kingdoms/Season 01/S01E01.mkv");
    store_matched(&db, library, path, knight_episode(1, 1), *pilot).await;
    let user = ada(&db).await;
    let halfway = Progress::new(1_000, 2_000).unwrap();
    report_under(&db, user, MovieOrEpisode::episode(&tvdb), halfway).await;
    let browse = Browse::new(db, ExtensionPoints::default());

    let episodes = (browse.episodes(user, library, &ExternalId::tmdb(KNIGHT_TMDB_ID), Some(1)))
        .await
        .unwrap();

    let listed: Vec<_> = (episodes.into_iter())
        .map(|(episode, state)| (episode.id, position(state)))
        .collect();
    assert_eq!(
        listed,
        [(ExternalId::tmdb(KNIGHT_PILOT_TMDB_ID), Some((1_000, false)))]
    );
}

/// The Movies the User `user` has in Continue Watching, by id, with how far in they are.
async fn continuing_movies(browse: &Browse, user: i64) -> Vec<(ExternalId, i64)> {
    let (started, _) = browse
        .continue_watching(user, &PageRequest::default())
        .await
        .unwrap();
    (started.into_iter())
        .map(|started| match started {
            Started::Movie { movie, state, .. } => (movie.id, state.position_ms),
            Started::Episode { .. } => panic!("only Movies were started"),
        })
        .collect()
}

#[tokio::test]
async fn a_movie_started_under_two_ids_is_in_continue_watching_once_at_the_newer() {
    let (data, folder) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let library = library_with_alien_by_two_ids(&db, &folder).await;
    let user = ada(&db).await;
    let tmdb = MovieOrEpisode::movie(&ExternalId::tmdb(ALIEN_TMDB_ID));
    let imdb = MovieOrEpisode::movie(&alien_imdb());
    report_under(&db, user, imdb, Progress::new(500, 2_000).unwrap()).await;
    // `imdb` sorts first, so only being more recent keeps `tmdb`'s state.
    db::advance_clock(Duration::from_secs(2));
    report_under(&db, user, tmdb, Progress::new(1_000, 2_000).unwrap()).await;
    let browse = Browse::new(db, ExtensionPoints::default());

    let continuing = continuing_movies(&browse, user).await;
    let (_, state, _) = (browse.movie(user, library, &ExternalId::tmdb(ALIEN_TMDB_ID)))
        .await
        .unwrap();

    assert_eq!(continuing, [(ExternalId::tmdb(ALIEN_TMDB_ID), 1_000)]);
    assert_eq!(position(state), Some((1_000, false)));
}

#[tokio::test]
async fn a_movie_played_under_one_id_after_it_was_started_under_another_is_not_continued() {
    let (data, folder) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let library = library_with_alien_by_two_ids(&db, &folder).await;
    let user = ada(&db).await;
    let tmdb = MovieOrEpisode::movie(&ExternalId::tmdb(ALIEN_TMDB_ID));
    let imdb = MovieOrEpisode::movie(&alien_imdb());
    report_under(&db, user, imdb, Progress::new(500, 2_000).unwrap()).await;
    // `imdb` sorts first, so only being more recent keeps `tmdb`'s state.
    db::advance_clock(Duration::from_secs(2));
    report_under(&db, user, tmdb, Progress::new(1_950, 2_000).unwrap()).await;
    let browse = Browse::new(db, ExtensionPoints::default());

    let continuing = continuing_movies(&browse, user).await;
    let (_, state, _) = (browse.movie(user, library, &ExternalId::tmdb(ALIEN_TMDB_ID)))
        .await
        .unwrap();

    assert_eq!(continuing, []);
    assert_eq!(position(state), Some((1_950, true)));
}

#[tokio::test]
async fn a_movie_played_under_an_id_its_library_doesnt_prefer_is_marked_unplayed() {
    let (data, folder) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let library = library_with_alien_by_two_ids(&db, &folder).await;
    let user = ada(&db).await;
    let imdb = MovieOrEpisode::movie(&alien_imdb());
    report_under(&db, user, imdb, Progress::new(1_950, 2_000).unwrap()).await;
    let browse = Browse::new(db, ExtensionPoints::default());
    let alien = ExternalId::tmdb(ALIEN_TMDB_ID);

    (browse.mark(user, library, Target::Movie(alien.clone()), false))
        .await
        .unwrap();

    let (_, state, _) = browse.movie(user, library, &alien).await.unwrap();
    assert_eq!(position(state), None);
}
