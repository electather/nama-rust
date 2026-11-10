//! Browsing Movies and Shows through the generated `BrowseService` client, the Watch State that
//! `WatchStateService` reports or marks and browsing shows, and playing their Versions, over a
//! real socket, with the TMDB Plugin enabled against a fake TMDB.

#[path = "support/http.rs"]
mod http;
#[path = "support/refused.rs"]
mod refused;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    slice,
    sync::Arc,
    time::Duration,
};

use buffa::EnumValue;
use connectrpc::{
    ConnectError, ErrorCode,
    client::{ClientConfig, HttpClient},
};
use http::{fetch, fetch_range};
use nama_proto::nama::v1::{
    AudioTrack, BrowseServiceClient, ClearMatchRequest, ContinueWatchingEntry,
    DeleteLibraryRequest, DeviceDecoding, Episode, EventType, ExternalId, GetMovieRequest,
    GetShowRequest, GetShowResponse, HdrFormat, LibraryKind, LibraryServiceClient,
    ListContinueWatchingRequest, ListEpisodesRequest, ListMoviesRequest, ListShowsRequest,
    MatchServiceClient, Movie, PlanPlaybackRequest, Playback, PlaybackEnded, PlaybackLimit,
    PlaybackMethod, PlaybackPlan, PlaybackServiceClient, PlaybackSettings, PlaybackStarted,
    RefusalReason, ReportProgressRequest, Season, ServerServiceClient, SetMatchRequest,
    SetPlayedRequest, Show, ShowSeason, StopPlaybackRequest, Track, UpdatePlaybackSettingsRequest,
    Version, VideoTrack, WatchState, WatchStateServiceClient, continue_watching_entry, external_id,
    set_played_request::Target,
};
use nama_server::{
    config::Config,
    contents::Contents,
    ffmpeg::{self, Ffmpeg, FoundProgram},
};
use refused::assert_refused;
use server::{
    Builder, Library, SignedIn, TestServer, WAIT, subscribe,
    tmdb::{ALIEN, DARK, THE_OFFICE as THE_OFFICE_US},
};

/// TMDB Matches both to Alien.
const ALIEN_1080P: &str = "Alien (1979)/Alien (1979) - 1080p.mkv";
const ALIEN_2160P: &str = "Alien (1979)/Alien (1979) - 2160p.mkv";

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

/// Two hours, in milliseconds: the length a test's player reports.
const TWO_HOURS: u64 = 7_200_000;
/// An hour, in milliseconds: the length a test's player reports for the two-Episode file.
const AN_HOUR: u64 = 3_600_000;
const MINUTE: u64 = 60_000;

/// A Server with one Library and TMDB on, and the clients a test browses and fixes Matches with.
struct Fixture {
    library: i64,
    folder: PathBuf,
    browse: BrowseServiceClient<HttpClient>,
    matches: MatchServiceClient<HttpClient>,
    contents: Contents,
    server: TestServer,
    media: Library,
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
impl Fixture {
    /// Starts a Server with a Library of `kind`, and no ffmpeg.
    async fn start(kind: LibraryKind) -> Self {
        Self::start_with_ffmpeg(kind, Ffmpeg::default()).await
    }

    /// Starts a Server with a Library of `kind` that found `ffmpeg` at startup.
    async fn start_with_ffmpeg(kind: LibraryKind, ffmpeg: Ffmpeg) -> Self {
        Self::start_on(kind, TestServer::builder().ffmpeg(ffmpeg)).await
    }

    /// Starts a Server as `builder` says, with a fake TMDB and a Library of `kind`.
    async fn start_on(kind: LibraryKind, builder: Builder) -> Self {
        let server = builder.tmdb().start().await;
        let media = server.create_library(kind, &[]).await;
        Self::over(server, media)
    }

    /// The clients for browsing `media` on `server`.
    fn over(server: TestServer, media: Library) -> Self {
        let config = server.client_config();
        Self {
            library: media.id,
            folder: media.folder().to_owned(),
            browse: BrowseServiceClient::new(HttpClient::plaintext(), config.clone()),
            matches: MatchServiceClient::new(HttpClient::plaintext(), config),
            contents: Contents::new(Arc::clone(server.db())),
            server,
            media,
        }
    }

    /// Starts a Server with a Shows Library holding Dark and The Office, scanned and Matched.
    async fn shows() -> Self {
        let fx = Self::start(LibraryKind::LIBRARY_KIND_SHOWS).await;
        fx.write(DARK_S01E01_E02, b"two episodes");
        fx.write(DARK_S01E01, b"one episode");
        fx.write(DARK_S02E01, b"video");
        fx.write(THE_OFFICE, b"video");
        fx.scan_until_decided(4).await;
        fx
    }

    /// Starts a Server with the ffmpeg on `PATH` and a Movies Library holding Alien, with English
    /// SDH and French Forced SRT subtitles, scanned and Matched.
    async fn subtitled_alien() -> Self {
        Self::alien_written_by(ALIEN_1080P, |path| {
            write_english_and_french_srt(path, "Bonjour");
        })
        .await
    }

    /// Starts a Server with the ffmpeg on `PATH` and a Movies Library holding Alien as `file`,
    /// which `write` writes, scanned and Matched.
    async fn alien_written_by(file: &str, write: impl FnOnce(&Path)) -> Self {
        let path = std::env::var_os("PATH").unwrap_or_default();
        let ffmpeg = Ffmpeg::find(&Config::default(), &path);
        assert!(
            ffmpeg.direct_play_only_because().is_none(),
            "this test needs ffmpeg and ffprobe on PATH"
        );
        let fx = Self::start_with_ffmpeg(LibraryKind::LIBRARY_KIND_MOVIES, ffmpeg).await;
        let file = fx.folder.join(file);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        write(&file);
        fx.scan_until_decided(1).await;
        fx
    }

    /// Fetches Alien's subtitles, so the Server caches them.
    async fn cache_subtitles(&self) {
        let version = *self.version_ids(ALIEN).await.first().unwrap();
        let token = Some(self.server.admin_token.as_str());
        for subtitle in self.plan(version).await.unwrap().subtitles {
            assert_eq!(fetch(&self.server, &subtitle.url, token).await.status, 200);
        }
        assert_ne!(self.cached_subtitles(), Vec::<PathBuf>::new());
    }

    /// What the Server's subtitle cache holds a folder for, as the Versions it caches.
    fn cached_subtitles(&self) -> Vec<PathBuf> {
        let versions = self.server.data_dir().join("subtitles/versions");
        fs::read_dir(versions).map_or_else(
            |_| Vec::new(),
            |entries| entries.map(|entry| entry.unwrap().path()).collect(),
        )
    }

    /// Starts a Server with a Movies Library holding Alien, scanned and Matched.
    async fn alien() -> Self {
        Self::alien_on(TestServer::builder()).await
    }

    /// [`Self::alien`] on a Server `builder` starts.
    async fn alien_on(builder: Builder) -> Self {
        let server = builder.tmdb().start().await;
        let media = server
            .matched_library(LibraryKind::LIBRARY_KIND_MOVIES, &[ALIEN_1080P])
            .await;
        Self::over(server, media)
    }

    fn write(&self, relative: &str, contents: &[u8]) {
        self.media.write(relative, contents);
    }

    /// Scans the Library, and waits until `files` files are stored and Matching has decided
    /// about all their items.
    async fn scan_until_decided(&self, files: usize) {
        self.scan_until(self.library, async || {
            self.all_decided(self.library, files).await
        })
        .await;
    }

