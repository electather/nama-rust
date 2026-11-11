//! Browsing Movies and Shows through the generated `BrowseService` client, and the Watch State
//! that `WatchStateService` reports or marks and browsing shows, over a real socket, with the TMDB
//! Plugin enabled against a fake TMDB.

#[path = "support/browsing.rs"]
mod browsing;
#[path = "support/http.rs"]
mod http;
#[path = "support/refused.rs"]
mod refused;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use std::{collections::BTreeSet, fs, slice, time::Duration};

use browsing::{ALIEN_1080P, ALIEN_2160P, Fixture, tmdb_id};
use connectrpc::{
    ConnectError, ErrorCode,
    client::{ClientConfig, HttpClient},
};
use http::fetch;
use nama_proto::nama::v1::{
    AudioTrack, BrowseServiceClient, ClearMatchRequest, ContinueWatchingEntry, Episode, ExternalId,
    GetMovieRequest, GetShowRequest, GetShowResponse, HdrFormat, LibraryKind,
    ListContinueWatchingRequest, ListEpisodesRequest, ListMoviesRequest, ListShowsRequest, Movie,
    ProbingFinished, RefusalReason, ReportProgressRequest, SearchLibrariesRequest,
    SearchLibrariesResponse, Season, SetMatchRequest, SetPlayedRequest, Show, ShowSeason, Track,
    Version, VideoTrack, WatchState, WatchStateServiceClient, continue_watching_entry, event,
    set_played_request::Target,
};
use nama_server::{
    config::Config,
    ffmpeg::{self, Ffmpeg},
};
use refused::assert_refused;
use server::{
    SignedIn,
    tmdb::{ALIEN, DARK, DUNE, THE_OFFICE as THE_OFFICE_US},
};

/// TMDB finds nothing for this file, so it is Unmatched.
const HOME_MOVIES: &str = "Home Movies (2019)/Home Movies (2019).mkv";

/// Dark in two folders: TMDB finds the first by its title and year, and the second names it in an
/// ID hint. The first file holds two Episodes.
const DARK_S01E01_E02: &str = "Dark (2017)/Season 01/Dark - S01E01-E02.mkv";
const DARK_S01E01: &str = "Dark {tmdb-70523}/Season 01/Dark - S01E01.mkv";
const DARK_S02E01: &str = "Dark {tmdb-70523}/Season 02/Dark - S02E01.mkv";
/// The TMDB ids of Dark's Episodes S01E01, S01E02 and S02E01.
const SECRETS: i64 = 1_388_538;
const LIES: i64 = 1_388_539;
const BEGINNINGS_AND_ENDINGS: i64 = 1_803_405;

/// Both the US and the UK Office fit, so this is Unmatched.
const THE_OFFICE: &str = "The Office/Season 01/The Office - S01E01.mkv";

/// The Office (US) by an ID hint, since TMDB's search fits the UK one too.
const THE_OFFICE_US_S01E02: &str = "The Office {tmdb-2316}/Season 01/The Office - S01E02.mkv";

/// TMDB Matches it to the 2021 Dune.
const DUNE_2021: &str = "Dune (2021)/Dune (2021).mkv";

/// Two hours, in milliseconds: the length a test's player reports.
const TWO_HOURS: u64 = 7_200_000;
/// An hour, in milliseconds: the length a test's player reports for the two-Episode file.
const AN_HOUR: u64 = 3_600_000;
const MINUTE: u64 = 60_000;

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
impl Fixture {
    /// Starts a Server with a Library of `kind`, and no ffmpeg.
    async fn start(kind: LibraryKind) -> Self {
        Self::start_with_ffmpeg(kind, Ffmpeg::default()).await
    }

    /// Starts a Server with a Shows Library holding Dark and The Office, scanned and Matched.
    async fn shows() -> Self {
        let mut fx = Self::start(LibraryKind::LIBRARY_KIND_SHOWS).await;
        fx.write(DARK_S01E01_E02, b"two episodes");
        fx.write(DARK_S01E01, b"one episode");
        fx.write(DARK_S02E01, b"video");
        fx.write(THE_OFFICE, b"video");
        fx.items = fx.scan_and_match().await;
        fx
    }

    /// The Admin, browsing and playing.
    fn admin(&self) -> Viewer {
        Viewer::new(self.server.client_config())
    }

    /// The Admin's search of every Library for `query`.
    async fn search(&self, query: &str) -> SearchLibrariesResponse {
        self.browse
            .search_libraries(SearchLibrariesRequest {
                query: query.to_owned(),
                ..Default::default()
            })
            .await
            .unwrap()
            .into_owned()
    }

    async fn list_movies(&self, library: i64) -> Result<Vec<Movie>, ConnectError> {
        Ok(self
            .browse
            .list_movies(ListMoviesRequest {
                library_id: library,
                ..Default::default()
            })
            .await?
            .into_owned()
            .movies)
    }

    async fn list_shows(&self, library: i64) -> Result<Vec<Show>, ConnectError> {
        Ok(self
            .browse
            .list_shows(ListShowsRequest {
                library_id: library,
                ..Default::default()
            })
            .await?
            .into_owned()
            .shows)
    }

    /// The Show of the Library `library` with the TMDB id `tmdb`, with its Seasons, as `GetShow`
    /// gives them.
    async fn get_show(&self, library: i64, tmdb: i64) -> Result<GetShowResponse, ConnectError> {
        Ok(self
            .browse
            .get_show(GetShowRequest {
                library_id: library,
                show_id: tmdb_id(tmdb).into(),
                ..Default::default()
            })
            .await?
            .into_owned())
    }

    /// The Episodes of Season `season` of the Show with the TMDB id `tmdb`, as `ListEpisodes`
    /// lists them.
    async fn list_episodes(
        &self,
        library: i64,
        tmdb: i64,
        season: u32,
    ) -> Result<Vec<Episode>, ConnectError> {
        Ok(self
            .browse
            .list_episodes(ListEpisodesRequest {
                library_id: library,
                show_id: tmdb_id(tmdb).into(),
                season_number: season,
                ..Default::default()
            })
            .await?
            .into_owned()
            .episodes)
    }

