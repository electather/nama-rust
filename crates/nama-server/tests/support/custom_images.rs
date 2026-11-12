//! Uploading and removing Custom Images as the Admin, and the images and titles to do it with, for
//! tests of what shows them.

use std::io::Cursor;

use http_body_util::Full;
use hyper::{Method, Request, body::Bytes};
use image::{ImageFormat, RgbImage};
use nama_proto::nama::v1::{GetShowRequest, GetShowResponse, LibraryKind};

use crate::{
    browsing::{Fixture, tmdb_id},
    http::{Reply, send},
    server::{TestServer, tmdb::DARK},
};

/// TMDB Matches it to Dark, by its title and year.
pub(crate) const DARK_S01E01: &str = "Dark (2017)/Season 01/Dark - S01E01.mkv";

/// A `width` by `height` image in `format`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
pub(crate) fn image(format: ImageFormat, width: u32, height: u32) -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    RgbImage::new(width, height)
        .write_to(&mut bytes, format)
        .unwrap();
    bytes.into_inner()
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
impl Fixture {
    /// Uploads `bytes` to `path` as the Admin.
    pub(crate) async fn upload(&self, path: &str, bytes: &[u8]) -> Reply {
        let body = Full::new(Bytes::copy_from_slice(bytes));
        let request = Request::put(path).body(body).unwrap();
        send(&self.server, request, Some(&self.server.admin_token)).await
    }

    /// Removes the Custom Image at `path` as the Admin.
    pub(crate) async fn remove(&self, path: &str) -> Reply {
        let request = Request::builder()
            .method(Method::DELETE)
            .uri(path)
            .body(Full::new(Bytes::new()))
            .unwrap();
        send(&self.server, request, Some(&self.server.admin_token)).await
    }

    /// Starts a Server with a Shows Library holding Dark's first Episode, scanned and Matched.
    pub(crate) async fn dark() -> Self {
        let fx = Self::start_on(LibraryKind::LIBRARY_KIND_SHOWS, TestServer::builder()).await;
        fx.write(DARK_S01E01, b"video");
        fx.scan_and_match().await;
        fx
    }

    /// Dark as `GetShow` gives it in the fixture's Library.
    pub(crate) async fn get_dark(&self) -> GetShowResponse {
        self.browse
            .get_show(GetShowRequest {
                library_id: self.library,
                show_id: tmdb_id(DARK).into(),
                ..Default::default()
            })
            .await
            .unwrap()
            .into_owned()
    }
}