    /// Scans the Library `library`, and waits until `done` says so.
    async fn scan_until(&self, library: i64, mut done: impl AsyncFnMut() -> bool) {
        self.server.scan(library).await;
        tokio::time::timeout(WAIT, async {
            while !done().await {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
    }

    /// Whether `files` files are stored for the Library `library`, and Matching has decided about
    /// every item they hold.
    async fn all_decided(&self, library: i64, files: usize) -> bool {
        let stored = self.contents.files(library).await.unwrap();
        stored.len() == files
            && stored
                .iter()
                .flat_map(|file| &file.items)
                .all(|item| item.recorded.is_some())
    }

    /// Rescans the Library after the file at `relative`, which held the item `before`, changed,
    /// and waits until a new item has replaced it and Matching has decided about that.
    async fn rescan_until_replaced(&self, relative: &str, before: i64) {
        self.scan_until(self.library, async || self.replaced(relative, before).await)
            .await;
    }

    /// Whether the file at `relative` holds an item other than `before`, which Matching has
    /// decided about.
    async fn replaced(&self, relative: &str, before: i64) -> bool {
        let path = self.folder.join(relative);
        let files = self.contents.files(self.library).await.unwrap();
        let file = files.iter().find(|file| path == *file.path).unwrap();
        let item = file.items.first().unwrap();
        item.id != before && item.recorded.is_some()
    }

    /// The Admin, browsing and playing.
    fn admin(&self) -> Viewer {
        Viewer::new(self.server.client_config())
    }

    /// The Admin's Playback Plan for the Version `version`, on [`chrome`].
    async fn plan(&self, version: i64) -> Result<PlaybackPlan, ConnectError> {
        self.plan_on_device(version, &[], &[], 0).await
    }

    /// As [`Self::plan`], on a Device that also decodes the content types `also_decodes`, where
    /// the methods `failed` just failed to play it, and which plays `audio_channels` channels at
    /// once.
    async fn plan_on_device(
        &self,
        version: i64,
        also_decodes: &[&str],
        failed: &[PlaybackMethod],
        audio_channels: u32,
    ) -> Result<PlaybackPlan, ConnectError> {
        let mut device = chrome();
        (device.content_types).extend(also_decodes.iter().map(|&kind| kind.to_owned()));
        device.audio_channels = audio_channels;
        self.ask_for_plan(PlanPlaybackRequest {
            version_id: version,
            device_decoding: device.into(),
            failed_methods: failed.iter().copied().map(EnumValue::from).collect(),
            ..Default::default()
        })
        .await
    }

    /// As [`Self::plan`], from `start_position_ms`, with the audio Track at the place
    /// `audio_track` in the Version's Tracks.
    async fn plan_with_audio(
        &self,
        version: i64,
        audio_track: u32,
        start_position_ms: u64,
    ) -> Result<PlaybackPlan, ConnectError> {
        self.ask_for_plan(PlanPlaybackRequest {
            version_id: version,
            start_position_ms,
            audio_track: Some(audio_track),
            device_decoding: chrome().into(),
            ..Default::default()
        })
        .await
    }

    async fn ask_for_plan(
        &self,
        request: PlanPlaybackRequest,
    ) -> Result<PlaybackPlan, ConnectError> {
        ask_for_plan_as(self.server.client_config(), request).await
    }

    /// As [`Self::ask_for_plan`], through a proxy that says it relays for the address `client`.
    async fn ask_for_plan_relayed_for(
        &self,
        client: &str,
        request: PlanPlaybackRequest,
    ) -> Result<PlaybackPlan, ConnectError> {
        let config = (self.server.client_config()).with_default_header("x-forwarded-for", client);
        ask_for_plan_as(config, request).await
    }

    /// Sets the default Playback Limit for remote playback to `limit`, as the Admin.
    async fn set_remote_limit(&self, limit: PlaybackLimit) {
        ServerServiceClient::new(HttpClient::plaintext(), self.server.client_config())
            .update_playback_settings(UpdatePlaybackSettingsRequest {
                settings: PlaybackSettings {
                    remote_playback_limit: limit.into(),
                    max_transcodes: Some(2),
                    ..Default::default()
                }
                .into(),
                ..Default::default()
            })
            .await
            .unwrap();
    }

    /// The ids of the Versions of the Movie with the TMDB id `tmdb`, by path.
    async fn version_ids(&self, tmdb: i64) -> Vec<i64> {
        let movie = self.get_movie(self.library, tmdb).await.unwrap();
        movie.versions.iter().map(|version| version.id).collect()
    }

    /// The id of the first item stored for the file at `relative`.
    async fn item(&self, relative: &str) -> i64 {
        let path = self.folder.join(relative);
        let files = self.contents.files(self.library).await.unwrap();
        let file = files.into_iter().find(|file| path == *file.path).unwrap();
        file.items.first().unwrap().id
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

    /// The Movie of the Library `library` with the TMDB id `tmdb`, as `GetMovie` gives it.
    async fn get_movie(&self, library: i64, tmdb: i64) -> Result<Movie, ConnectError> {
        let response = self
            .browse
            .get_movie(GetMovieRequest {
                library_id: library,
                movie_id: tmdb_id(tmdb).into(),
                ..Default::default()
            })
            .await?
            .into_owned();
        Ok(response.movie.into_option().unwrap())
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

fn tmdb_id(id: i64) -> ExternalId {
    ExternalId {
        id: Some(external_id::Id::Tmdb(id)),
        ..Default::default()
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
    fx.scan_until_decided(3).await;

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
async fn a_version_has_the_tracks_probing_found_once_a_scan_has_probed_it() {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let ffmpeg = Ffmpeg::find(&Config::default(), &path);
    assert!(ffmpeg.ffprobe.is_ok(), "this test needs ffprobe on PATH");
    let fx = Fixture::start_with_ffmpeg(LibraryKind::LIBRARY_KIND_MOVIES, ffmpeg).await;
    let file = fx.folder.join(ALIEN_2160P);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    ffmpeg::write_4k_with_5_1(&file, "smpte2084").unwrap();

    fx.scan_until(fx.library, async || {
        fx.get_movie(fx.library, ALIEN).await.is_ok_and(|movie| {
            movie
                .versions
                .iter()
                .all(|version| !version.tracks.is_empty())
        })
    })
    .await;

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
    let item = fx.item(ALIEN_1080P).await;
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
    let item = fx.item(DARK_S02E01).await;
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

/// Where a browser's `<video>` plays the Version `version` from.
fn version_url(version: &Version) -> String {
    format!("/media/versions/{}", version.id)
}

#[tokio::test]
async fn a_version_is_served_whole_with_what_a_video_element_needs() {
    let fx = Fixture::alien().await;
    let movie = fx.get_movie(fx.library, ALIEN).await.unwrap();

    let reply = fetch(
        &fx.server,
        &version_url(&movie.versions[0]),
        Some(&fx.server.admin_token),
    )
    .await;

    assert_eq!(reply.status, 200);
    assert_eq!(reply.body, b"video");
    assert_eq!(reply.header("content-type"), "video/x-matroska");
    assert_eq!(reply.header("accept-ranges"), "bytes");
    assert_ne!(reply.header("last-modified"), "");
}

/// A `<video>` seeks by asking for a range of bytes.
#[tokio::test]
async fn a_range_of_a_version_is_served_as_exactly_those_bytes() {
    let fx = Fixture::alien().await;
    let movie = fx.get_movie(fx.library, ALIEN).await.unwrap();

    let reply = fetch_range(
        &fx.server,
        &version_url(&movie.versions[0]),
        Some(&fx.server.admin_token),
        Some("bytes=1-3"),
    )
    .await;

    assert_eq!(reply.status, 206);
    assert_eq!(reply.body, b"ide");
    assert_eq!(reply.header("content-range"), "bytes 1-3/5");
}

/// The session cookie is what lets a browser's `<video src>` play a Version (ADR 0014).
#[tokio::test]
async fn a_version_without_a_session_is_refused() {
    let fx = Fixture::alien().await;
    let movie = fx.get_movie(fx.library, ALIEN).await.unwrap();

    let reply = fetch(&fx.server, &version_url(&movie.versions[0]), None).await;

    assert_eq!(reply.status, 401);
}

#[tokio::test]
async fn an_unknown_version_is_not_found() {
    let fx = Fixture::alien().await;

    let reply = fetch(
        &fx.server,
        "/media/versions/999",
        Some(&fx.server.admin_token),
    )
    .await;

    assert_eq!(reply.status, 404);
}

/// A stored file is a Version only while it is Matched.
#[tokio::test]
async fn a_file_whose_match_was_cleared_is_not_found() {
    let fx = Fixture::alien().await;
    let movie = fx.get_movie(fx.library, ALIEN).await.unwrap();
    let item = fx.item(ALIEN_1080P).await;
    fx.matches
        .clear_match(ClearMatchRequest {
            item_id: item,
            ..Default::default()
        })
        .await
        .unwrap();

    let reply = fetch(
        &fx.server,
        &version_url(&movie.versions[0]),
        Some(&fx.server.admin_token),
    )
    .await;

    assert_eq!(reply.status, 404);
}

/// Versions are served by id, so a client can't name a file by its path.
#[tokio::test]
async fn a_path_in_place_of_a_version_id_is_a_bad_request() {
    let fx = Fixture::alien().await;

    let reply = fetch(
        &fx.server,
        "/media/versions/..%2F..%2Fnama.db",
        Some(&fx.server.admin_token),
    )
    .await;

    assert_eq!(reply.status, 400);
}

#[tokio::test]
async fn a_remux_writes_its_segments_to_the_segment_folder_an_admin_set() {
    let (fx, version) = Fixture::alien_mkv(system_ffmpeg(), "aac").await;
    let tmpfs = tempfile::tempdir().unwrap();
    ServerServiceClient::new(HttpClient::plaintext(), fx.server.client_config())
        .update_playback_settings(UpdatePlaybackSettingsRequest {
            settings: PlaybackSettings {
                segment_folder: tmpfs.path().to_str().unwrap().to_owned(),
                max_transcodes: Some(2),
                ..Default::default()
            }
            .into(),
            ..Default::default()
        })
        .await
        .unwrap();

    let plan = fx.plan(version).await.unwrap();

    ffmpegs_own_playlist(tmpfs.path(), &plan.url).await;
}

/// A Server whose ffprobe, a shell script in `bin`, reads every file as an H.264 + AAC MP4.
#[cfg(unix)]
fn ffprobe_reading_h264_and_aac(bin: &Path) -> Ffmpeg {
    let ffprobe = r#"#!/bin/sh
cat <<'EOF'
{"streams": [{"codec_name": "h264", "codec_type": "video"},
             {"codec_name": "aac", "codec_type": "audio", "disposition": {"default": 1}}],
 "format": {"format_name": "mov,mp4,m4a,3gp,3g2,mj2"}}
EOF
"#;
    Ffmpeg {
        ffprobe: Ok(script(bin, "ffprobe", ffprobe)),
        ..Ffmpeg::default()
    }
}

/// The codecs of `plan`'s Tracks.
fn codecs(plan: &PlaybackPlan) -> Vec<&str> {
    plan.tracks
        .iter()
        .map(|track| track.codec.as_str())
        .collect()
}

// The fake ffprobe is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn an_h264_and_aac_mp4_direct_plays_through_its_plans_url() {
    let bin = tempfile::tempdir().unwrap();
    let ffmpeg = ffprobe_reading_h264_and_aac(bin.path());
    let fx = Fixture::start_with_ffmpeg(LibraryKind::LIBRARY_KIND_MOVIES, ffmpeg).await;
    fx.write("Alien (1979)/Alien (1979).mp4", b"video");
    fx.scan_until_decided(1).await;
    let version = fx.version_ids(ALIEN).await[0];

    let plan = fx.plan(version).await.unwrap();

    assert_eq!(plan.method, PlaybackMethod::PLAYBACK_METHOD_DIRECT_PLAY);
    assert_eq!(codecs(&plan), ["h264", "aac"]);
    assert_eq!(
        plan.reasons,
        [
            "Its Tracks need no change",
            "This Device decodes its container and Tracks",
            "This Server lacks ffmpeg or ffprobe, so it can only Direct Play",
            "No hardware acceleration passed its test when this Server started, so a Transcode \
             runs in software",
            "Nothing has failed to play it on this Device",
            "It plays the file's own audio Track",
            "This Device is on the local network",
            "The Playback Limit is Original",
            "No subtitles are burned into its picture",
            "This Server isn't running as many Transcodes as it may at once",
        ]
    );
    let played = fetch(&fx.server, &plan.url, Some(&fx.server.admin_token)).await;
    assert_eq!(played.body, b"video");
}

/// A Server whose ffmpeg and ffprobe are shell scripts in `bin`: ffprobe reads a file whose name
/// says 2160p as an HEVC and AAC MKV and any other as an H.264 and AAC one, and ffmpeg writes
/// nothing until it's stopped.
#[cfg(unix)]
fn ffmpeg_writing_nothing(bin: &Path) -> Ffmpeg {
    let ffprobe = r#"#!/bin/sh
case "$*" in *2160p*) video=hevc ;; *) video=h264 ;; esac
cat <<EOF
{"streams": [{"codec_name": "$video", "codec_type": "video"},
             {"codec_name": "aac", "codec_type": "audio", "disposition": {"default": 1}}],
 "format": {"format_name": "matroska,webm"}}
EOF
"#;
    Ffmpeg {
        ffmpeg: Ok(script(bin, "ffmpeg", "#!/bin/sh\nexec sleep 600\n")),
        ffprobe: Ok(script(bin, "ffprobe", ffprobe)),
    }
}

/// The program `name`, the shell script `body` written to `bin`.
#[cfg(unix)]
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn script(bin: &Path, name: &str, body: &str) -> FoundProgram {
    use std::os::unix::fs::PermissionsExt;

    let path = bin.join(name);
    fs::write(&path, body).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    FoundProgram {
        path,
        version: "7.1.1".to_owned(),
    }
}

#[cfg(unix)]
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
impl Fixture {
    /// Starts a Server with [`ffmpeg_writing_nothing`] in `bin` that runs one Transcode at once,
    /// and a Movies Library holding Alien in 1080p and 2160p, and returns it with the ids of those
    /// Versions, which Chrome Remuxes and Transcodes.
    async fn alien_with_room_for_one_transcode(bin: &Path) -> (Self, [i64; 2]) {
        let fx = Self::start_with_ffmpeg(
            LibraryKind::LIBRARY_KIND_MOVIES,
            ffmpeg_writing_nothing(bin),
        )
        .await;
        fx.write(ALIEN_1080P, b"video");
        fx.write(ALIEN_2160P, b"video");
        fx.scan_until_decided(2).await;
        ServerServiceClient::new(HttpClient::plaintext(), fx.server.client_config())
            .update_playback_settings(UpdatePlaybackSettingsRequest {
                settings: PlaybackSettings {
                    max_transcodes: Some(1),
                    ..Default::default()
                }
                .into(),
                ..Default::default()
            })
            .await
            .unwrap();
        let versions = fx.version_ids(ALIEN).await.try_into().unwrap();
        (fx, versions)
    }

    /// The Admin's Device's Transcode of the Version `version`.
    async fn transcode(&self, version: i64) -> PlaybackPlan {
        let plan = self.plan(version).await.unwrap();
        assert_eq!(plan.method, PlaybackMethod::PLAYBACK_METHOD_TRANSCODE);
        plan
    }
}

/// `who`'s Playback Plan for the Version `version`, on their own Device, which is [`chrome`].
#[cfg(unix)]
async fn plan_as(who: &SignedIn, version: i64) -> Result<PlaybackPlan, ConnectError> {
    let request = PlanPlaybackRequest {
        version_id: version,
        device_decoding: chrome().into(),
        ..Default::default()
    };
    ask_for_plan_as(who.client_config(), request).await
}

// The fake ffmpeg and ffprobe are shell scripts.
#[cfg(unix)]
#[tokio::test]
async fn with_the_cap_at_1_a_second_device_that_needs_a_transcode_is_told_the_server_is_busy() {
    let bin = tempfile::tempdir().unwrap();
    let (fx, [_, hevc]) = Fixture::alien_with_room_for_one_transcode(bin.path()).await;
    let bishop = fx.server.user_with_default_permissions("Bishop").await;
    fx.transcode(hevc).await;

    let planned = plan_as(&bishop, hevc).await;

    let refusal = assert_refused(
        planned,
        ErrorCode::ResourceExhausted,
        RefusalReason::REFUSAL_REASON_SERVER_BUSY,
    );
    assert_eq!(
        refusal.params.get("maxTranscodes").map(String::as_str),
        Some("1")
    );
}

// The fake ffmpeg and ffprobe are shell scripts.
#[cfg(unix)]
#[tokio::test]
async fn with_the_cap_full_a_device_that_plays_a_remux_gets_one_and_the_plan_says_why() {
    let bin = tempfile::tempdir().unwrap();
    let (fx, [h264, hevc]) = Fixture::alien_with_room_for_one_transcode(bin.path()).await;
    let bishop = fx.server.user_with_default_permissions("Bishop").await;
    fx.transcode(hevc).await;

    let plan = plan_as(&bishop, h264).await.unwrap();

    assert_eq!(plan.method, PlaybackMethod::PLAYBACK_METHOD_REMUX);
    assert_eq!(
        plan.reasons.last().map(String::as_str),
        Some(
            "This Server is busy, running as many Transcodes as it may at once (1), so it can't \
             Transcode it"
        )
    );
}

// The fake ffmpeg and ffprobe are shell scripts.
#[cfg(unix)]
#[tokio::test]
async fn when_a_transcodes_session_ends_its_slot_frees_up_for_the_next() {
    let bin = tempfile::tempdir().unwrap();
    let (fx, [_, hevc]) = Fixture::alien_with_room_for_one_transcode(bin.path()).await;
    let bishop = fx.server.user_with_default_permissions("Bishop").await;
    let admins = fx.transcode(hevc).await;

    PlaybackServiceClient::new(HttpClient::plaintext(), fx.server.client_config())
        .stop_playback(StopPlaybackRequest {
            plan_id: admins.id,
            ..Default::default()
        })
        .await
        .unwrap();

    let plan = plan_as(&bishop, hevc).await.unwrap();
    assert_eq!(plan.method, PlaybackMethod::PLAYBACK_METHOD_TRANSCODE);
}

// ADR 0026.
#[tokio::test]
async fn without_ffprobe_a_version_direct_plays_and_its_plan_says_its_tracks_arent_known() {
    let fx = Fixture::alien().await;
    let version = fx.version_ids(ALIEN).await[0];

    let plan = fx.plan(version).await.unwrap();

    assert_eq!(plan.method, PlaybackMethod::PLAYBACK_METHOD_DIRECT_PLAY);
    assert_eq!(plan.url, format!("/media/versions/{version}"));
    assert_eq!(plan.tracks, []);
    assert_eq!(
        plan.reasons,
        [
            "Its Tracks aren't known, so they're taken to need no change",
            "Its Tracks aren't known, so this Device is taken to decode them",
            "This Server lacks ffmpeg or ffprobe, so it can only Direct Play",
            "No hardware acceleration passed its test when this Server started, so a Transcode \
             runs in software",
            "Nothing has failed to play it on this Device",
            "It plays the file's own audio Track",
            "This Device is on the local network",
            "The Playback Limit is Original",
            "No subtitles are burned into its picture",
            "This Server isn't running as many Transcodes as it may at once",
        ]
    );
}

#[tokio::test]
async fn each_plan_tells_subscribers_its_playback_started() {
    let fx = Fixture::alien().await;
    let version = fx.version_ids(ALIEN).await[0];
    let started = [EventType::EVENT_TYPE_PLAYBACK_STARTED];
    let mut subscription = subscribe(&fx.server.client_config(), &started).await;

    let first = fx.plan(version).await.unwrap();
    let replacing = fx.plan(version).await.unwrap();

    for plan in [first, replacing] {
        let told = PlaybackStarted {
            playback: admins_playback(version, plan).into(),
            ..Default::default()
        };
        assert_eq!(subscription.next().await, told.into());
    }
}

#[tokio::test]
async fn a_direct_play_its_player_stops_tells_subscribers_its_playback_ended() {
    let fx = Fixture::alien().await;
    let version = fx.version_ids(ALIEN).await[0];
    let config = fx.server.client_config();
    let ended = [EventType::EVENT_TYPE_PLAYBACK_ENDED];
    let mut subscription = subscribe(&config, &ended).await;
    let plan = fx.plan(version).await.unwrap();
    assert_eq!(plan.method, PlaybackMethod::PLAYBACK_METHOD_DIRECT_PLAY);

    PlaybackServiceClient::new(HttpClient::plaintext(), config)
        .stop_playback(StopPlaybackRequest {
            plan_id: plan.id,
            ..Default::default()
        })
        .await
        .unwrap();

    let told = PlaybackEnded {
        playback: admins_playback(version, plan).into(),
        ..Default::default()
    };
    assert_eq!(subscription.next().await, told.into());
}

/// What Observers are told of the Admin playing the Version `version` by `plan`.
fn admins_playback(version: i64, plan: PlaybackPlan) -> Playback {
    Playback {
        // The first User and Device in a fresh database: the Admin's.
        user_id: 1,
        device_id: 1,
        version_id: version,
        method: plan.method,
        tracks: plan.tracks,
        reasons: plan.reasons,
        ..Default::default()
    }
}

/// What Chrome says it decodes, in part: H.264 and AAC in MP4, and HLS through Media Source
/// Extensions.
fn chrome() -> DeviceDecoding {
    let content_types = [
        "video/mp4",
        r#"video/mp4; codecs="avc1.640028""#,
        r#"audio/mp4; codecs="mp4a.40.2""#,
    ];
    DeviceDecoding {
        content_types: content_types.map(str::to_owned).to_vec(),
        mse: true,
        ..Default::default()
    }
}

/// The Playback Plan `request` asks for, asked as `who`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn ask_for_plan_as(
    who: ClientConfig,
    request: PlanPlaybackRequest,
) -> Result<PlaybackPlan, ConnectError> {
    let response = PlaybackServiceClient::new(HttpClient::plaintext(), who)
        .plan_playback(request)
        .await?
        .into_owned();
    Ok(response.plan.into_option().unwrap())
}

/// The ffmpeg and ffprobe on `PATH`, which these tests need.
fn system_ffmpeg() -> Ffmpeg {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let ffmpeg = Ffmpeg::find(&Config::default(), &path);
    assert!(
        ffmpeg.ffmpeg.is_ok() && ffmpeg.ffprobe.is_ok(),
        "this test needs ffmpeg and ffprobe on PATH"
    );
    ffmpeg
}

#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "a test helper fails its test by panicking"
)]
impl Fixture {
    /// Starts a Server with `ffmpeg` and a Movies Library holding Alien as an MKV of H.264 and 5.1
    /// audio encoded with `audio`, as [`ffmpeg::write_h264_with_5_1`] takes it, and returns it with
    /// that Version's id.
    async fn alien_mkv(ffmpeg: Ffmpeg, audio: &str) -> (Self, i64) {
        Self::alien_written(ffmpeg, |file| ffmpeg::write_h264_with_5_1(file, audio)).await
    }

    /// Starts a Server with `ffmpeg` and a Movies Library holding Alien as an MKV `write` writes,
    /// and returns it with that Version's id.
    async fn alien_written(
        ffmpeg: Ffmpeg,
        write: impl FnOnce(&Path) -> std::io::Result<()>,
    ) -> (Self, i64) {
        Self::alien_written_on(TestServer::builder().ffmpeg(ffmpeg), write).await
    }

    /// [`Self::alien_written`] on a Server `builder` starts.
    async fn alien_written_on(
        builder: Builder,
        write: impl FnOnce(&Path) -> std::io::Result<()>,
    ) -> (Self, i64) {
        let fx = Self::start_on(LibraryKind::LIBRARY_KIND_MOVIES, builder).await;
        let file = fx.folder.join("Alien (1979)/Alien (1979).mkv");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        write(&file).unwrap();
        fx.scan_until_decided(1).await;
        let version = fx.version_ids(ALIEN).await[0];
        (fx, version)
    }

    /// The body of `path` on the Server, fetched as the Admin, which must be served.
    async fn fetch_ok(&self, path: &str) -> Vec<u8> {
        let reply = fetch(&self.server, path, Some(&self.server.admin_token)).await;
        assert_eq!(reply.status, 200, "{path}");
        reply.body
    }
}

/// Each segment's duration in a playlist, as written, and the files it lists.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn segments(playlist: &str) -> (Vec<f64>, Vec<&str>) {
    let durations = (playlist.lines())
        .filter_map(|line| line.strip_prefix("#EXTINF:"))
        .map(|rest| rest.trim_end_matches(',').parse().unwrap())
        .collect();
    let files = (playlist.lines())
        .filter(|line| !line.starts_with('#'))
        .collect();
    (durations, files)
}

/// `url` with its last part replaced by `file`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn beside(url: &str, file: &str) -> String {
    let (folder, _) = url.rsplit_once('/').unwrap();
    format!("{folder}/{file}")
}

/// The HLS playlist ffmpeg wrote to the segment folder `folder` for the plan at `url`, once it's
/// done.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn ffmpegs_own_playlist(folder: &Path, url: &str) -> String {
    let id = url.split('/').nth(3).unwrap();
    let path = folder.join(format!("nama-plan-{id}")).join("ffmpeg.m3u8");
    tokio::time::timeout(WAIT, async {
        loop {
            if let Ok(playlist) = fs::read_to_string(&path) {
                return playlist;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn an_h264_and_aac_mkv_is_remuxed_over_hls_with_its_audio_copied() {
    let (fx, version) = Fixture::alien_mkv(system_ffmpeg(), "aac").await;

    let plan = fx.plan(version).await.unwrap();

    assert_eq!(plan.method, PlaybackMethod::PLAYBACK_METHOD_REMUX);
    assert_eq!(codecs(&plan), ["h264", "aac"]);
    assert_eq!(
        plan.reasons[1],
        "This Device can't decode its Matroska container, so it can't be Direct Played"
    );
    let reply = fetch(&fx.server, &plan.url, Some(&fx.server.admin_token)).await;
    assert_eq!(
        reply.header("content-type"),
        "application/vnd.apple.mpegurl"
    );
    let playlist = String::from_utf8(reply.body).unwrap();
    assert!(
        playlist.contains("#EXT-X-MAP:URI=\"init.mp4\""),
        "{playlist}"
    );
    assert!(playlist.ends_with("#EXT-X-ENDLIST\n"), "{playlist}");
    let (durations, files) = segments(&playlist);
    assert_eq!(files, ["0.m4s", "1.m4s", "2.m4s"]);
    let init = fx.fetch_ok(&beside(&plan.url, "init.mp4")).await;
    assert_eq!(
        init.get(4..8),
        Some(&b"ftyp"[..]),
        "an MP4 starts with its type"
    );
    for file in files {
        let segment = fx.fetch_ok(&beside(&plan.url, file)).await;
        assert_ne!(segment.len(), 0, "{file}");
    }
    assert_eq!(
        played_audio(&fx, &plan.url, "0.m4s").await,
        "aac,6",
        "copied, though this Device doesn't say it plays 5.1"
    );
    // HLS time is the file's time, so Watch State needs no conversion: the playlist cuts where
    // ffmpeg cut a file per keyframe, from the start of the file to its end.
    let default = fx.server.data_dir().join("transcodes");
    let (ffmpegs, _) = segments(&ffmpegs_own_playlist(&default, &plan.url).await);
    let ffmpegs_cuts = cuts(&ffmpegs);
    for cut in cuts(&durations) {
        assert!(
            ffmpegs_cuts.iter().any(|theirs| (cut - theirs).abs() < 0.1),
            "{durations:?} against {ffmpegs:?}"
        );
    }
}

/// Where segments lasting `durations` end.
fn cuts(durations: &[f64]) -> Vec<f64> {
    let ends = durations.iter().scan(0.0, |end, duration| {
        *end += duration;
        Some(*end)
    });
    ends.collect()
}

/// A Server whose ffprobe, a shell script in `bin`, writes the arguments of each run to `log`
/// and then runs the ffprobe on `PATH`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
#[cfg(unix)]
fn ffprobe_logging_its_runs(bin: &Path, log: &Path) -> Ffmpeg {
    let ffmpeg = system_ffmpeg();
    let real = ffmpeg.ffprobe.unwrap().path;
    let ffprobe = format!(
        "#!/bin/sh\necho \"$*\" >> '{}'\nexec '{}' \"$@\"\n",
        log.display(),
        real.display()
    );
    Ffmpeg {
        ffprobe: Ok(script(bin, "ffprobe", &ffprobe)),
        ..ffmpeg
    }
}

// The logging ffprobe is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn remuxing_a_version_again_reuses_the_keyframes_listed_the_first_time() {
    let bin = tempfile::tempdir().unwrap();
    let log = bin.path().join("runs.log");
    let (fx, version) = Fixture::alien_mkv(ffprobe_logging_its_runs(bin.path(), &log), "aac").await;

    let first = fx.plan(version).await.unwrap();
    let second = fx.plan(version).await.unwrap();

    assert_ne!(first.url, second.url, "each plan runs on its own");
    assert_eq!(
        fx.fetch_ok(&first.url).await,
        fx.fetch_ok(&second.url).await
    );
    let runs = fs::read_to_string(&log).unwrap();
    let listings = runs.lines().filter(|run| run.contains("packet=pts_time"));
    assert_eq!(listings.count(), 1, "{runs}");
}

/// As for Direct Play, the session cookie is what lets a browser's HLS player fetch them.
#[tokio::test]
async fn a_remuxs_playlist_and_segments_without_a_session_are_refused() {
    let (fx, version) = Fixture::alien_mkv(system_ffmpeg(), "aac").await;
    let plan = fx.plan(version).await.unwrap();

    for path in [plan.url.clone(), beside(&plan.url, "0.m4s")] {
        let reply = fetch(&fx.server, &path, None).await;

        assert_eq!(reply.status, 401, "{path}");
    }
}

/// The codec and channels of the audio the plan at `url` plays in its segment `segment`, such as
/// `aac,6`, as ffprobe reads it with its init segment.
async fn played_audio(fx: &Fixture, url: &str, segment: &str) -> String {
    played_streams(fx, url, "a", segment, "codec_name,channels").await
}

/// The `entries` of the streams of type `kind`, as ffprobe's `-select_streams` and
/// `-show_entries stream=` take them, that the plan at `url` plays in its segment `segment`, as
/// ffprobe reads it with its init segment.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn played_streams(
    fx: &Fixture,
    url: &str,
    kind: &str,
    segment: &str,
    entries: &str,
) -> String {
    // The segment first, so ffmpeg starts there and the init segment is that run's.
    let segment = fx.fetch_ok(&beside(url, segment)).await;
    let mut mp4 = fx.fetch_ok(&beside(url, "init.mp4")).await;
    mp4.extend(segment);
    let file = tempfile::NamedTempFile::new().unwrap();
    fs::write(file.path(), mp4).unwrap();
    let output = std::process::Command::new("ffprobe")
        .args(["-v", "error", "-select_streams", kind])
        .args([
            "-show_entries",
            &format!("stream={entries}"),
            "-of",
            "csv=p=0",
        ])
        .arg(file.path())
        .output()
        .unwrap();
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

#[tokio::test]
async fn dts_is_remuxed_as_aac_with_up_to_as_many_channels_as_the_device_plays() {
    let (fx, version) = Fixture::alien_mkv(system_ffmpeg(), "dca").await;

    for (plays, converted, to) in [
        (6, "aac,6", "5.1 AAC"),
        (2, "aac,2", "stereo AAC, as this Device doesn't play 5.1"),
    ] {
        let plan = fx.plan_on_device(version, &[], &[], plays).await.unwrap();

        assert_eq!(plan.method, PlaybackMethod::PLAYBACK_METHOD_REMUX);
        assert_eq!(codecs(&plan), ["h264", "aac"]);
        assert_eq!(
            plan.reasons[1],
            format!(
                "This Device can't decode its Matroska container or DTS audio, so it can't be \
                 Direct Played, and a Remux converts that audio to {to}"
            )
        );
        assert_eq!(played_audio(&fx, &plan.url, "0.m4s").await, converted);
    }
}

/// Writes 14 seconds of H.264 with English 5.1 AAC, played by default, and a stereo AAC
/// commentary, as Matroska, to `path`, with the ffmpeg on `PATH`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn write_english_and_a_stereo_commentary(path: &Path) {
    let status = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-y"])
        .args(["-f", "lavfi", "-i", "testsrc=size=320x240:rate=24"])
        .args(["-f", "lavfi", "-i", "sine,aformat=channel_layouts=5.1"])
        .args(["-f", "lavfi", "-i", "sine=frequency=880"])
        .args(["-map", "0", "-map", "1", "-map", "2", "-t", "14"])
        .args(["-c:v", "libx264", "-preset", "ultrafast", "-g", "48"])
        .args(["-c:a", "aac", "-ac:a:1", "2"])
        .args([
            "-metadata:s:a:0",
            "language=eng",
            "-disposition:a:0",
            "default",
        ])
        .args(["-metadata:s:a:1", "language=eng", "-disposition:a:1", "0"])
        .args(["-metadata:s:a:1", "title=Commentary", "-f", "matroska"])
        .arg(path)
        .status()
        .unwrap();
    assert!(status.success(), "ffmpeg failed: {status}");
}

#[tokio::test]
async fn switching_to_the_commentary_mid_play_remuxes_it_from_there_with_its_audio() {
    let fx = Fixture::alien_written_by(ALIEN_1080P, write_english_and_a_stereo_commentary).await;
    let version = fx.version_ids(ALIEN).await[0];
    let english = fx.plan(version).await.unwrap();
    assert_eq!(played_audio(&fx, &english.url, "1.m4s").await, "aac,6");

    let commentary = fx.plan_with_audio(version, 2, 7_000).await.unwrap();

    assert_eq!(commentary.method, PlaybackMethod::PLAYBACK_METHOD_REMUX);
    assert_eq!(commentary.tracks[1].title.as_deref(), Some("Commentary"));
    assert_eq!(
        commentary.reasons[5],
        "The audio Track chosen isn't the one browsers play from its file, so it can't be Direct \
         Played"
    );
    // 7 seconds in is in the second segment, as they're cut about 6 seconds apart.
    assert_eq!(played_audio(&fx, &commentary.url, "1.m4s").await, "aac,2");
}

#[tokio::test]
async fn an_hevc_mkv_is_transcoded_to_h264_and_aac_over_hls_in_six_second_segments() {
    let (fx, version) = Fixture::alien_written(system_ffmpeg(), ffmpeg::write_hevc_with_5_1).await;

    let plan = fx.plan(version).await.unwrap();

    assert_eq!(plan.method, PlaybackMethod::PLAYBACK_METHOD_TRANSCODE);
    assert_eq!(codecs(&plan), ["h264", "aac"]);
    assert_eq!(
        plan.reasons[1],
        "This Device can't decode its Matroska container, HEVC video or AC-3 audio, so it \
         can't be Direct Played or Remuxed, and a Transcode converts it to H.264 and stereo \
         AAC, as this Device doesn't say it plays more"
    );
    let playlist = String::from_utf8(fx.fetch_ok(&plan.url).await).unwrap();
    let (durations, files) = segments(&playlist);
    assert_eq!(files, ["0.m4s", "1.m4s", "2.m4s", "3.m4s"], "20 seconds");
    assert_eq!(durations[..3], [6.0, 6.0, 6.0]);
    for file in files {
        let segment = fx.fetch_ok(&beside(&plan.url, file)).await;
        assert_ne!(segment.len(), 0, "{file}");
    }
    let video = played_streams(&fx, &plan.url, "v", "0.m4s", "codec_name,width,height").await;
    assert_eq!(video, "h264,320,240");
    assert_eq!(played_audio(&fx, &plan.url, "0.m4s").await, "aac,2");
}

#[tokio::test]
async fn a_1080p_h264_mkv_limited_to_720p_is_transcoded_down_to_it_and_original_remuxes_it_again() {
    let (fx, version) = Fixture::alien_written(system_ffmpeg(), ffmpeg::write_1080p_h264).await;
    let limited = |playback_limit: PlaybackLimit| PlanPlaybackRequest {
        version_id: version,
        playback_limit: playback_limit.into(),
        device_decoding: chrome().into(),
        ..Default::default()
    };

    let at_720p = fx
        .ask_for_plan(limited(PlaybackLimit::PLAYBACK_LIMIT_720P))
        .await
        .unwrap();
    let original = fx
        .ask_for_plan(limited(PlaybackLimit::PLAYBACK_LIMIT_ORIGINAL))
        .await
        .unwrap();

    assert_eq!(at_720p.method, PlaybackMethod::PLAYBACK_METHOD_TRANSCODE);
    assert_eq!(
        at_720p.reasons[7],
        "Its picture is larger than the 720p Playback Limit, so a Transcode scales it down to \
         1280×720"
    );
    assert_eq!(
        played_streams(&fx, &at_720p.url, "v", "0.m4s", "codec_name,width,height").await,
        "h264,1280,720"
    );
    assert_eq!(original.method, PlaybackMethod::PLAYBACK_METHOD_REMUX);
    assert_eq!(original.reasons[7], "The Playback Limit is Original");
}

/// A Server behind a reverse proxy at the test's own address, which it trusts.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn behind_proxy() -> Builder {
    TestServer::builder().config(Config {
        trusted_proxies: vec!["127.0.0.1".parse().unwrap()],
        ..Config::default()
    })
}

/// A plan for `version` on Chrome, with the Playback Limit `picked`.
fn picking(version: i64, picked: PlaybackLimit) -> PlanPlaybackRequest {
    PlanPlaybackRequest {
        version_id: version,
        playback_limit: picked.into(),
        device_decoding: chrome().into(),
        ..Default::default()
    }
}

const REMOTE_720P: &str = "This Device is outside the local network, so the default Playback \
                           Limit for remote playback, 720p, applies";
const LOCAL: &str = "This Device is on the local network";

#[tokio::test]
async fn a_1080p_h264_mkv_played_from_outside_the_local_network_transcodes_to_the_remote_default() {
    let behind = behind_proxy().ffmpeg(system_ffmpeg());
    let (fx, version) = Fixture::alien_written_on(behind, ffmpeg::write_1080p_h264).await;
    fx.set_remote_limit(PlaybackLimit::PLAYBACK_LIMIT_720P)
        .await;
    let unpicked = picking(version, PlaybackLimit::PLAYBACK_LIMIT_UNSPECIFIED);
    let original = picking(version, PlaybackLimit::PLAYBACK_LIMIT_ORIGINAL);
    let remote = "203.0.113.7";

    let defaulted = fx
        .ask_for_plan_relayed_for(remote, unpicked.clone())
        .await
        .unwrap();
    let picked_original = fx.ask_for_plan_relayed_for(remote, original).await.unwrap();
    let local = fx.ask_for_plan(unpicked).await.unwrap();

    assert_eq!(defaulted.method, PlaybackMethod::PLAYBACK_METHOD_TRANSCODE);
    assert_eq!(defaulted.reasons[6], REMOTE_720P);
    assert_eq!(defaulted.playback_limit, PlaybackLimit::PLAYBACK_LIMIT_720P);
    assert_eq!(
        played_streams(&fx, &defaulted.url, "v", "0.m4s", "width,height").await,
        "1280,720"
    );
    assert_eq!(
        picked_original.method,
        PlaybackMethod::PLAYBACK_METHOD_REMUX
    );
    assert_eq!(local.method, PlaybackMethod::PLAYBACK_METHOD_REMUX);
    assert_eq!(local.reasons[6], LOCAL);
    assert_eq!(local.playback_limit, PlaybackLimit::PLAYBACK_LIMIT_ORIGINAL);
}

#[tokio::test]
async fn behind_a_trusted_proxy_the_forwarded_client_address_decides_whether_playback_is_remote() {
    let fx = Fixture::alien_on(behind_proxy()).await;
    fx.set_remote_limit(PlaybackLimit::PLAYBACK_LIMIT_720P)
        .await;
    let version = fx.version_ids(ALIEN).await[0];
    let unpicked = picking(version, PlaybackLimit::PLAYBACK_LIMIT_UNSPECIFIED);

    let remote = fx
        .ask_for_plan_relayed_for("203.0.113.7", unpicked.clone())
        .await
        .unwrap();
    let local = fx
        .ask_for_plan_relayed_for("192.168.1.20", unpicked)
        .await
        .unwrap();

    assert_eq!(remote.reasons[6], REMOTE_720P);
    assert_eq!(remote.playback_limit, PlaybackLimit::PLAYBACK_LIMIT_720P);
    assert_eq!(local.reasons[6], LOCAL);
    assert_eq!(local.playback_limit, PlaybackLimit::PLAYBACK_LIMIT_ORIGINAL);
}

#[tokio::test]
async fn behind_an_untrusted_proxy_the_peers_address_decides_whether_playback_is_remote() {
    let fx = Fixture::alien().await;
    fx.set_remote_limit(PlaybackLimit::PLAYBACK_LIMIT_720P)
        .await;
    let version = fx.version_ids(ALIEN).await[0];

    let plan = fx
        .ask_for_plan_relayed_for(
            "203.0.113.7",
            picking(version, PlaybackLimit::PLAYBACK_LIMIT_UNSPECIFIED),
        )
        .await
        .unwrap();

    assert_eq!(plan.reasons[6], LOCAL);
    assert_eq!(plan.playback_limit, PlaybackLimit::PLAYBACK_LIMIT_ORIGINAL);
}

#[tokio::test]
async fn a_pgs_track_turned_on_mid_play_is_burned_into_a_transcode_from_there() {
    let (fx, version) = Fixture::alien_written(system_ffmpeg(), ffmpeg::write_black_with_pgs).await;

    let plan = fx
        .ask_for_plan(PlanPlaybackRequest {
            version_id: version,
            start_position_ms: 7000,
            subtitle_track: Some(1),
            device_decoding: chrome().into(),
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(plan.method, PlaybackMethod::PLAYBACK_METHOD_TRANSCODE);
    assert_eq!(
        plan.reasons[8],
        "The subtitles chosen are burned into its picture, which only a Transcode does"
    );
    // The player asks first for the segment 7 seconds falls in, where ffmpeg starts.
    let picture = last_frame(&fx, &plan.url, "1.m4s").await;
    assert!(picture[108 * 320 + 108] > 200, "the square is burned in");
    assert!(
        picture[10 * 320 + 10] < 50,
        "the picture around it stays black"
    );
}

/// The last frame of the segment `segment` of the 320×240 plan at `url`, as 8-bit grey pixels row
/// by row.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn last_frame(fx: &Fixture, url: &str, segment: &str) -> Vec<u8> {
    // The segment first, so ffmpeg starts there and the init segment is that run's.
    let segment = fx.fetch_ok(&beside(url, segment)).await;
    let mut mp4 = fx.fetch_ok(&beside(url, "init.mp4")).await;
    mp4.extend(segment);
    let file = tempfile::NamedTempFile::new().unwrap();
    fs::write(file.path(), mp4).unwrap();
    let output = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(file.path())
        .args(["-f", "rawvideo", "-pix_fmt", "gray", "-"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    output
        .stdout
        .rchunks_exact(320 * 240)
        .next()
        .unwrap()
        .to_vec()
}

#[tokio::test]
async fn hdr10_transcoded_for_a_browser_on_an_sdr_screen_is_tone_mapped_to_sdr() {
    let write = |file: &Path| ffmpeg::write_4k_with_5_1(file, "smpte2084");
    let (fx, version) = Fixture::alien_written(system_ffmpeg(), write).await;

    let plan = fx.plan(version).await.unwrap();

    assert_eq!(plan.method, PlaybackMethod::PLAYBACK_METHOD_TRANSCODE);
    assert_eq!(
        plan.reasons[2],
        "This Server has ffmpeg, which tone-maps its HDR10 picture to SDR in a Transcode"
    );
    let colours = "color_transfer,color_primaries";
    let played = played_streams(&fx, &plan.url, "v", "0.m4s", colours).await;
    assert_eq!(played, "bt709,bt709");
}

#[tokio::test]
async fn a_version_whose_direct_play_just_failed_on_the_device_cant_be_played_and_says_why() {
    let fx = Fixture::alien().await;
    let version = fx.version_ids(ALIEN).await[0];

    let plan = fx
        .plan_on_device(
            version,
            &[],
            &[PlaybackMethod::PLAYBACK_METHOD_DIRECT_PLAY],
            0,
        )
        .await
        .unwrap();

    assert_eq!(plan.method, PlaybackMethod::PLAYBACK_METHOD_UNSPECIFIED);
    assert_eq!(plan.url, "");
    assert_eq!(plan.reasons[4], "Direct Play failed on this Device");
}

#[tokio::test]
async fn a_version_whose_direct_play_just_failed_on_a_device_that_decodes_it_is_remuxed() {
    let (fx, version) = Fixture::alien_mkv(system_ffmpeg(), "aac").await;
    let matroska = "video/x-matroska";
    let direct_play = fx
        .plan_on_device(version, &[matroska], &[], 0)
        .await
        .unwrap();
    assert_eq!(
        direct_play.method,
        PlaybackMethod::PLAYBACK_METHOD_DIRECT_PLAY
    );

    let failed = [PlaybackMethod::PLAYBACK_METHOD_DIRECT_PLAY];
    let plan = fx
        .plan_on_device(version, &[matroska], &failed, 0)
        .await
        .unwrap();

    assert_eq!(plan.method, PlaybackMethod::PLAYBACK_METHOD_REMUX);
    assert!(
        plan.reasons
            .iter()
            .any(|reason| reason == "Direct Play failed on this Device"),
        "{:?}",
        plan.reasons
    );
}

#[tokio::test]
async fn planning_an_unknown_version_is_refused() {
    let fx = Fixture::alien().await;

    let planned = fx.plan(999).await;

    assert_refused(
        planned,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_VERSION_NOT_FOUND,
    );
}

/// Writes a second of video with English SDH and French Forced SRT subtitles, the French saying
/// `french`, as Matroska, to `path`, with the ffmpeg on `PATH`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn write_english_and_french_srt(path: &Path, french_says: &str) {
    let srt = tempfile::tempdir().unwrap();
    let (english, french) = (srt.path().join("eng.srt"), srt.path().join("fre.srt"));
    fs::write(&english, "1\n00:00:00,000 --> 00:00:01,000\nHello\n").unwrap();
    fs::write(
        &french,
        format!("1\n00:00:00,000 --> 00:00:01,000\n{french_says}\n"),
    )
    .unwrap();
    let status = std::process::Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "color=size=64x64:rate=24",
        ])
        .arg("-i")
        .arg(&english)
        .arg("-i")
        .arg(&french)
        .args([
            "-map", "0", "-map", "1", "-map", "2", "-t", "1", "-c:v", "ffv1", "-c:s", "srt",
        ])
        .args([
            "-metadata:s:s:0",
            "language=eng",
            "-metadata:s:s:0",
            "title=English SDH",
        ])
        .args(["-disposition:s:0", "hearing_impaired"])
        .args([
            "-metadata:s:s:1",
            "language=fre",
            "-disposition:s:1",
            "forced",
        ])
        .args(["-f", "matroska"])
        .arg(path)
        .status()
        .unwrap();
    assert!(status.success(), "ffmpeg failed: {status}");
}

#[tokio::test]
async fn an_mkvs_srt_tracks_are_offered_with_their_plan_and_served_as_webvtt() {
    let fx = Fixture::subtitled_alien().await;
    let version = fx.version_ids(ALIEN).await[0];

    let plan = fx.plan(version).await.unwrap();

    let offered: Vec<_> = (plan.subtitles.iter())
        .map(|subtitle| {
            let track = &subtitle.track;
            let (language, title) = (track.language.as_deref(), track.title.as_deref());
            (language, title, track.forced, track.sdh)
        })
        .collect();
    assert_eq!(
        offered,
        [
            (Some("eng"), Some("English SDH"), false, true),
            (Some("fre"), None, true, false),
        ]
    );
    let token = Some(fx.server.admin_token.as_str());
    let french = fetch(&fx.server, &plan.subtitles[1].url, token).await;
    assert_eq!(french.status, 200);
    assert_eq!(french.header("content-type"), "text/vtt; charset=utf-8");
    let french = String::from_utf8(french.body).unwrap();
    assert!(french.starts_with("WEBVTT"), "{french}");
    assert!(
        french.contains("00:00.000 --> 00:01.000\nBonjour"),
        "{french}"
    );
    let again = fetch(&fx.server, &plan.subtitles[1].url, token).await;
    assert_eq!(String::from_utf8(again.body).unwrap(), french);
}

#[tokio::test]
async fn a_replaced_file_probed_again_serves_its_own_subtitles_not_the_cached_ones() {
    let fx = Fixture::subtitled_alien().await;
    let version = fx.version_ids(ALIEN).await[0];
    let token = Some(fx.server.admin_token.as_str());
    let old_french = &fx.plan(version).await.unwrap().subtitles[1].url;
    let cached = fetch(&fx.server, old_french, token).await;
    assert!(String::from_utf8(cached.body).unwrap().contains("Bonjour"));
    let before = fx.item(ALIEN_1080P).await;

    write_english_and_french_srt(&fx.folder.join(ALIEN_1080P), "Salut");
    fx.rescan_until_replaced(ALIEN_1080P, before).await;
    let french = &fx.plan(version).await.unwrap().subtitles[1].url;
    let replaced = fetch(&fx.server, french, token).await;

    let replaced = String::from_utf8(replaced.body).unwrap();
    assert!(
        replaced.contains("00:00.000 --> 00:01.000\nSalut"),
        "{replaced}"
    );
}

#[tokio::test]
async fn a_removed_versions_cached_subtitles_are_removed() {
    let fx = Fixture::subtitled_alien().await;
    fx.cache_subtitles().await;

    fs::remove_file(fx.folder.join(ALIEN_1080P)).unwrap();

    fx.scan_until(fx.library, async || fx.cached_subtitles().is_empty())
        .await;
}

#[tokio::test]
async fn a_deleted_librarys_cached_subtitles_are_removed() {
    let fx = Fixture::subtitled_alien().await;
    fx.cache_subtitles().await;

    let libraries = LibraryServiceClient::new(HttpClient::plaintext(), fx.server.client_config());
    libraries
        .delete_library(DeleteLibraryRequest {
            id: fx.library,
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(fx.cached_subtitles(), Vec::<PathBuf>::new());
}

#[tokio::test]
async fn an_mp4s_mov_text_track_is_served_as_webvtt() {
    let webvtt = served_webvtt("Alien (1979)/Alien (1979).mp4", HELLO_SRT, "mov_text").await;

    assert!(webvtt.starts_with("WEBVTT"), "{webvtt}");
    assert!(
        webvtt.contains("00:00.000 --> 00:01.000\nHello"),
        "{webvtt}"
    );
}

#[tokio::test]
async fn an_embedded_webvtt_track_is_served() {
    let webvtt = served_webvtt(ALIEN_1080P, HELLO_SRT, "webvtt").await;

    assert!(webvtt.starts_with("WEBVTT"), "{webvtt}");
    assert!(
        webvtt.contains("00:00.000 --> 00:01.000\nHello"),
        "{webvtt}"
    );
}

#[tokio::test]
async fn an_ass_track_is_served_as_plain_webvtt_cues() {
    let webvtt = served_webvtt(ALIEN_1080P, STYLED_ASS, "ass").await;

    assert_eq!(
        webvtt,
        "WEBVTT\n\n\
         00:00:00.000 --> 00:00:00.500\nHello there\nfriend\n\n\
         00:00:00.500 --> 00:00:01.000\nStyled line\n\n"
    );
}

/// A subtitle script's file name, which tells ffmpeg its format, and what it says.
type Script = (&'static str, &'static str);

const HELLO_SRT: Script = ("hello.srt", "1\n00:00:00,000 --> 00:00:01,000\nHello\n");

/// Two lines, positioned, coloured and styled, and a shape drawn over the whole second.
const STYLED_ASS: Script = (
    "styled.ass",
    r"[Script Info]
ScriptType: v4.00+

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1
Style: Fancy,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,-1,-1,-1,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:00.00,0:00:00.50,Default,,0,0,0,,{\an8\pos(10,10)\c&H0000FF&\b1}Hello{\b0} {\i1}there{\i0}\Nfriend
Dialogue: 0,0:00:00.50,0:00:01.00,Fancy,,0,0,0,,Styled line
Dialogue: 0,0:00:00.00,0:00:01.00,Default,,0,0,0,,{\p1}m 0 0 l 100 0 100 100 0 100{\p0}
",
);

/// What the Server serves for the one subtitle Track of an Alien at `file`, which is `script`
/// stored as `codec`.
#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "a test helper fails its test by panicking"
)]
async fn served_webvtt(file: &str, script: Script, codec: &str) -> String {
    let fx = Fixture::alien_written_by(file, |path| write_subtitled(path, script, codec)).await;
    let version = fx.version_ids(ALIEN).await[0];
    let plan = fx.plan(version).await.unwrap();
    let token = Some(fx.server.admin_token.as_str());
    let reply = fetch(&fx.server, &plan.subtitles[0].url, token).await;
    assert_eq!(reply.status, 200);
    String::from_utf8(reply.body).unwrap()
}

/// Writes a second of video with `script` as its one subtitle Track, stored as `codec`, to
/// `path`, in the format its extension names, with the ffmpeg on `PATH`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn write_subtitled(path: &Path, (name, contents): Script, codec: &str) {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join(name);
    fs::write(&script, contents).unwrap();
    let status = std::process::Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "color=size=64x64:rate=24",
            "-i",
        ])
        .arg(&script)
        .args([
            "-map", "0", "-map", "1", "-t", "1", "-c:v", "mpeg4", "-c:s", codec,
        ])
        .arg(path)
        .status()
        .unwrap();
    assert!(status.success(), "ffmpeg failed: {status}");
}