    /// The id of the Version of an Episode of Dark's Season `season` with the file name
    /// `file_name`.
    async fn dark_version(&self, season: u32, file_name: &str) -> i64 {
        let episodes = self
            .list_episodes(self.library, DARK, season)
            .await
            .unwrap();
        let mut versions = episodes.iter().flat_map(|episode| &episode.versions);
        versions
            .find(|version| version.file_name == file_name)
            .unwrap()
            .id
    }
}

/// One signed-in User's clients for browsing and for reporting what they play.
struct Viewer {
    browse: BrowseServiceClient<HttpClient>,
    watch_states: WatchStateServiceClient<HttpClient>,
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
impl Viewer {
    /// Clients that call the Server as the User `user`.
    fn of(user: &SignedIn) -> Self {
        Self::new(user.client_config())
    }

    /// Clients that call the Server with `config`'s session.
    fn new(config: ClientConfig) -> Self {
        Self {
            browse: BrowseServiceClient::new(HttpClient::plaintext(), config.clone()),
            watch_states: WatchStateServiceClient::new(HttpClient::plaintext(), config),
        }
    }

    /// Reports `position_ms` into the Version `version`, `duration_ms` long.
    async fn report(
        &self,
        version: i64,
        position_ms: u64,
        duration_ms: u64,
    ) -> Result<(), ConnectError> {
        self.watch_states
            .report_progress(ReportProgressRequest {
                version_id: version,
                position_ms,
                duration_ms,
                ..Default::default()
            })
            .await?;
        Ok(())
    }

    /// Marks `target` in the Library `library` played, or unplayed.
    async fn set_played(
        &self,
        library: i64,
        target: Target,
        played: bool,
    ) -> Result<(), ConnectError> {
        self.watch_states
            .set_played(SetPlayedRequest {
                library_id: library,
                target: Some(target),
                played,
                ..Default::default()
            })
            .await?;
        Ok(())
    }

    /// The Viewer's Watch State of the Movie with the TMDB id `tmdb` in the Library `library`, as
    /// `GetMovie` gives it, after checking `ListMovies` gives the same.
    async fn watch_state(&self, library: i64, tmdb: i64) -> Option<WatchState> {
        let got = self
            .browse
            .get_movie(GetMovieRequest {
                library_id: library,
                movie_id: tmdb_id(tmdb).into(),
                ..Default::default()
            })
            .await
            .unwrap()
            .into_owned()
            .movie
            .into_option()
            .unwrap()
            .watch_state
            .into_option();
        let listed = self
            .browse
            .list_movies(ListMoviesRequest {
                library_id: library,
                ..Default::default()
            })
            .await
            .unwrap()
            .into_owned()
            .movies
            .into_iter()
            .find(|movie| movie.id.as_option() == Some(&tmdb_id(tmdb)))
            .unwrap()
            .watch_state
            .into_option();
        assert_eq!(got, listed);
        got
    }

    async fn continue_watching(&self) -> Vec<ContinueWatchingEntry> {
        self.browse
            .list_continue_watching(ListContinueWatchingRequest::default())
            .await
            .unwrap()
            .into_owned()
            .entries
    }

