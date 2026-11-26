//! Finding and waiting for the copies the Server makes of an image ahead of a request, at the
//! widths it offers that image at.

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use connectrpc::client::HttpClient;
use nama_proto::nama::v1::{GetServerInfoRequest, GetServerInfoResponse, ServerServiceClient};

use super::browsing::Fixture;

/// Waits until a file is at `path`, failing the test after 60 seconds.
pub(crate) async fn until_exists(path: &Path) {
    let made = tokio::time::timeout(Duration::from_secs(60), async {
        while !path.exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    assert!(made.is_ok(), "{} was never made", path.display());
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
impl Fixture {
    /// Where the copies of the image served at `url`, under `/media/`, are kept, for an image
    /// whose key is its URL's path.
    pub(crate) fn copies_of(&self, url: &str) -> PathBuf {
        let key = url.strip_prefix("/media/").unwrap();
        self.server.data_dir().join("resized").join(key)
    }

    /// What the Server tells clients about itself, the widths it offers each kind of image at
    /// among it.
    pub(crate) async fn server_info(&self) -> GetServerInfoResponse {
        let client = ServerServiceClient::new(HttpClient::plaintext(), self.server.client_config());
        let info = client.get_server_info(GetServerInfoRequest::default());
        info.await.unwrap().into_owned()
    }
}