/// Anyone who may play a Version may fetch its subtitles: anyone with a session.
#[tokio::test]
async fn a_subtitle_without_a_session_is_refused() {
    let fx = Fixture::subtitled_alien().await;
    let version = fx.version_ids(ALIEN).await[0];
    let plan = fx.plan(version).await.unwrap();

    let reply = fetch(&fx.server, &plan.subtitles[0].url, None).await;

    assert_eq!(reply.status, 401);
}

#[tokio::test]
async fn a_track_that_isnt_a_text_subtitle_is_not_found_as_one() {
    let fx = Fixture::subtitled_alien().await;
    let version = fx.version_ids(ALIEN).await[0];
    let token = Some(fx.server.admin_token.as_str());

    let (video, beyond) = (
        format!("/media/versions/{version}/subtitles/0"),
        format!("/media/versions/{version}/subtitles/9"),
    );

    assert_eq!(fetch(&fx.server, &video, token).await.status, 404);
    assert_eq!(fetch(&fx.server, &beyond, token).await.status, 404);
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
    let before = fx.item(ALIEN_1080P).await;

    viewer.report(version, 60_000, TWO_HOURS).await.unwrap();
    fx.write(ALIEN_1080P, b"a re-encoded copy of the video");
    fx.rescan_until_replaced(ALIEN_1080P, before).await;

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
    fx.scan_until_decided(2).await;
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
    let item = fx.item(ALIEN_1080P).await;
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
    let before = fx.item(DARK_S02E01).await;

    viewer.report(version, MINUTE, TWO_HOURS).await.unwrap();
    fx.write(DARK_S02E01, b"a re-encoded copy of the video");
    fx.rescan_until_replaced(DARK_S02E01, before).await;

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
                MovieOrEpisode::Episode(episode) => {
                    let show = entry.show.as_option().unwrap().title.as_str();
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
    assert!(entries[1].show.is_unset(), "a Movie has no Show");
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
    fx.scan_until(fx.library, async || fx.all_decided(fx.library, 0).await)
        .await;
    let gone = viewer.continue_watching().await;
    fx.write(ALIEN_1080P, b"video");
    fx.scan_until_decided(1).await;
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