    /// The Viewer's Watch State of each Episode of Dark's Season `season` in the Library
    /// `library`, in episode order, as `ListEpisodes` gives them.
    async fn dark_watch_states(&self, library: i64, season: u32) -> Vec<Option<WatchState>> {
        self.browse
            .list_episodes(ListEpisodesRequest {
                library_id: library,
                show_id: tmdb_id(DARK).into(),
                season_number: season,
                ..Default::default()
            })
            .await
            .unwrap()
            .into_owned()
            .episodes
            .into_iter()
            .map(|episode| episode.watch_state.into_option())
            .collect()
    }
}

/// Each Version's file name and size; ids are checked apart, since the Server assigns them.
fn versions(versions: &[Version]) -> Vec<(&str, u64)> {
    versions
        .iter()
        .map(|version| (version.file_name.as_str(), version.size))
        .collect()
}

#[tokio::test]
async fn two_files_matching_one_movie_are_one_movie_with_two_versions() {
    let fx = Fixture::start(LibraryKind::LIBRARY_KIND_MOVIES).await;
    fx.write(ALIEN_1080P, b"1080p");
    fx.write(ALIEN_2160P, b"a bigger 2160p copy");
    fx.write(HOME_MOVIES, b"video");
    fx.scan_and_match().await;

    let listed = fx.list_movies(fx.library).await.unwrap();
    let got = fx.get_movie(fx.library, ALIEN).await.unwrap();

    assert_eq!(listed, slice::from_ref(&got));
    assert_eq!(got.id.as_option(), Some(&tmdb_id(ALIEN)));
    assert_eq!(got.title, "Alien");
    assert_eq!(got.release_date.as_deref(), Some("1979-05-25"));
    assert_eq!(got.runtime_minutes, Some(117));
    assert_eq!(
        got.poster_url.as_deref(),
        Some("/media/artwork/tmdb/%2FvfrQk5IPloGg1v9Rzbh2Eg3VGyM.jpg")
    );
    assert_eq!(
        got.backdrop_url.as_deref(),
        Some("/media/artwork/tmdb/%2FAmR3JG1VQVxU8TfAvljUhfSFUOx.jpg")
    );
    assert_eq!(
        versions(&got.versions),
        [
            ("Alien (1979) - 1080p.mkv", 5),
            ("Alien (1979) - 2160p.mkv", 19)
        ]
    );
    let first = got.versions.first().map(|version| version.id);
    let last = got.versions.last().map(|version| version.id);
    assert_ne!(first, last);
}

#[tokio::test]
async fn a_version_has_the_tracks_probing_found_once_probing_finished_says_so() {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let ffmpeg = Ffmpeg::find(&Config::default(), &path);
    assert!(ffmpeg.ffprobe.is_ok(), "this test needs ffprobe on PATH");
    let fx = Fixture::start_with_ffmpeg(LibraryKind::LIBRARY_KIND_MOVIES, ffmpeg).await;
    let file = fx.folder.join(ALIEN_2160P);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    ffmpeg::write_4k_with_5_1(&file, "smpte2084").unwrap();

    let events = fx
        .server
        .scan_until(fx.library, |event| {
            matches!(event, event::Event::ProbingFinished(_))
        })
        .await;

    let probed = ProbingFinished {
        library_id: fx.library,
        probed: 1,
        ..Default::default()
    };
    assert_eq!(events.last(), Some(&probed.into()));
    let movie = fx.get_movie(fx.library, ALIEN).await.unwrap();
    let version = movie.versions.first().unwrap();
    assert_eq!(
        version.tracks,
        [
            Track {
                codec: "ffv1".to_owned(),
                kind: VideoTrack {
                    width: 3840,
                    height: 2160,
                    frame_rate: 24.0,
                    bit_depth: 10,
                    hdr: HdrFormat::HDR_FORMAT_HDR10.into(),
                    ..Default::default()
                }
                .into(),
                ..Default::default()
            },
            Track {
                codec: "ac3".to_owned(),
                language: Some("eng".to_owned()),
                kind: AudioTrack {
                    channels: 6,
                    ..Default::default()
                }
                .into(),
                ..Default::default()
            },
        ]
    );
}

#[tokio::test]
async fn an_unmatched_item_is_in_neither_list_movies_nor_get_movie() {
    let fx = Fixture::alien().await;
    let item = fx.item(ALIEN_1080P);
    fx.matches
        .clear_match(ClearMatchRequest {
            item_id: item,
            ..Default::default()
        })
        .await
        .unwrap();

    let listed = fx.list_movies(fx.library).await.unwrap();
    let got = fx.get_movie(fx.library, ALIEN).await;

    assert_eq!(listed, []);
    assert_refused(
        got,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_NOTHING_MATCHED,
    );
}

#[tokio::test]
async fn an_unknown_library_is_not_found() {
    let fx = Fixture::start(LibraryKind::LIBRARY_KIND_MOVIES).await;
    let unknown = fx.library + 1;

    let movies = fx.list_movies(unknown).await;
    let movie = fx.get_movie(unknown, ALIEN).await;
    let shows = fx.list_shows(unknown).await;
    let show = fx.get_show(unknown, DARK).await;
    let episodes = fx.list_episodes(unknown, DARK, 1).await;

    let refusal = assert_refused(
        movies,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_LIBRARY_NOT_FOUND,
    );
    let library_id = unknown.to_string();
    assert_eq!(refusal.field, "library_id");
    assert_eq!(refusal.params.get("libraryId"), Some(&library_id));
    assert_refused(
        movie,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_LIBRARY_NOT_FOUND,
    );
    assert_refused(
        shows,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_LIBRARY_NOT_FOUND,
    );
    assert_refused(
        show,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_LIBRARY_NOT_FOUND,
    );
    assert_refused(
        episodes,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_LIBRARY_NOT_FOUND,
    );
}

#[tokio::test]
async fn a_shows_library_is_an_invalid_argument() {
    let fx = Fixture::start(LibraryKind::LIBRARY_KIND_SHOWS).await;

    let listed = fx.list_movies(fx.library).await;
    let got = fx.get_movie(fx.library, ALIEN).await;

    let refusal = assert_refused(
        listed,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_WRONG_LIBRARY_KIND,
    );
    assert_eq!(refusal.field, "library_id");
    assert_refused(
        got,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_WRONG_LIBRARY_KIND,
    );
}

#[tokio::test]
async fn get_movie_for_an_id_nothing_matches_is_not_found() {
    let fx = Fixture::alien().await;

    let got = fx.get_movie(fx.library, 949).await;

    assert_refused(
        got,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_NOTHING_MATCHED,
    );
}

#[tokio::test]
async fn episodes_of_one_show_in_several_folders_are_one_show_with_its_seasons() {
    let fx = Fixture::shows().await;

    let listed = fx.list_shows(fx.library).await.unwrap();
    let got = fx.get_show(fx.library, DARK).await.unwrap();
    let episodes = fx.list_episodes(fx.library, DARK, 1).await.unwrap();

    // Both folders' files are in the one Show.
    let files: BTreeSet<&str> = episodes
        .iter()
        .flat_map(|episode| &episode.versions)
        .map(|version| version.file_name.as_str())
        .collect();
    assert_eq!(
        files,
        BTreeSet::from(["Dark - S01E01-E02.mkv", "Dark - S01E01.mkv"])
    );
    let show = got.show.into_option().unwrap();
    assert_eq!(listed, slice::from_ref(&show));
    assert_eq!(show.id.as_option(), Some(&tmdb_id(DARK)));
    assert_eq!(show.title, "Dark");
    // Season 1's poster: a Match stores no poster of the Show's own.
    assert_eq!(
        show.poster_url.as_deref(),
        Some("/media/artwork/tmdb/%2F5LoHuHWA4H8jElFlZDvsmU2n63b.jpg")
    );
    assert_eq!(
        show.backdrop_url.as_deref(),
        Some("/media/artwork/tmdb/%2F3lBDg3i6nn5R2NKFCJ6oKyUo2j5.jpg")
    );
    assert_eq!(
        got.seasons,
        [
            season(1, "/media/artwork/tmdb/%2F5LoHuHWA4H8jElFlZDvsmU2n63b.jpg"),
            season(2, "/media/artwork/tmdb/%2FkaReZbBdlEAJzfUbuPBRQLrSh9z.jpg"),
        ]
    );
}

#[tokio::test]
async fn list_episodes_lists_one_seasons_episodes_in_episode_order() {
    let fx = Fixture::shows().await;

    let first = fx.list_episodes(fx.library, DARK, 1).await.unwrap();
    let second = fx.list_episodes(fx.library, DARK, 2).await.unwrap();

    assert_eq!(
        ids_and_numbers(&first),
        [(Some(tmdb_id(SECRETS)), 1, 1), (Some(tmdb_id(LIES)), 1, 2)]
    );
    assert_eq!(
        ids_and_numbers(&second),
        [(Some(tmdb_id(BEGINNINGS_AND_ENDINGS)), 2, 1)]
    );
    let secrets = first.first().unwrap();
    assert_eq!(secrets.title, "Secrets");
    assert!(secrets.overview.as_deref().unwrap().starts_with("In 2019"));
    assert_eq!(secrets.air_date.as_deref(), Some("2017-12-01"));
    assert_eq!(secrets.runtime_minutes, Some(52));
}

#[tokio::test]
async fn a_file_holding_two_episodes_is_a_version_of_each() {
    let fx = Fixture::shows().await;

    let episodes = fx.list_episodes(fx.library, DARK, 1).await.unwrap();

    let [secrets, lies] = episodes.as_slice() else {
        panic!("{episodes:?}");
    };
    assert_eq!(
        versions(&secrets.versions),
        [("Dark - S01E01-E02.mkv", 12), ("Dark - S01E01.mkv", 11)]
    );
    assert_eq!(versions(&lies.versions), [("Dark - S01E01-E02.mkv", 12)]);
    assert_eq!(secrets.versions.first(), lies.versions.first());
}

#[tokio::test]
async fn an_unmatched_item_is_in_no_show_season_or_episode_list() {
    let fx = Fixture::shows().await;
    // Unmatches every Episode in the `Dark {tmdb-70523}` folder.
    let item = fx.item(DARK_S02E01);
    fx.matches
        .clear_match(ClearMatchRequest {
            item_id: item,
            ..Default::default()
        })
        .await
        .unwrap();

    let shows = fx.list_shows(fx.library).await.unwrap();
    let seasons = fx.get_show(fx.library, DARK).await.unwrap().seasons;
    let first = fx.list_episodes(fx.library, DARK, 1).await.unwrap();
    let second = fx.list_episodes(fx.library, DARK, 2).await;

    assert_eq!(
        shows
            .iter()
            .map(|show| show.title.as_str())
            .collect::<Vec<_>>(),
        ["Dark"]
    );
    assert_eq!(
        seasons
            .iter()
            .map(|season| season.number)
            .collect::<Vec<_>>(),
        [1]
    );
    let versions: Vec<_> = first.iter().map(|e| versions(&e.versions)).collect();
    assert_eq!(
        versions,
        [
            [("Dark - S01E01-E02.mkv", 12)],
            [("Dark - S01E01-E02.mkv", 12)]
        ]
    );
    assert_refused(
        second,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_SEASON_NOT_FOUND,
    );
}

#[tokio::test]
async fn an_unknown_show_or_season_is_not_found() {
    let fx = Fixture::shows().await;

    let show = fx.get_show(fx.library, THE_OFFICE_US).await;
    let episodes = fx.list_episodes(fx.library, THE_OFFICE_US, 1).await;
    let season = fx.list_episodes(fx.library, DARK, 3).await;

    assert_refused(
        show,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_NOTHING_MATCHED,
    );
    assert_refused(
        episodes,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_NOTHING_MATCHED,
    );
    assert_refused(
        season,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_SEASON_NOT_FOUND,
    );
}

#[tokio::test]
async fn a_movies_library_is_an_invalid_argument_to_browse_shows() {
    let fx = Fixture::start(LibraryKind::LIBRARY_KIND_MOVIES).await;

    let shows = fx.list_shows(fx.library).await;
    let show = fx.get_show(fx.library, DARK).await;
    let episodes = fx.list_episodes(fx.library, DARK, 1).await;

    assert_refused(
        shows,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_WRONG_LIBRARY_KIND,
    );
    assert_refused(
        show,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_WRONG_LIBRARY_KIND,
    );
    assert_refused(
        episodes,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_WRONG_LIBRARY_KIND,
    );
}

#[tokio::test]
async fn a_poster_a_match_stores_is_served_with_its_content_type() {
    let fx = Fixture::alien().await;
    let movie = fx.get_movie(fx.library, ALIEN).await.unwrap();
    let poster = movie.poster_url.unwrap();

    let reply = fetch(&fx.server, &poster, Some(&fx.server.admin_token)).await;

    assert_eq!(reply.status, 200);
    assert_eq!(reply.header("content-type"), "image/jpeg");
    assert!(reply.body.starts_with(b"\xff\xd8"));
}

/// Only images some Match stores are served, so clients can't use the cache to fetch others.
#[tokio::test]
async fn any_other_artwork_path_is_not_found() {
    let fx = Fixture::alien().await;

    for path in [
        "/media/artwork/tmdb/%2Fnot-stored.jpg",
        "/media/artwork/other-plugin/%2FvfrQk5IPloGg1v9Rzbh2Eg3VGyM.jpg",
        "/media/artwork/tmdb/../../nama.db",
        "/media/artwork/tmdb/..%2F..%2Fnama.db",
        "/media/artwork/%2E%2E/%2E%2E%2Fnama.db",
        "/media/artwork/tmdb",
    ] {
        let reply = fetch(&fx.server, path, Some(&fx.server.admin_token)).await;

        assert_eq!(reply.status, 404, "{path}");
    }
    let images = fx.server.tmdb().requests();
    assert!(
        !images.iter().any(|request| request.starts_with("/t/p/")),
        "{images:?}"
    );
}

/// The session cookie is what lets a browser's `<img src>` fetch artwork.
#[tokio::test]
async fn artwork_without_a_session_is_refused() {
    let fx = Fixture::alien().await;
    let movie = fx.get_movie(fx.library, ALIEN).await.unwrap();

    let reply = fetch(&fx.server, &movie.poster_url.unwrap(), None).await;

    assert_eq!(reply.status, 401);
}

/// A client should try again later, rather than take the image to be gone.
#[tokio::test]
async fn artwork_from_a_provider_that_fails_is_unavailable() {
    let mut fx = Fixture::alien().await;
    let movie = fx.get_movie(fx.library, ALIEN).await.unwrap();
    fx.server.stop_tmdb();

    let reply = fetch(
        &fx.server,
        &movie.poster_url.unwrap(),
        Some(&fx.server.admin_token),
    )
    .await;

    assert_eq!(reply.status, 503);
}

/// A Season as `GetShow` gives it.
fn season(number: u32, poster_url: &str) -> Season {
    Season {
        number,
        poster_url: Some(poster_url.to_owned()),
        ..Default::default()
    }
}

/// Each Episode's id, season number and episode number.
fn ids_and_numbers(episodes: &[Episode]) -> Vec<(Option<ExternalId>, u32, u32)> {
    episodes
        .iter()
        .map(|episode| {
            let id = episode.id.as_option().cloned();
            (id, episode.season_number, episode.episode_number)
        })
        .collect()
}

/// The position and played flag of `state`, if it is set.
fn progress(state: Option<WatchState>) -> Option<(u64, bool)> {
    state.map(|state| (state.position_ms, state.played))
}

#[tokio::test]
async fn progress_survives_a_rescan_that_replaces_the_item() {
    let fx = Fixture::alien().await;
    let viewer = fx.admin();
    let [version] = fx.version_ids(ALIEN).await[..] else {
        panic!("Alien has one Version");
    };

    viewer.report(version, 60_000, TWO_HOURS).await.unwrap();
    fx.write(ALIEN_1080P, b"a re-encoded copy of the video");
    fx.scan_and_match().await;

    let state = viewer.watch_state(fx.library, ALIEN).await.unwrap();
    assert_eq!((state.position_ms, state.duration_ms), (60_000, TWO_HOURS));
    assert!(!state.played);
    assert!(state.updated_at > 0);
}

#[tokio::test]
async fn progress_reported_on_one_version_shows_on_the_movie_for_every_version() {
    let fx = Fixture::start(LibraryKind::LIBRARY_KIND_MOVIES).await;
    fx.write(ALIEN_1080P, b"1080p");
    fx.write(ALIEN_2160P, b"a bigger 2160p copy");
    fx.scan_and_match().await;
    let viewer = fx.admin();
    let [hd, uhd] = fx.version_ids(ALIEN).await[..] else {
        panic!("Alien has two Versions");
    };

    viewer.report(uhd, 60_000, TWO_HOURS).await.unwrap();
    let first_report = progress(viewer.watch_state(fx.library, ALIEN).await);
    viewer.report(hd, 90_000, TWO_HOURS).await.unwrap();
    let second_report = progress(viewer.watch_state(fx.library, ALIEN).await);

    assert_eq!(first_report, Some((60_000, false)));
    assert_eq!(second_report, Some((90_000, false)));
}

#[tokio::test]
async fn passing_90_percent_of_the_duration_marks_the_movie_played_and_it_stays_played() {
    let fx = Fixture::alien().await;
    let viewer = fx.admin();
    let [version] = fx.version_ids(ALIEN).await[..] else {
        panic!("Alien has one Version");
    };

    viewer.report(version, 900, 1_000).await.unwrap();
    let at_90 = progress(viewer.watch_state(fx.library, ALIEN).await);
    viewer.report(version, 901, 1_000).await.unwrap();
    let past_90 = progress(viewer.watch_state(fx.library, ALIEN).await);
    viewer.report(version, 0, 1_000).await.unwrap();
    let played_again = progress(viewer.watch_state(fx.library, ALIEN).await);

    assert_eq!(at_90, Some((900, false)));
    assert_eq!(past_90, Some((901, true)));
    assert_eq!(played_again, Some((0, true)));
}

#[tokio::test]
async fn a_version_of_an_unmatched_item_is_accepted_and_stores_nothing() {
    let fx = Fixture::alien().await;
    let viewer = fx.admin();
    let [version] = fx.version_ids(ALIEN).await[..] else {
        panic!("Alien has one Version");
    };
    let item = fx.item(ALIEN_1080P);
    fx.matches
        .clear_match(ClearMatchRequest {
            item_id: item,
            ..Default::default()
        })
        .await
        .unwrap();

    let reported = viewer.report(version, 60_000, TWO_HOURS).await;
    fx.matches
        .set_match(SetMatchRequest {
            item_id: item,
            title_id: tmdb_id(ALIEN).into(),
            ..Default::default()
        })
        .await
        .unwrap();

    assert!(reported.is_ok(), "{reported:?}");
    assert_eq!(viewer.watch_state(fx.library, ALIEN).await, None);
}

#[tokio::test]
async fn two_users_watch_state_for_the_same_movie_are_kept_apart() {
    let fx = Fixture::alien().await;
    let admin = fx.admin();
    let grace = Viewer::of(&fx.server.user_with_default_permissions("Grace").await);
    let [version] = fx.version_ids(ALIEN).await[..] else {
        panic!("Alien has one Version");
    };

    admin.report(version, 60_000, TWO_HOURS).await.unwrap();
    let grace_before = grace.watch_state(fx.library, ALIEN).await;
    grace.report(version, 7_000_000, TWO_HOURS).await.unwrap();

    assert_eq!(grace_before, None);
    assert_eq!(
        progress(admin.watch_state(fx.library, ALIEN).await),
        Some((60_000, false))
    );
    assert_eq!(
        progress(grace.watch_state(fx.library, ALIEN).await),
        Some((7_000_000, true))
    );
}

#[tokio::test]
async fn reporting_on_an_unknown_version_is_not_found_and_a_zero_duration_is_invalid() {
    let fx = Fixture::alien().await;
    let viewer = fx.admin();
    let [version] = fx.version_ids(ALIEN).await[..] else {
        panic!("Alien has one Version");
    };

    let unknown = viewer.report(version + 1, 0, TWO_HOURS).await;
    let zero = viewer.report(version, 0, 0).await;

    assert_refused(
        unknown,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_VERSION_NOT_FOUND,
    );
    let refusal = assert_refused(
        zero,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_DURATION_OUT_OF_RANGE,
    );
    assert_eq!(refusal.field, "duration_ms");
}

/// The position, duration and played flag of each of `states` that is set.
fn episode_progress(states: Vec<Option<WatchState>>) -> Vec<Option<(u64, u64, bool)>> {
    states
        .into_iter()
        .map(|state| state.map(|state| (state.position_ms, state.duration_ms, state.played)))
        .collect()
}

#[tokio::test]
async fn progress_on_a_version_of_an_episode_shows_on_that_episode() {
    let fx = Fixture::shows().await;
    let viewer = fx.admin();
    let version = fx.dark_version(2, "Dark - S02E01.mkv").await;

    viewer.report(version, MINUTE, TWO_HOURS).await.unwrap();

    let states = viewer.dark_watch_states(fx.library, 2).await;
    assert_eq!(episode_progress(states), [Some((MINUTE, TWO_HOURS, false))]);
    let first_season = viewer.dark_watch_states(fx.library, 1).await;
    assert_eq!(first_season, [None, None]);
}

#[tokio::test]
async fn a_two_episode_file_splits_its_position_evenly_in_episode_order() {
    let fx = Fixture::shows().await;
    let viewer = fx.admin();
    let version = fx.dark_version(1, "Dark - S01E01-E02.mkv").await;
    let half = AN_HOUR / 2;

    viewer.report(version, 20 * MINUTE, AN_HOUR).await.unwrap();
    let at_20 = episode_progress(viewer.dark_watch_states(fx.library, 1).await);
    viewer.report(version, 40 * MINUTE, AN_HOUR).await.unwrap();
    let at_40 = episode_progress(viewer.dark_watch_states(fx.library, 1).await);

    assert_eq!(at_20, [Some((20 * MINUTE, half, false)), None]);
    assert_eq!(
        at_40,
        [Some((half, half, true)), Some((10 * MINUTE, half, false))]
    );
}

#[tokio::test]
async fn an_episode_of_a_two_episode_file_is_played_once_past_90_percent_of_its_share() {
    let fx = Fixture::shows().await;
    let viewer = fx.admin();
    let version = fx.dark_version(1, "Dark - S01E01-E02.mkv").await;
    let half = AN_HOUR / 2;

    viewer.report(version, 27 * MINUTE, AN_HOUR).await.unwrap();
    let at_27 = episode_progress(viewer.dark_watch_states(fx.library, 1).await);
    viewer.report(version, 28 * MINUTE, AN_HOUR).await.unwrap();
    let at_28 = episode_progress(viewer.dark_watch_states(fx.library, 1).await);

    assert_eq!(at_27, [Some((27 * MINUTE, half, false)), None], "at 90%");
    assert_eq!(at_28, [Some((28 * MINUTE, half, true)), None], "past 90%");
}

#[tokio::test]
async fn a_position_in_the_first_episode_leaves_progress_on_the_second_alone() {
    let fx = Fixture::shows().await;
    let viewer = fx.admin();
    let version = fx.dark_version(1, "Dark - S01E01-E02.mkv").await;
    viewer.report(version, 40 * MINUTE, AN_HOUR).await.unwrap();
    let [_, lies_before]: [_; 2] = viewer
        .dark_watch_states(fx.library, 1)
        .await
        .try_into()
        .unwrap();

    viewer.report(version, 5 * MINUTE, AN_HOUR).await.unwrap();

    let [secrets, lies]: [_; 2] = viewer
        .dark_watch_states(fx.library, 1)
        .await
        .try_into()
        .unwrap();
    assert_eq!(progress(secrets), Some((5 * MINUTE, true)));
    assert_eq!(lies, lies_before);
}

#[tokio::test]
async fn progress_on_an_episode_survives_a_rescan_that_replaces_the_item() {
    let fx = Fixture::shows().await;
    let viewer = fx.admin();
    let version = fx.dark_version(2, "Dark - S02E01.mkv").await;

    viewer.report(version, MINUTE, TWO_HOURS).await.unwrap();
    fx.write(DARK_S02E01, b"a re-encoded copy of the video");
    fx.scan_and_match().await;

    let states = viewer.dark_watch_states(fx.library, 2).await;
    assert_eq!(episode_progress(states), [Some((MINUTE, TWO_HOURS, false))]);
}

/// A Continue Watching entry's Movie or Episode, as a test checks it.
#[derive(Debug, PartialEq)]
enum Listed<'a> {
    /// A Movie, by its id.
    Movie(ExternalId),
    /// An Episode, by its Show's title and its season and episode numbers.
    Episode(&'a str, u32, u32),
}

/// Each entry's Library, its Movie or Episode, and the position its Watch State is at.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn listed(entries: &[ContinueWatchingEntry]) -> Vec<(i64, Listed<'_>, u64)> {
    use continue_watching_entry::MovieOrEpisode;
    entries
        .iter()
        .map(|entry| {
            let (listed, state) = match entry.movie_or_episode.as_ref().unwrap() {
                MovieOrEpisode::Movie(movie) => {
                    let id = movie.id.as_option().unwrap().clone();
                    (Listed::Movie(id), &movie.watch_state)
                }
                MovieOrEpisode::Episode(started) => {
                    let show = started.show.as_option().unwrap().title.as_str();
                    let episode = started.episode.as_option().unwrap();
                    let numbers = (episode.season_number, episode.episode_number);
                    (
                        Listed::Episode(show, numbers.0, numbers.1),
                        &episode.watch_state,
                    )
                }
            };
            (
                entry.library_id,
                listed,
                state.as_option().unwrap().position_ms,
            )
        })
        .collect()
}

