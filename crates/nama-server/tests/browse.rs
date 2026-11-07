//! Browsing Movies and Shows through the generated `BrowseService` client, and the Watch State
//! that `WatchStateService` reports and browsing shows, over a real socket, with the TMDB Plugin
//! enabled against a fake TMDB.

#[expect(dead_code, reason = "the harness's plain `start` has no use here")]
#[path = "support/server.rs"]
mod server;

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::PathBuf,
    slice,
    sync::Arc,
    time::Duration,
};

use connectrpc::{
    ConnectError, ErrorCode,
    client::{ClientConfig, HttpClient},
};
use nama_plugin_api::settings::{SettingValue, Settings};
use nama_plugin_tmdb::{
    Tmdb,
    testing::{API_KEY, FakeTmdb},
};
use nama_proto::nama::v1::{
    BrowseServiceClient, ClearMatchRequest, CreateLibraryRequest, CreateUserRequest, Episode,
    ExternalId, GetMovieRequest, GetShowRequest, GetShowResponse, LibraryKind,
    LibraryServiceClient, ListEpisodesRequest, ListMoviesRequest, ListShowsRequest,
    MatchServiceClient, Movie, ReportProgressRequest, Role, ScanLibraryRequest, Season,
    SessionDelivery, SetMatchRequest, Show, SignInRequest, SignInServiceClient, UserServiceClient,
    Version, WatchState, WatchStateServiceClient, external_id,
};
use nama_server::{config::Config, contents::Contents, plugins::Registry};
use server::{PASSWORD, TestServer};
use tempfile::TempDir;

/// TMDB Matches both to Alien, TMDB id 348.
const ALIEN_1080P: &str = "Alien (1979)/Alien (1979) - 1080p.mkv";
const ALIEN_2160P: &str = "Alien (1979)/Alien (1979) - 2160p.mkv";
const ALIEN: i64 = 348;

/// TMDB finds nothing for this file, so it is Unmatched.
const HOME_MOVIES: &str = "Home Movies (2019)/Home Movies (2019).mkv";

/// Dark, TMDB id 70523, in two folders: TMDB finds the first by its title and year, and the
/// second names it in an ID hint. The first file holds two Episodes.
const DARK_S01E01_E02: &str = "Dark (2017)/Season 01/Dark - S01E01-E02.mkv";
const DARK_S01E01: &str = "Dark {tmdb-70523}/Season 01/Dark - S01E01.mkv";
const DARK_S02E01: &str = "Dark {tmdb-70523}/Season 02/Dark - S02E01.mkv";
const DARK: i64 = 70523;
/// The TMDB ids of Dark's Episodes S01E01, S01E02 and S02E01.
const SECRETS: i64 = 1_388_538;
const LIES: i64 = 1_388_539;
const BEGINNINGS_AND_ENDINGS: i64 = 1_803_405;

/// Both the US and the UK Office fit, so this is Unmatched.
const THE_OFFICE: &str = "The Office/Season 01/The Office - S01E01.mkv";
const THE_OFFICE_US: i64 = 2316;

/// Two hours, in milliseconds: the length a test's player reports.
const TWO_HOURS: u64 = 7_200_000;

