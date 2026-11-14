//! A local stand-in for TVDB's v4 API and its artwork host, serving the responses recorded in
//! `plugins/tvdb/fixtures/` for Severance, so the TVDB Plugin's tests need no TVDB. See that
//! folder's `README.md` for which answers what. Any artwork path answers with a small JPEG.

use std::{
    collections::HashMap,
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, Mutex, PoisonError},
};

use axum::{
    Router,
    body::Bytes,
    extract::{Query, State},
    http::{HeaderMap, Method, StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use connectrpc::client::HttpClient;
use image::ImageFormat;
use nama_proto::nama::v1::{
    EnablePluginRequest, PluginServiceClient, SettingValue, UpdatePluginSettingsRequest,
    setting_value,
};
use serde_json::Value;
use tokio::{net::TcpListener, task::JoinHandle};

use crate::{custom_images::image, server::TestServer};

/// Severance's TVDB id, the one Show the fake knows.
pub(crate) const SEVERANCE: i64 = 371_980;

/// The API key the fake TVDB accepts.
const API_KEY: &str = "test-tvdb-key";

/// The token `login.json` hands out.
const TOKEN: &str = "fixture-token";

/// A fake TVDB serving on a local port until dropped.
pub(crate) struct FakeTvdb {
    addr: SocketAddr,
    requests: Arc<Mutex<Vec<String>>>,
    server: JoinHandle<()>,
}

impl FakeTvdb {
    /// Starts serving on a free local port.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    pub(crate) async fn start() -> Self {
        let requests = Arc::<Mutex<Vec<String>>>::default();
        let app = Router::new()
            .fallback(answer)
            .with_state(Arc::clone(&requests));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let _: Result<(), _> = axum::serve(listener, app).await;
        });
        Self {
            addr,
            requests,
            server,
        }
    }

    /// Every request served so far, as its path and query, in order.
    pub(crate) fn requests(&self) -> Vec<String> {
        lock(&self.requests).clone()
    }

    /// Has `server`'s Admin set the TVDB Plugin up against this fake, its API and artwork URLs
    /// included, and enable it.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    pub(crate) async fn enable_on(&self, server: &TestServer) {
        let plugins = PluginServiceClient::new(HttpClient::plaintext(), server.client_config());
        let url = format!("http://{}", self.addr);
        let settings = [
            ("api_key", API_KEY.to_owned()),
            ("api_url", format!("{url}/v4")),
            ("artwork_url", url),
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
                id: "tvdb".to_owned(),
                settings: settings.into_iter().collect(),
                ..Default::default()
            })
            .await
            .unwrap();
        plugins
            .enable_plugin(EnablePluginRequest {
                id: "tvdb".to_owned(),
                ..Default::default()
            })
            .await
            .unwrap();
    }
}

impl Drop for FakeTvdb {
    /// Stops serving: dropping a task's handle alone would leave it running.
    fn drop(&mut self) {
        self.server.abort();
    }
}

async fn answer(
    State(requests): State<Arc<Mutex<Vec<String>>>>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    lock(&requests).push(uri.to_string());
    let query: HashMap<String, String> = Query::try_from_uri(&uri)
        .map(|Query(query)| query)
        .unwrap_or_default();
    let path = uri.path();
    if path.starts_with("/banners/") {
        let jpeg = image(ImageFormat::Jpeg, 2, 3);
        return ([(header::CONTENT_TYPE, "image/jpeg")], jpeg).into_response();
    }
    if method == Method::POST && path == "/v4/login" {
        let login: Value = serde_json::from_slice(&body).unwrap_or_default();
        return if login.get("apikey") == Some(&Value::from(API_KEY)) {
            fixture("login.json")
        } else {
            StatusCode::UNAUTHORIZED.into_response()
        };
    }
    let bearer = format!("Bearer {TOKEN}");
    if headers
        .get(header::AUTHORIZATION)
        .is_none_or(|token| *token != *bearer)
    {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let severance = format!("/v4/series/{SEVERANCE}");
    let param = |key: &str| query.get(key).map_or("", String::as_str);
    match path {
        "/v4/search" if param("query").eq_ignore_ascii_case("severance") => fixture("search.json"),
        "/v4/search" => (
            [(header::CONTENT_TYPE, "application/json")],
            r#"{"status":"success","data":[]}"#,
        )
            .into_response(),
        _ if path == format!("{severance}/extended") => fixture("series-extended.json"),
        _ if path == format!("{severance}/episodes/default") && param("season") == "1" => {
            fixture("episodes-season-1.json")
        }
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}

/// The recorded response `file`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn fixture(file: &str) -> Response {
    let folder = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../plugins/tvdb/fixtures");
    let body = std::fs::read(folder.join(file)).unwrap();
    ([(header::CONTENT_TYPE, "application/json")], body).into_response()
}

/// Locks `mutex`, even one a panicking test poisoned.
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