#[tokio::test]
async fn started_movies_and_episodes_are_listed_most_recent_first() {
    let fx = Fixture::alien().await;
    let shows_media = fx
        .server
        .matched_library(LibraryKind::LIBRARY_KIND_SHOWS, &[DARK_S02E01])
        .await;
    let shows = shows_media.id;
    let viewer = fx.admin();
    let [alien] = fx.version_ids(ALIEN).await[..] else {
        panic!("Alien has one Version");
    };
    let episodes = fx.list_episodes(shows, DARK, 2).await.unwrap();
    let dark = episodes[0].versions[0].id;

    viewer.report(alien, MINUTE, TWO_HOURS).await.unwrap();
    // A Watch State's `updated_at` is in seconds.
    tokio::time::sleep(Duration::from_millis(1_100)).await;
    viewer.report(dark, 2 * MINUTE, TWO_HOURS).await.unwrap();

    let entries = viewer.continue_watching().await;
    assert_eq!(
        listed(&entries),
        [
            (shows, Listed::Episode("Dark", 2, 1), 2 * MINUTE),
            (fx.library, Listed::Movie(tmdb_id(ALIEN)), MINUTE),
        ]
    );
    let Some(continue_watching_entry::MovieOrEpisode::Movie(movie)) = &entries[1].movie_or_episode
    else {
        panic!("Alien is a Movie");
    };
    assert_eq!(versions(&movie.versions), [("Alien (1979) - 1080p.mkv", 5)]);
}

