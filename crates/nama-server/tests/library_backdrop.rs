//! A Library's Backdrop: its Custom Image, or else the Backdrop of the Matched title most recently
//! added to it (ADR 0036), as `ListLibraries` and `GetLibrary` give it, over a real socket with
//! the TMDB Plugin enabled against a fake TMDB.

#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/browsing.rs"]
mod browsing;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/http.rs"]
mod http;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use std::io::Cursor;

use browsing::{Fixture, tmdb_id};
use connectrpc::client::HttpClient;
use http::{Reply, fetch, send};
use http_body_util::Full;
use hyper::{Method, Request, body::Bytes};
use image::{ImageFormat, RgbImage};
use nama_proto::nama::v1::{
    DeleteLibraryRequest, GetLibraryRequest, GetShowRequest, Library, LibraryKind,
    LibraryServiceClient, ListLibrariesRequest,
};
use server::{
    TestServer,
    tmdb::{ALIEN, DARK, DUNE},
};

/// TMDB Matches it to Dune (2021), which has a Backdrop.
const DUNE_2021: &str = "Dune (2021)/Dune (2021).mkv";
/// TMDB Matches it to Dark, whose Episodes have the Show's Backdrop.
const DARK_S01E01: &str = "Dark (2017)/Season 01/Dark - S01E01.mkv";
/// Its ID hint Matches it to a Hamlet that has no Backdrop.
const HAMLET: &str = "Hamlet (2000) {tmdb-10688}/Hamlet (2000).mkv";

/// A 3 by 2 PNG.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn png() -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    RgbImage::new(3, 2)
        .write_to(&mut bytes, ImageFormat::Png)
        .unwrap();
    bytes.into_inner()
}

