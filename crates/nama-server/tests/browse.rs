//! Browsing Movies through the generated `BrowseService` client, over a real socket, with the TMDB
//! Plugin enabled against a fake TMDB.

#[expect(dead_code, reason = "the harness's plain `start` has no use here")]
#[path = "support/server.rs"]
mod server;

use std::{collections::BTreeMap, fs, path::PathBuf, slice, sync::Arc, time::Duration};

use connectrpc::{ConnectError, ErrorCode, client::HttpClient};
use nama_plugin_api::settings::{SettingValue, Settings};
use nama_plugin_tmdb::{
    Tmdb,
    testing::{API_KEY, FakeTmdb},
};
use nama_proto::nama::v1::{
    BrowseServiceClient, ClearMatchRequest, CreateLibraryRequest, ExternalId, GetMovieRequest,
    LibraryKind, LibraryServiceClient, ListMoviesRequest, MatchServiceClient, Movie,
    ScanLibraryRequest, external_id,
};
use nama_server::{config::Config, contents::Contents, plugins::Registry};
use server::TestServer;
use tempfile::TempDir;

/// TMDB Matches both to Alien, TMDB id 348.
const ALIEN_1080P: &str = "Alien (1979)/Alien (1979) - 1080p.mkv";
const ALIEN_2160P: &str = "Alien (1979)/Alien (1979) - 2160p.mkv";
const ALIEN: i64 = 348;

/// TMDB finds nothing for this file, so it is Unmatched.
const HOME_MOVIES: &str = "Home Movies (2019)/Home Movies (2019).mkv";

/// A Server with one Library, the TMDB Plugin enabled against a fake TMDB, and the clients a test
/// browses and fixes Matches with.
struct Fixture {
    library: i64,
    folder: PathBuf,
    libraries: LibraryServiceClient<HttpClient>,
    browse: BrowseServiceClient<HttpClient>,
    matches: MatchServiceClient<HttpClient>,
    contents: Contents,
    _tmdb: FakeTmdb,
    _server: TestServer,
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
            _tmdb: tmdb,
            _server: server,
            _media: media,
        }
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
        self.libraries
            .scan_library(ScanLibraryRequest {
                id: self.library,
                ..Default::default()
            })
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            while !self.all_decided(files).await {
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
}

fn tmdb_id(id: i64) -> ExternalId {
    ExternalId {
        id: Some(external_id::Id::Tmdb(id)),
        ..Default::default()
    }
}

/// Each Version's file name and size; ids are checked apart, since the Server assigns them.
fn versions(movie: &Movie) -> Vec<(&str, u64)> {
    movie
        .versions
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
        versions(&got),
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

    let listed = fx.list_movies(unknown).await;
    let got = fx.get_movie(unknown, ALIEN).await;

    assert_eq!(listed.unwrap_err().code, ErrorCode::NotFound);
    assert_eq!(got.unwrap_err().code, ErrorCode::NotFound);
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
