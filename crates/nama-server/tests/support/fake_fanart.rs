//! A local stand-in for fanart.tv's API and its assets host, serving Heat's lists recorded in
//! `plugins/fanart/fixtures/movie.json`, so the fanart.tv Plugin's tests need no fanart.tv. Any
//! asset path answers with [`asset`].

use std::{io::Cursor, net::SocketAddr, path::PathBuf};

use axum::{
    Router,
    http::{StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use connectrpc::client::HttpClient;
use image::{ImageFormat, RgbImage};
use nama_proto::nama::v1::{
    EnablePluginRequest, PluginServiceClient, SettingValue, UpdatePluginSettingsRequest,
    setting_value,
};
use tokio::{net::TcpListener, task::JoinHandle};

use crate::server::TestServer;

/// A fake fanart.tv serving on a local port until dropped.
pub(crate) struct FakeFanart {
    addr: SocketAddr,
    server: JoinHandle<()>,
}

impl FakeFanart {
    /// Starts serving on a free local port.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    pub(crate) async fn start() -> Self {
        let app = Router::new().fallback(answer);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let _: Result<(), _> = axum::serve(listener, app).await;
        });
        Self { addr, server }
    }

    /// Has `server`'s Admin set the fanart.tv Plugin up against this fake, with an API key and
    /// its API and assets URLs, and enable it.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    pub(crate) async fn enable_on(&self, server: &TestServer) {
        let plugins = PluginServiceClient::new(HttpClient::plaintext(), server.client_config());
        let url = format!("http://{}", self.addr);
        let settings = [
            ("api_key", "test-fanart-key".to_owned()),
            ("api_url", url.clone()),
            ("assets_url", url),
        ]
        .map(|(key, value)| {
            let value = SettingValue {
                value: Some(setting_value::Value::TextValue(value)),
                ..Default::default()
            };
            (key.to_owned(), value)
        });
        plugins
            .update_plugin_settings(UpdatePluginSettingsRequest {
                id: "fanart".to_owned(),
                settings: settings.into_iter().collect(),
                ..Default::default()
            })
            .await
            .unwrap();
        plugins
            .enable_plugin(EnablePluginRequest {
                id: "fanart".to_owned(),
                ..Default::default()
            })
            .await
            .unwrap();
    }
}

impl Drop for FakeFanart {
    /// Stops serving: dropping a task's handle alone would leave it running.
    fn drop(&mut self) {
        self.server.abort();
    }
}

/// The bytes every asset the fake serves answers with: a tiny PNG.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
pub(crate) fn asset() -> Vec<u8> {
    let mut png = Cursor::new(Vec::new());
    RgbImage::new(2, 1)
        .write_to(&mut png, ImageFormat::Png)
        .unwrap();
    png.into_inner()
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn answer(uri: Uri) -> Response {
    let path = uri.path();
    if path.starts_with("/fanart/") {
        return ([(header::CONTENT_TYPE, "image/png")], asset()).into_response();
    }
    if path == "/movies/949" {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../plugins/fanart/fixtures/movie.json");
        let body = std::fs::read(fixture).unwrap();
        return ([(header::CONTENT_TYPE, "application/json")], body).into_response();
    }
    StatusCode::NOT_FOUND.into_response()
}
