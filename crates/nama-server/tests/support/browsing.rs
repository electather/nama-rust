//! A Server with one Library and TMDB on, for the tests that browse it and those that play its
//! Versions.

use std::{collections::HashMap, path::PathBuf};

use connectrpc::{ConnectError, client::HttpClient};
use nama_proto::nama::v1::{
    BrowseServiceClient, ExternalId, GetMovieRequest, LibraryKind, MatchServiceClient, Movie,
    external_id,
};
use nama_server::ffmpeg::Ffmpeg;

use crate::server::{Builder, Library, TestServer};

/// TMDB Matches both to Alien.
pub(crate) const ALIEN_1080P: &str = "Alien (1979)/Alien (1979) - 1080p.mkv";
pub(crate) const ALIEN_2160P: &str = "Alien (1979)/Alien (1979) - 2160p.mkv";

/// A Server with one Library and TMDB on, and the clients a test browses and fixes Matches with.
pub(crate) struct Fixture {
    pub(crate) library: i64,
    pub(crate) folder: PathBuf,
    pub(crate) browse: BrowseServiceClient<HttpClient>,
    pub(crate) matches: MatchServiceClient<HttpClient>,
    /// The first Item of each file, by its path in the Library, as the scan that set the fixture
    /// up stored them.
    pub(crate) items: HashMap<String, i64>,
    pub(crate) server: TestServer,
    pub(crate) media: Library,
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
impl Fixture {
    /// Starts a Server with a Library of `kind` that found `ffmpeg` at startup.
    pub(crate) async fn start_with_ffmpeg(kind: LibraryKind, ffmpeg: Ffmpeg) -> Self {
        Self::start_on(kind, TestServer::builder().ffmpeg(ffmpeg)).await
    }

    /// Starts a Server as `builder` says, with a fake TMDB and a Library of `kind`.
    pub(crate) async fn start_on(kind: LibraryKind, builder: Builder) -> Self {
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
            items: HashMap::new(),
            server,
            media,
        }
    }

    /// Starts a Server with a Movies Library holding Alien, scanned and Matched.
    pub(crate) async fn alien() -> Self {
        Self::alien_on(TestServer::builder()).await
    }

    /// [`Self::alien`] on a Server `builder` starts.
    pub(crate) async fn alien_on(builder: Builder) -> Self {
        let server = builder.tmdb().start().await;
        let media = server
            .create_library(LibraryKind::LIBRARY_KIND_MOVIES, &[ALIEN_1080P])
            .await;
        let mut fx = Self::over(server, media);
        fx.items = fx.scan_and_match().await;
        fx
    }

    pub(crate) fn write(&self, relative: &str, contents: &[u8]) {
        self.media.write(relative, contents);
    }

    /// Scans the Library until Matching has finished, and returns the first Item the scan stored
    /// for each file, by its path in the Library.
    pub(crate) async fn scan_and_match(&self) -> HashMap<String, i64> {
        self.server.scan_and_match(&self.media).await
    }

    /// The ids of the Versions of the Movie with the TMDB id `tmdb`, by path.
    pub(crate) async fn version_ids(&self, tmdb: i64) -> Vec<i64> {
        let movie = self.get_movie(self.library, tmdb).await.unwrap();
        movie.versions.iter().map(|version| version.id).collect()
    }

    /// The id of the first Item the scan that set the fixture up stored for the file at
    /// `relative`.
    #[expect(
        clippy::indexing_slicing,
        reason = "a test helper fails its test by panicking"
    )]
    pub(crate) fn item(&self, relative: &str) -> i64 {
        self.items[relative]
    }

    /// The Movie of the Library `library` with the TMDB id `tmdb`, as `GetMovie` gives it.
    pub(crate) async fn get_movie(&self, library: i64, tmdb: i64) -> Result<Movie, ConnectError> {
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

pub(crate) fn tmdb_id(id: i64) -> ExternalId {
    ExternalId {
        id: Some(external_id::Id::Tmdb(id)),
        ..Default::default()
    }
}