/// A Server with one Library, the TMDB Plugin enabled against a fake TMDB, and the clients a test
/// browses and fixes Matches with.
struct Fixture {
    library: i64,
    folder: PathBuf,
    libraries: LibraryServiceClient<HttpClient>,
    browse: BrowseServiceClient<HttpClient>,
    matches: MatchServiceClient<HttpClient>,
    contents: Contents,
    tmdb: FakeTmdb,
    server: TestServer,
    _media: TempDir,
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
impl Fixture {
    /// Starts a Server with a Library of `kind`.
    async fn start(kind: LibraryKind) -> Self {
        let tmdb = FakeTmdb::start().await;
        let plugins = Registry::new();
        let settings = Settings::new(BTreeMap::from([(
            "api_key".to_owned(),
            SettingValue::Text(API_KEY.to_owned()),
        )]));
        plugins
            .enable_with_settings(Arc::new(Tmdb::with_endpoints(tmdb.endpoints())), settings)
            .unwrap();
        let (server, db) = TestServer::start_with_db(Config::default(), plugins).await;
        let config = server.client_config();
        let libraries = LibraryServiceClient::new(HttpClient::plaintext(), config.clone());
        let media = tempfile::tempdir().unwrap();
        let folder = media.path().canonicalize().unwrap();
        let library = libraries
            .create_library(CreateLibraryRequest {
                name: "Media".to_owned(),
                kind: kind.into(),
                folders: vec![folder.to_str().unwrap().to_owned()],
                ..Default::default()
            })
            .await
            .unwrap()
            .into_owned()
            .library
            .into_option()
            .unwrap();
        Self {
            library: library.id,
            folder,
            libraries,
            browse: BrowseServiceClient::new(HttpClient::plaintext(), config.clone()),
            matches: MatchServiceClient::new(HttpClient::plaintext(), config),
            contents: Contents::new(db),
            tmdb,
            server,
            _media: media,
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

    /// Starts a Server with a Movies Library holding Alien, scanned and Matched.
    async fn alien() -> Self {
        let fx = Self::start(LibraryKind::LIBRARY_KIND_MOVIES).await;
        fx.write(ALIEN_1080P, b"video");
        fx.scan_until_decided(1).await;
        fx
    }

    /// Writes `contents` to the file at `relative` in the Library's folder.
    fn write(&self, relative: &str, contents: &[u8]) {
        let path = self.folder.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    /// Scans the Library, and waits until `files` files are stored and Matching has decided
    /// about all their items.
    async fn scan_until_decided(&self, files: usize) {
        self.scan_until(async || self.all_decided(files).await)
            .await;
    }

    /// Scans the Library, and waits until `done` says so.
    async fn scan_until(&self, mut done: impl AsyncFnMut() -> bool) {
        self.libraries
            .scan_library(ScanLibraryRequest {
                id: self.library,
                ..Default::default()
            })
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            while !done().await {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
    }

    /// Whether `files` files are stored, and Matching has decided about every item they hold.
    async fn all_decided(&self, files: usize) -> bool {
        let stored = self.contents.files(self.library).await.unwrap();
        stored.len() == files
            && stored
                .iter()
                .flat_map(|file| &file.items)
                .all(|item| item.recorded.is_some())
    }

    /// Rescans the Library after the file at `relative`, which held the item `before`, changed,
    /// and waits until a new item has replaced it and Matching has decided about that.
    async fn rescan_until_replaced(&self, relative: &str, before: i64) {
        self.scan_until(async || self.replaced(relative, before).await)
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

    /// A Member named Grace, created and signed in, browsing and playing.
    async fn member(&self) -> Viewer {
        UserServiceClient::new(HttpClient::plaintext(), self.server.client_config())
            .create_user(CreateUserRequest {
                name: "Grace".to_owned(),
                password: PASSWORD.to_owned(),
                role: Role::ROLE_MEMBER.into(),
                ..Default::default()
            })
            .await
            .unwrap();
        let token = SignInServiceClient::new(
            HttpClient::plaintext(),
            self.server.anonymous_client_config(),
        )
        .sign_in(SignInRequest {
            name: "Grace".to_owned(),
            password: PASSWORD.to_owned(),
            device_name: "Phone".to_owned(),
            delivery: SessionDelivery::SESSION_DELIVERY_TOKEN.into(),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .token;
        Viewer::new(self.server.client_config_as(&token))
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

    /// The Movies of the Library `library`, as `ListMovies` lists them.
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

    /// The Shows of the Library `library`, as `ListShows` lists them.
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
}

/// An HTTP response, as a test checks it.
struct Reply {
    status: u16,
    content_type: String,
    body: Vec<u8>,
}

/// Sends a plain HTTP/1.1 `GET` for `path` to `server`, presenting `token`, if any, in the
/// session cookie, as a browser's `<img src>` does.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn fetch(server: &TestServer, path: &str, token: Option<&str>) -> Reply {
    use http_body_util::{BodyExt, Empty};
    use hyper::{Request, body::Bytes, client::conn::http1};
    use hyper_util::rt::TokioIo;

    let stream = tokio::net::TcpStream::connect(server.addr).await.unwrap();
    let (mut sender, connection) = http1::handshake(TokioIo::new(stream)).await.unwrap();
    tokio::spawn(connection);
    let mut request = Request::get(path).header("host", server.addr.to_string());
    if let Some(token) = token {
        request = request.header("cookie", format!("nama_session={token}"));
    }
    let response = sender
        .send_request(request.body(Empty::<Bytes>::new()).unwrap())
        .await
        .unwrap();
    Reply {
        status: response.status().as_u16(),
        content_type: response
            .headers()
            .get("content-type")
            .map(|value| value.to_str().unwrap().to_owned())
            .unwrap_or_default(),
        body: response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    }
}

/// The external id for the TMDB id `id`.
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
async fn an_unmatched_item_is_in_neither_list_movies_nor_get_movie() {
    let fx = Fixture::start(LibraryKind::LIBRARY_KIND_MOVIES).await;
    fx.write(ALIEN_1080P, b"1080p");
    fx.scan_until_decided(1).await;
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
    assert_eq!(got.unwrap_err().code, ErrorCode::NotFound);
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

    assert_eq!(movies.unwrap_err().code, ErrorCode::NotFound);
    assert_eq!(movie.unwrap_err().code, ErrorCode::NotFound);
    assert_eq!(shows.unwrap_err().code, ErrorCode::NotFound);
    assert_eq!(show.unwrap_err().code, ErrorCode::NotFound);
    assert_eq!(episodes.unwrap_err().code, ErrorCode::NotFound);
}

#[tokio::test]
async fn a_shows_library_is_an_invalid_argument() {
    let fx = Fixture::start(LibraryKind::LIBRARY_KIND_SHOWS).await;

    let listed = fx.list_movies(fx.library).await;
    let got = fx.get_movie(fx.library, ALIEN).await;

    assert_eq!(listed.unwrap_err().code, ErrorCode::InvalidArgument);
    assert_eq!(got.unwrap_err().code, ErrorCode::InvalidArgument);
}

#[tokio::test]
async fn get_movie_for_an_id_nothing_matches_is_not_found() {
    let fx = Fixture::start(LibraryKind::LIBRARY_KIND_MOVIES).await;
    fx.write(ALIEN_1080P, b"1080p");
    fx.scan_until_decided(1).await;

    let got = fx.get_movie(fx.library, 949).await;

    assert_eq!(got.unwrap_err().code, ErrorCode::NotFound);
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
    assert_eq!(second.unwrap_err().code, ErrorCode::NotFound);
}

#[tokio::test]
async fn an_unknown_show_or_season_is_not_found() {
    let fx = Fixture::shows().await;

    let show = fx.get_show(fx.library, THE_OFFICE_US).await;
    let episodes = fx.list_episodes(fx.library, THE_OFFICE_US, 1).await;
    let season = fx.list_episodes(fx.library, DARK, 3).await;

    assert_eq!(show.unwrap_err().code, ErrorCode::NotFound);
    assert_eq!(episodes.unwrap_err().code, ErrorCode::NotFound);
    assert_eq!(season.unwrap_err().code, ErrorCode::NotFound);
}

#[tokio::test]
async fn a_movies_library_is_an_invalid_argument_to_browse_shows() {
    let fx = Fixture::start(LibraryKind::LIBRARY_KIND_MOVIES).await;

    let shows = fx.list_shows(fx.library).await;
    let show = fx.get_show(fx.library, DARK).await;
    let episodes = fx.list_episodes(fx.library, DARK, 1).await;

    assert_eq!(shows.unwrap_err().code, ErrorCode::InvalidArgument);
    assert_eq!(show.unwrap_err().code, ErrorCode::InvalidArgument);
    assert_eq!(episodes.unwrap_err().code, ErrorCode::InvalidArgument);
}

#[tokio::test]
async fn a_poster_a_match_stores_is_served_with_its_content_type() {
    let fx = Fixture::alien().await;
    let movie = fx.get_movie(fx.library, ALIEN).await.unwrap();
    let poster = movie.poster_url.unwrap();

    let reply = fetch(&fx.server, &poster, Some(&fx.server.admin_token)).await;

    assert_eq!(reply.status, 200);
    assert_eq!(reply.content_type, "image/jpeg");
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
    let images = fx.tmdb.requests();
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
    let fx = Fixture::alien().await;
    let movie = fx.get_movie(fx.library, ALIEN).await.unwrap();
    drop(fx.tmdb);

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
    let grace = fx.member().await;
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

    assert_eq!(unknown.unwrap_err().code, ErrorCode::NotFound);
    assert_eq!(zero.unwrap_err().code, ErrorCode::InvalidArgument);
}