#[tokio::test]
async fn played_episodes_and_episodes_never_started_are_not_listed() {
    let fx = Fixture::shows().await;
    let viewer = fx.admin();
    let two_episodes = fx.dark_version(1, "Dark - S01E01-E02.mkv").await;
    let beginnings_and_endings = fx.dark_version(2, "Dark - S02E01.mkv").await;

    // Secrets played, Lies 10 minutes in.
    viewer
        .report(two_episodes, 40 * MINUTE, AN_HOUR)
        .await
        .unwrap();
    viewer
        .report(beginnings_and_endings, 0, AN_HOUR)
        .await
        .unwrap();

    let entries = viewer.continue_watching().await;
    assert_eq!(
        listed(&entries),
        [(fx.library, Listed::Episode("Dark", 1, 2), 10 * MINUTE)]
    );
}

#[tokio::test]
async fn a_played_movie_is_not_listed() {
    let fx = Fixture::alien().await;
    let viewer = fx.admin();
    let [version] = fx.version_ids(ALIEN).await[..] else {
        panic!("Alien has one Version");
    };

    viewer.report(version, 901, 1_000).await.unwrap();
    viewer.report(version, 100, 1_000).await.unwrap();

    assert_eq!(viewer.continue_watching().await, []);
}

#[tokio::test]
async fn a_movie_whose_file_has_gone_is_listed_again_once_a_rescan_matches_it() {
    let fx = Fixture::alien().await;
    let viewer = fx.admin();
    let [version] = fx.version_ids(ALIEN).await[..] else {
        panic!("Alien has one Version");
    };
    viewer.report(version, MINUTE, TWO_HOURS).await.unwrap();

    fs::remove_file(fx.folder.join(ALIEN_1080P)).unwrap();
    fx.scan_and_match().await;
    let gone = viewer.continue_watching().await;
    fx.write(ALIEN_1080P, b"video");
    fx.scan_and_match().await;
    let back = viewer.continue_watching().await;

    assert_eq!(gone, []);
    assert_eq!(
        listed(&back),
        [(fx.library, Listed::Movie(tmdb_id(ALIEN)), MINUTE)]
    );
}

