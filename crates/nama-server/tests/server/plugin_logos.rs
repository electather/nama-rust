//! A Plugin's logo: always its Manifest's, cleaned and served by URL (ADR 0037), over a real
//! socket.

use crate::support::{http, server, test_plugin};

use std::io::Cursor;

use connectrpc::client::HttpClient;
use http::{Reply, fetch, send};
use http_body_util::Full;
use hyper::{Method, Request, body::Bytes};
use image::{ImageFormat, RgbImage};
use nama_proto::nama::v1::{ListPluginsRequest, PluginServiceClient, PluginSummary};
use nama_server::{backup, plugins::Registry};
use server::TestServer;

/// Where TMDB's logo was uploaded to and removed from while a Plugin's logo could be a Custom Image.
const TMDB_LOGO: &str = "/custom-images/plugin/tmdb/logo";

/// A running Server with its Built-in Plugins and no Installed ones yet.
struct Fixture {
    server: TestServer,
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
impl Fixture {
    async fn start() -> Self {
        let plugins = Registry::with_builtins();
        let server = TestServer::builder().plugins(plugins).start().await;
        Self { server }
    }

    /// The Plugin `id` as `ListPlugins` lists it to the Admin, which also reads `plugins/`.
    async fn listed(&self, id: &str) -> Option<PluginSummary> {
        PluginServiceClient::new(HttpClient::plaintext(), self.server.client_config())
            .list_plugins(ListPluginsRequest::default())
            .await
            .unwrap()
            .into_owned()
            .summaries
            .into_iter()
            .find(|plugin| plugin.id == id)
    }

    /// The URL of the Plugin `id`'s logo.
    async fn logo(&self, id: &str) -> String {
        self.listed(id).await.unwrap().logo_url
    }

    /// Writes the test Plugin's Package into `plugins/` as `test.wasm`, or with `None` deletes it.
    fn package(&self, wasm: Option<&[u8]>) {
        let dir = self.server.data_dir().join("plugins");
        std::fs::create_dir_all(&dir).unwrap();
        match wasm {
            Some(wasm) => std::fs::write(dir.join("test.wasm"), wasm).unwrap(),
            None => std::fs::remove_file(dir.join("test.wasm")).unwrap(),
        }
    }

    /// Uploads `bytes` to `path` as the Admin.
    async fn upload(&self, path: &str, bytes: &[u8]) -> Reply {
        let body = Full::new(Bytes::copy_from_slice(bytes));
        let request = Request::put(path).body(body).unwrap();
        send(&self.server, request, Some(&self.server.admin_token)).await
    }

    /// Removes the Custom Image at `path` as the Admin.
    async fn remove(&self, path: &str) -> Reply {
        let request = Request::builder()
            .method(Method::DELETE)
            .uri(path)
            .body(Full::new(Bytes::new()))
            .unwrap();
        send(&self.server, request, Some(&self.server.admin_token)).await
    }

    /// The image at `url`, fetched as the Admin.
    async fn fetch(&self, url: &str) -> Reply {
        fetch(&self.server, url, Some(&self.server.admin_token)).await
    }
}

/// A 2 by 2 PNG.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn png() -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    RgbImage::new(2, 2)
        .write_to(&mut bytes, ImageFormat::Png)
        .unwrap();
    bytes.into_inner()
}

/// Asserts `served` is an SVG with neither a script nor an event handler, and the headers that
/// keep it from running when opened in its own tab.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn assert_safe_svg(served: &Reply) {
    assert_eq!(served.status, 200);
    let svg = String::from_utf8(served.body.clone()).unwrap();
    assert!(svg.contains("<svg"), "{svg}");
    for gone in ["<script", "onload", "alert"] {
        assert!(!svg.contains(gone), "{svg}");
    }
    assert_eq!(served.header("content-type"), "image/svg+xml");
    assert_eq!(
        served.header("content-security-policy"),
        "default-src 'none'; style-src 'unsafe-inline'; sandbox"
    );
    assert_eq!(served.header("x-content-type-options"), "nosniff");
    assert_eq!(served.header("content-disposition"), "attachment");
    assert_eq!(
        served.header("cache-control"),
        "private, max-age=31536000, immutable"
    );
}

#[tokio::test]
async fn a_packages_logo_is_served_by_url_without_its_script_and_only_with_a_session() {
    let fx = Fixture::start().await;
    fx.package(Some(&test_plugin::build("")));

    let url = fx.logo("test-plugin").await;

    assert!(url.starts_with("/media/plugin-logos/test-plugin/"), "{url}");
    assert_safe_svg(&fx.fetch(&url).await);
    assert_eq!(fetch(&fx.server, &url, None).await.status, 401);
}

#[tokio::test]
async fn uploading_or_removing_a_plugins_logo_is_not_found_and_keeps_its_manifests() {
    let fx = Fixture::start().await;
    let manifests = fx.logo("tmdb").await;

    let uploaded = fx.upload(TMDB_LOGO, &png()).await;
    let removed = fx.remove(TMDB_LOGO).await;

    assert_eq!((uploaded.status, removed.status), (404, 404));
    assert_eq!(fx.logo("tmdb").await, manifests);
    assert!(!fx.server.data_dir().join("custom-images").exists());
}

#[tokio::test]
async fn startup_deletes_plugins_former_custom_images_so_a_backup_and_restore_hold_none() {
    let fx = Fixture::start().await;
    let images = fx.server.data_dir().join("custom-images");
    let (plugin_logo, poster) = ("plugin/tmdb/logo/v1", "movie/tmdb-348/poster/v1");
    for stored in [plugin_logo, poster] {
        let path = images.join(stored);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, png()).unwrap();
    }

    let server = TestServer::builder()
        .plugins(Registry::with_builtins())
        .restart(fx.server)
        .await;
    let elsewhere = tempfile::tempdir().unwrap();
    let (config, tar) = (
        elsewhere.path().join("nama.toml"),
        elsewhere.path().join("b.tar"),
    );
    backup::write(server.data_dir(), &config, &tar)
        .await
        .unwrap();
    let restored = elsewhere.path().join("restored");
    backup::restore(&restored, &config, &tar).await.unwrap();

    assert!(!images.join("plugin").exists());
    assert!(images.join(poster).is_file());
    assert!(!restored.join("custom-images/plugin").exists());
    assert!(restored.join("custom-images").join(poster).is_file());
}