/// Where the Library `id`'s Backdrop is uploaded to and removed from.
fn backdrop_path(id: i64) -> String {
    format!("/custom-images/library/{id}/backdrop")
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
impl Fixture {
    fn libraries_as(&self, token: &str) -> LibraryServiceClient<HttpClient> {
        LibraryServiceClient::new(HttpClient::plaintext(), self.server.client_config_as(token))
    }

    /// The fixture's Library as `ListLibraries` gives it to the User signed in as `token`.
    async fn listed_as(&self, token: &str) -> Library {
        let listed = self
            .libraries_as(token)
            .list_libraries(ListLibrariesRequest::default())
            .await
            .unwrap()
            .into_owned()
            .libraries;
        listed.into_iter().find(|it| it.id == self.library).unwrap()
    }

    /// The fixture's Library as `ListLibraries` gives it to the Admin.
    async fn listed(&self) -> Library {
        self.listed_as(&self.server.admin_token).await
    }

    /// The Backdrop URL of the Movie with the TMDB id `tmdb`, as Browse gives it.
    async fn movie_backdrop(&self, tmdb: i64) -> Option<String> {
        self.get_movie(self.library, tmdb)
            .await
            .unwrap()
            .backdrop_url
    }

    /// Sends `bytes` to `path` with `method` as the User signed in as `token`.
    async fn send_as(&self, token: &str, method: Method, path: &str, bytes: &[u8]) -> Reply {
        let body = Full::new(Bytes::copy_from_slice(bytes));
        let request = Request::builder()
            .method(method)
            .uri(path)
            .body(body)
            .unwrap();
        send(&self.server, request, Some(token)).await
    }

    /// Uploads a PNG as the fixture Library's Custom Image, as the Admin.
    async fn upload_backdrop(&self) -> Reply {
        let token = &self.server.admin_token;
        let path = backdrop_path(self.library);
        self.send_as(token, Method::PUT, &path, &png()).await
    }
}

#[tokio::test]
async fn a_library_with_a_matched_movie_is_shown_by_its_backdrop() {
    let fx = Fixture::alien().await;

    let library = fx.listed().await;

    assert!(library.backdrop_url.is_some());
    assert_eq!(library.backdrop_url, fx.movie_backdrop(ALIEN).await);
    assert!(!library.custom_backdrop);
}

#[tokio::test]
async fn matching_a_newer_title_with_a_backdrop_shows_the_library_by_it() {
    let fx = Fixture::alien().await;

    fx.write(DUNE_2021, b"video");
    fx.scan_and_match().await;

    assert_eq!(
        fx.listed().await.backdrop_url,
        fx.movie_backdrop(DUNE).await
    );
}

#[tokio::test]
async fn a_newer_title_without_a_backdrop_leaves_the_librarys_as_it_was() {
    let fx = Fixture::alien().await;

    fx.write(HAMLET, b"video");
    fx.scan_and_match().await;

    assert_eq!(
        fx.listed().await.backdrop_url,
        fx.movie_backdrop(ALIEN).await
    );
}

#[tokio::test]
async fn a_shows_library_is_shown_by_its_newest_shows_backdrop() {
    let fx = Fixture::start_on(LibraryKind::LIBRARY_KIND_SHOWS, TestServer::builder()).await;
    fx.write(DARK_S01E01, b"video");
    fx.scan_and_match().await;

    let show = fx
        .browse
        .get_show(GetShowRequest {
            library_id: fx.library,
            show_id: tmdb_id(DARK).into(),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .show
        .into_option()
        .unwrap();

    assert!(show.backdrop_url.is_some());
    assert_eq!(fx.listed().await.backdrop_url, show.backdrop_url);
}

#[tokio::test]
async fn the_newest_titles_custom_backdrop_is_the_one_the_library_shows() {
    let fx = Fixture::alien().await;
    let token = &fx.server.admin_token;
    let path = format!("/custom-images/movie/tmdb-{ALIEN}/backdrop");

    fx.send_as(token, Method::PUT, &path, &png()).await;

    let custom = fx.movie_backdrop(ALIEN).await.unwrap();
    assert!(custom.starts_with("/media/custom-images/movie/"));
    let library = fx.listed().await;
    assert_eq!(library.backdrop_url, Some(custom));
    assert!(!library.custom_backdrop);
}

#[tokio::test]
async fn a_library_with_no_matched_title_or_none_with_a_backdrop_has_no_backdrop() {
    let fx = Fixture::start_on(LibraryKind::LIBRARY_KIND_MOVIES, TestServer::builder()).await;
    let empty = fx.listed().await;

    fx.write(HAMLET, b"video");
    fx.scan_and_match().await;

    assert_eq!(empty.backdrop_url, None);
    assert_eq!(fx.listed().await.backdrop_url, None);
}

#[tokio::test]
async fn an_uploaded_backdrop_shows_for_everyone_until_removed() {
    let fx = Fixture::alien().await;
    let grace = fx.server.user_with_default_permissions("Grace").await.token;

    let uploaded = fx.upload_backdrop().await;

    assert_eq!(uploaded.status, 204);
    let library = fx.listed_as(&grace).await;
    assert!(library.custom_backdrop);
    let url = library.backdrop_url.unwrap();
    assert!(url.starts_with(&format!(
        "/media/custom-images/library/{}/backdrop/",
        fx.library
    )));
    assert_eq!(fetch(&fx.server, &url, Some(&grace)).await.body, png());
    let got = fx
        .libraries_as(&fx.server.admin_token)
        .get_library(GetLibraryRequest {
            id: fx.library,
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .library
        .into_option()
        .unwrap();
    assert_eq!(got.backdrop_url, Some(url));
}

#[tokio::test]
async fn removing_a_librarys_custom_image_brings_the_newest_titles_backdrop_back() {
    let fx = Fixture::alien().await;
    fx.upload_backdrop().await;

    let path = backdrop_path(fx.library);
    let removed = (fx.send_as(&fx.server.admin_token, Method::DELETE, &path, &[])).await;

    assert_eq!(removed.status, 204);
    let library = fx.listed().await;
    assert!(!library.custom_backdrop);
    assert_eq!(library.backdrop_url, fx.movie_backdrop(ALIEN).await);
}

#[tokio::test]
async fn deleting_a_library_deletes_its_custom_image() {
    let fx = Fixture::alien().await;
    fx.upload_backdrop().await;
    let url = fx.listed().await.backdrop_url.unwrap();
    let folder = fx
        .server
        .data_dir()
        .join(format!("custom-images/library/{}", fx.library));
    assert!(folder.exists());

    fx.libraries_as(&fx.server.admin_token)
        .delete_library(DeleteLibraryRequest {
            id: fx.library,
            ..Default::default()
        })
        .await
        .unwrap();

    let token = Some(fx.server.admin_token.as_str());
    assert_eq!(fetch(&fx.server, &url, token).await.status, 404);
    assert!(!folder.exists(), "{} is still there", folder.display());
}

#[tokio::test]
async fn an_image_for_a_library_that_isnt_there_is_not_found_and_not_kept() {
    let fx = Fixture::alien().await;
    let missing = fx.library + 1;

    let token = &fx.server.admin_token;
    let refused = (fx.send_as(token, Method::PUT, &backdrop_path(missing), &png())).await;

    assert_eq!(refused.status, 404);
    let folder = format!("custom-images/library/{missing}");
    assert!(!fx.server.data_dir().join(folder).exists());
}