#[tokio::test]
async fn another_users_progress_never_appears() {
    let fx = Fixture::alien().await;
    let admin = fx.admin();
    let grace = Viewer::of(&fx.server.user_with_default_permissions("Grace").await);
    let [version] = fx.version_ids(ALIEN).await[..] else {
        panic!("Alien has one Version");
    };

    admin.report(version, MINUTE, TWO_HOURS).await.unwrap();
    let grace_before = grace.continue_watching().await;
    grace.report(version, 2 * MINUTE, TWO_HOURS).await.unwrap();

    assert_eq!(grace_before, []);
    let alien = |position| [(fx.library, Listed::Movie(tmdb_id(ALIEN)), position)];
    assert_eq!(listed(&admin.continue_watching().await), alien(MINUTE));
    assert_eq!(listed(&grace.continue_watching().await), alien(2 * MINUTE));
}

fn movie(tmdb: i64) -> Target {
    Target::MovieId(Box::new(tmdb_id(tmdb)))
}

fn episode(tmdb: i64) -> Target {
    Target::EpisodeId(Box::new(tmdb_id(tmdb)))
}

fn season_of(tmdb: i64, number: u32) -> Target {
    Target::Season(Box::new(ShowSeason {
        show_id: tmdb_id(tmdb).into(),
        season_number: number,
        ..Default::default()
    }))
}

