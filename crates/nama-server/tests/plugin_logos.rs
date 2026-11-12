//! A Plugin's logo: its Manifest's, cleaned and served by URL, and the Custom Image an admin with
//! Manage Settings uploads in its place (ADR 0036, ADR 0037), over a real socket.

#[path = "support/http.rs"]
mod http;
#[path = "support/server.rs"]
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
mod server;
#[path = "support/test_plugin.rs"]
mod test_plugin;

use std::io::Cursor;

use connectrpc::client::HttpClient;
use http::{Reply, fetch, send};
use http_body_util::Full;
use hyper::{Method, Request, body::Bytes};
use image::{ImageFormat, RgbImage};
use nama_proto::nama::v1::{ListPluginsRequest, PluginServiceClient, PluginSummary};
use nama_server::plugins::Registry;
use server::TestServer;

/// Where TMDB's logo is uploaded to and removed from.
const TMDB_LOGO: &str = "/custom-images/plugin/tmdb/logo";

/// An SVG with a script and an event handler, which the Server strips.
const SCRIPTED_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" onload="alert(1)">
    <script>alert(2)</script><circle r="4"/></svg>"#;

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

    /// The URL of the Plugin `id`'s logo and whether it is a Custom Image.
    async fn logo(&self, id: &str) -> (String, bool) {
        let plugin = self.listed(id).await.unwrap();
        (plugin.logo_url, plugin.custom_logo)
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

    let (url, custom) = fx.logo("test-plugin").await;

    assert!(url.starts_with("/media/plugin-logos/test-plugin/"), "{url}");
    assert!(!custom);
    assert_safe_svg(&fx.fetch(&url).await);
    assert_eq!(fetch(&fx.server, &url, None).await.status, 401);
}

#[tokio::test]
async fn a_png_uploaded_as_a_plugins_logo_is_the_one_listed() {
    let fx = Fixture::start().await;

    let uploaded = fx.upload(TMDB_LOGO, &png()).await;
    let (url, custom) = fx.logo("tmdb").await;
    let served = fx.fetch(&url).await;

    assert_eq!(uploaded.status, 204);
    assert!(
        url.starts_with("/media/custom-images/plugin/tmdb/logo/"),
        "{url}"
    );
    assert!(custom);
    assert_eq!(served.header("content-type"), "image/png");
    assert_eq!(served.body, png());
}

#[tokio::test]
async fn an_svg_uploaded_as_a_plugins_logo_is_listed_and_served_without_its_script() {
    let fx = Fixture::start().await;

    let uploaded = fx.upload(TMDB_LOGO, SCRIPTED_SVG.as_bytes()).await;
    let (url, custom) = fx.logo("tmdb").await;

    assert_eq!(uploaded.status, 204);
    assert!(custom);
    assert_safe_svg(&fx.fetch(&url).await);
}

#[tokio::test]
async fn an_svg_over_64_kib_or_not_an_image_is_refused_as_a_plugins_logo_and_changes_nothing() {
    let fx = Fixture::start().await;
    let manifests = fx.logo("tmdb").await;
    let padding = " ".repeat(64 * 1024);
    let over_64_kib = format!(r#"<svg xmlns="http://www.w3.org/2000/svg">{padding}</svg>"#);

    for (bytes, status) in [
        (over_64_kib.into_bytes(), 413),
        (
            b"<html xmlns='http://www.w3.org/1999/xhtml'/>".to_vec(),
            415,
        ),
        (b"a text file named logo.svg".to_vec(), 415),
    ] {
        let refused = fx.upload(TMDB_LOGO, &bytes).await;

        assert_eq!(refused.status, status);
        assert_eq!(fx.logo("tmdb").await, manifests);
    }
}

#[tokio::test]
async fn removing_a_plugins_custom_image_brings_its_manifests_logo_back() {
    let fx = Fixture::start().await;
    let manifests = fx.logo("tmdb").await;
    fx.upload(TMDB_LOGO, &png()).await;
    let (custom, _) = fx.logo("tmdb").await;

    let removed = fx.remove(TMDB_LOGO).await;

    assert_eq!(removed.status, 204);
    assert_eq!(fx.logo("tmdb").await, manifests);
    assert_eq!(fx.fetch(&custom).await.status, 404);
    assert_eq!(fx.fetch(&manifests.0).await.status, 200);
}

#[tokio::test]
async fn a_plugins_custom_image_outlives_its_package_being_removed_and_replaced() {
    let fx = Fixture::start().await;
    let wasm = test_plugin::build("");
    fx.package(Some(&wasm));
    fx.listed("test-plugin").await;
    fx.upload("/custom-images/plugin/test-plugin/logo", &png())
        .await;
    let (custom, _) = fx.logo("test-plugin").await;

    fx.package(None);
    assert_eq!(fx.listed("test-plugin").await, None);
    fx.package(Some(&wasm));

    assert_eq!(fx.logo("test-plugin").await, (custom, true));
}
