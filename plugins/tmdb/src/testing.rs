//! A local stand-in for TMDB that serves the recorded responses in `fixtures/`, so tests never
//! touch the network. See `fixtures/README.md` for how requests map to responses.

use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::PathBuf,
    sync::{Arc, Mutex, PoisonError},
};

use axum::{
    Router,
    extract::{Query, State},
    http::{StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use tokio::net::TcpListener;

use crate::Endpoints;

/// The API key [`FakeTmdb`] accepts.
pub const API_KEY: &str = "test-api-key";

/// Query parameters that don't pick the response.
const IGNORED_PARAMS: &[&str] = &["api_key", "language", "include_adult"];

/// A fake TMDB serving recorded responses on a local port until dropped.
#[derive(Debug)]
pub struct FakeTmdb {
    endpoints: Endpoints,
    state: Arc<Fake>,
    server: tokio::task::JoinHandle<()>,
}

#[derive(Debug)]
struct Fake {
    routes: HashMap<String, String>,
    requests: Mutex<Vec<String>>,
}

impl FakeTmdb {
    /// Starts serving on a free local port.
    ///
    /// # Panics
    ///
    /// If the fixtures can't be read or no port can be bound: a test helper fails its test.
    #[expect(
        clippy::unwrap_used,
        clippy::expect_used,
        reason = "a test helper fails its test by panicking"
    )]
    pub async fn start() -> Self {
        let routes = fs::read_to_string(fixtures().join("routes.json")).unwrap();
        let state = Arc::new(Fake {
            routes: serde_json::from_str(&routes).expect("routes.json"),
            requests: Mutex::default(),
        });
        let app = Router::new()
            .fallback(answer)
            .with_state(Arc::clone(&state));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let _: Result<(), _> = axum::serve(listener, app).await;
        });
        Self {
            endpoints: Endpoints {
                api: format!("http://{addr}/3"),
                images: format!("http://{addr}/t/p"),
            },
            state,
            server,
        }
    }

    /// Where the Plugin should send its requests.
    #[must_use]
    pub fn endpoints(&self) -> Endpoints {
        self.endpoints.clone()
    }

    /// Every request served so far, as its path and query, in order.
    #[must_use]
    pub fn requests(&self) -> Vec<String> {
        lock(&self.state.requests).clone()
    }
}

impl Drop for FakeTmdb {
    /// Stops serving: dropping a task's handle alone would leave it running.
    fn drop(&mut self) {
        self.server.abort();
    }
}

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

async fn answer(
    State(fake): State<Arc<Fake>>,
    uri: Uri,
    Query(params): Query<BTreeMap<String, String>>,
) -> Response {
    lock(&fake.requests).push(uri.to_string());
    let path = uri.path();
    if let Some(image) = path.strip_prefix("/t/p/") {
        let file = image.rsplit('/').next().unwrap_or_default();
        return match read_fixture(fixtures().join("images").join(file)) {
            Some(bytes) => ([(header::CONTENT_TYPE, "image/jpeg")], bytes).into_response(),
            None => StatusCode::NOT_FOUND.into_response(),
        };
    }
    if params.get("api_key").map(String::as_str) != Some(API_KEY) {
        return json(
            StatusCode::UNAUTHORIZED,
            r#"{"success":false,"status_code":7,"status_message":"Invalid API key: You must be granted a valid key."}"#
                .to_owned(),
        );
    }
    let query: Vec<String> = params
        .iter()
        .filter(|(name, _)| !IGNORED_PARAMS.contains(&name.as_str()))
        .map(|(name, value)| format!("{name}={value}"))
        .collect();
    let route = if query.is_empty() {
        path.to_owned()
    } else {
        format!("{path}?{}", query.join("&"))
    };
    let body = fake
        .routes
        .get(&route)
        .and_then(|file| read_fixture(fixtures().join("api").join(file)));
    match body {
        Some(body) => json(StatusCode::OK, String::from_utf8_lossy(&body).into_owned()),
        None => json(
            StatusCode::NOT_FOUND,
            r#"{"success":false,"status_code":34,"status_message":"The resource you requested could not be found."}"#
                .to_owned(),
        ),
    }
}

fn read_fixture(path: PathBuf) -> Option<Vec<u8>> {
    fs::read(path).ok()
}

fn json(status: StatusCode, body: String) -> Response {
    (status, [(header::CONTENT_TYPE, "application/json")], body).into_response()
}