fn show(tmdb: i64) -> Target {
    Target::ShowId(Box::new(tmdb_id(tmdb)))
}

/// Whether each of `states` is played, `None` for one that isn't set.
fn played(states: Vec<Option<WatchState>>) -> Vec<Option<bool>> {
    states
        .into_iter()
        .map(|state| state.map(|state| state.played))
        .collect()
}

#[tokio::test]
async fn marking_a_movie_played_without_playing_it_shows_in_get_movie() {
    let fx = Fixture::alien().await;
    let viewer = fx.admin();

    viewer
        .set_played(fx.library, movie(ALIEN), true)
        .await
        .unwrap();

    let state = viewer.watch_state(fx.library, ALIEN).await.unwrap();
    assert_eq!((state.position_ms, state.duration_ms), (0, 0));
    assert!(state.played);
}

#[tokio::test]
async fn marking_an_episode_played_shows_in_list_episodes() {
    let fx = Fixture::shows().await;
    let viewer = fx.admin();

    viewer
        .set_played(fx.library, episode(LIES), true)
        .await
        .unwrap();

    let states = viewer.dark_watch_states(fx.library, 1).await;
    assert_eq!(played(states), [None, Some(true)]);
}

#[tokio::test]
async fn marking_a_season_played_marks_each_of_its_episodes_and_no_other_seasons() {
    let fx = Fixture::shows().await;
    let viewer = fx.admin();

    viewer
        .set_played(fx.library, season_of(DARK, 1), true)
        .await
        .unwrap();

    let first = viewer.dark_watch_states(fx.library, 1).await;
    let second = viewer.dark_watch_states(fx.library, 2).await;
    assert_eq!(played(first), [Some(true), Some(true)]);
    assert_eq!(second, [None]);
}

#[tokio::test]
async fn marking_a_show_played_marks_every_episode_of_every_season_and_unplayed_clears_them() {
    let fx = Fixture::shows().await;
    let viewer = fx.admin();

    viewer
        .set_played(fx.library, show(DARK), true)
        .await
        .unwrap();
    let first = viewer.dark_watch_states(fx.library, 1).await;
    let second = viewer.dark_watch_states(fx.library, 2).await;
    viewer
        .set_played(fx.library, show(DARK), false)
        .await
        .unwrap();
    let first_unplayed = viewer.dark_watch_states(fx.library, 1).await;
    let second_unplayed = viewer.dark_watch_states(fx.library, 2).await;

    assert_eq!(played(first), [Some(true), Some(true)]);
    assert_eq!(played(second), [Some(true)]);
    assert_eq!(first_unplayed, [None, None]);
    assert_eq!(second_unplayed, [None]);
}

#[tokio::test]
async fn marking_a_partly_watched_movie_played_keeps_its_position_and_unplayed_clears_it() {
    let fx = Fixture::alien().await;
    let viewer = fx.admin();
    let [version] = fx.version_ids(ALIEN).await[..] else {
        panic!("Alien has one Version");
    };
    viewer.report(version, MINUTE, TWO_HOURS).await.unwrap();

    viewer
        .set_played(fx.library, movie(ALIEN), true)
        .await
        .unwrap();
    let marked_played = progress(viewer.watch_state(fx.library, ALIEN).await);
    viewer
        .set_played(fx.library, movie(ALIEN), false)
        .await
        .unwrap();
    let marked_unplayed = viewer.watch_state(fx.library, ALIEN).await;

    assert_eq!(marked_played, Some((MINUTE, true)));
    assert_eq!(marked_unplayed, None);
}

#[tokio::test]
async fn marking_one_users_movie_played_leaves_anothers_alone() {
    let fx = Fixture::alien().await;
    let admin = fx.admin();
    let grace = Viewer::of(&fx.server.user_with_default_permissions("Grace").await);

    grace
        .set_played(fx.library, movie(ALIEN), true)
        .await
        .unwrap();

    assert_eq!(admin.watch_state(fx.library, ALIEN).await, None);
}

#[tokio::test]
async fn marking_an_unknown_target_is_not_found() {
    let shows = Fixture::shows().await;
    let movies = Fixture::alien().await;
    let (in_shows, in_movies) = (shows.admin(), movies.admin());
    let unknown_id = 1;

    for (reason, marked) in [
        (
            RefusalReason::REFUSAL_REASON_LIBRARY_NOT_FOUND,
            in_shows
                .set_played(shows.library + 100, show(DARK), true)
                .await,
        ),
        (
            RefusalReason::REFUSAL_REASON_NOTHING_MATCHED,
            in_shows
                .set_played(shows.library, show(unknown_id), true)
                .await,
        ),
        (
            RefusalReason::REFUSAL_REASON_SEASON_NOT_FOUND,
            in_shows
                .set_played(shows.library, season_of(DARK, 9), true)
                .await,
        ),
        (
            RefusalReason::REFUSAL_REASON_NOTHING_MATCHED,
            in_shows
                .set_played(shows.library, episode(unknown_id), true)
                .await,
        ),
        (
            RefusalReason::REFUSAL_REASON_NOTHING_MATCHED,
            in_movies
                .set_played(movies.library, movie(unknown_id), true)
                .await,
        ),
    ] {
        assert_refused(marked, ErrorCode::NotFound, reason);
    }
}

#[tokio::test]
async fn marking_a_movie_of_a_shows_library_or_a_show_of_a_movies_library_is_invalid() {
    let shows = Fixture::shows().await;
    let movies = Fixture::alien().await;

    let a_movie = shows
        .admin()
        .set_played(shows.library, movie(ALIEN), true)
        .await;
    let in_movies = movies.admin();
    let a_show = in_movies.set_played(movies.library, show(DARK), true).await;
    let a_season = in_movies
        .set_played(movies.library, season_of(DARK, 1), true)
        .await;
    let an_episode = in_movies
        .set_played(movies.library, episode(LIES), true)
        .await;
    let nothing = in_movies
        .watch_states
        .set_played(SetPlayedRequest {
            library_id: movies.library,
            played: true,
            ..Default::default()
        })
        .await;

    assert_refused(
        a_movie,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_WRONG_LIBRARY_KIND,
    );
    assert_refused(
        a_show,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_WRONG_LIBRARY_KIND,
    );
    assert_refused(
        a_season,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_WRONG_LIBRARY_KIND,
    );
    assert_refused(
        an_episode,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_WRONG_LIBRARY_KIND,
    );
    let refusal = assert_refused(
        nothing,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
    );
    assert_eq!(refusal.field, "target");
}

#[tokio::test]
async fn a_search_finds_a_show_by_its_title_and_an_episode_with_its_show_and_season() {
    let fx = Fixture::start(LibraryKind::LIBRARY_KIND_SHOWS).await;
    fx.write(THE_OFFICE_US_S01E02, b"video");
    fx.scan_and_match().await;

    let office = fx.search("office").await;
    let diversity = fx.search("diversity").await;

    let shows: Vec<_> = (office.shows.iter())
        .map(|show| (show.library_id, &*show.id, show.title.as_str()))
        .collect();
    assert_eq!(shows, [(fx.library, &tmdb_id(THE_OFFICE_US), "The Office")]);
    let episodes: Vec<_> = (diversity.episodes.iter())
        .map(|episode| {
            let numbers = (episode.season_number, episode.episode_number);
            (
                episode.library_id,
                &*episode.show_id,
                episode.show_title.as_str(),
                numbers,
            )
        })
        .collect();
    let office_us = tmdb_id(THE_OFFICE_US);
    assert_eq!(episodes, [(fx.library, &office_us, "The Office", (1, 2))]);
    assert_eq!(diversity.episodes[0].title, "Diversity Day");
}

#[tokio::test]
async fn a_movie_a_scan_matches_is_found_right_after_it_and_not_once_its_file_is_removed() {
    let fx = Fixture::start(LibraryKind::LIBRARY_KIND_MOVIES).await;
    fx.write(DUNE_2021, b"video");
    fx.scan_and_match().await;

    let found = fx.search("dune 2021").await.movies;

    let movies: Vec<_> = (found.iter())
        .map(|movie| (movie.library_id, &*movie.id, movie.title.as_str()))
        .collect();
    assert_eq!(movies, [(fx.library, &tmdb_id(DUNE), "Dune")]);
    fs::remove_file(fx.folder.join(DUNE_2021)).unwrap();
    fx.scan_and_match().await;
    assert_eq!(fx.search("dune").await.movies, []);
}
