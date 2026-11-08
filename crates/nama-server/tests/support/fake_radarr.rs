//! A local stand-in for Radarr serving the responses recorded in `plugins/radarr/fixtures/`, so
//! the Radarr Plugin's tests need no Radarr. See that folder's `README.md` for which answers what.

use std::{
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, Mutex, PoisonError},
};

use axum::{
    Router,
    body::Bytes,
    extract::State,
    http::{HeaderMap, Method, StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use serde_json::Value;
use tokio::{net::TcpListener, task::JoinHandle};

/// The API key the fake Radarr accepts.
pub(crate) const API_KEY: &str = "test-radarr-key";

/// A fake Radarr serving on a local port while up, until dropped.
pub(crate) struct FakeRadarr {
    addr: SocketAddr,
    state: Arc<Fake>,
    server: Option<JoinHandle<()>>,
}

struct Fake {
    /// The TMDB ids of the Movies it already has.
    has: Vec<i64>,
    /// The TMDB ids it can't find.
    unknown: Vec<i64>,
    /// The body of each request that added a Movie, in order.
    added: Mutex<Vec<Value>>,
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
impl FakeRadarr {
    /// Starts serving on a free local port, already having the Movies with the TMDB ids `has`,
    /// and finding none with the TMDB ids `unknown`.
    pub(crate) async fn start(has: &[i64], unknown: &[i64]) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut radarr = Self {
            addr: listener.local_addr().unwrap(),
            state: Arc::new(Fake {
                has: has.to_vec(),
                unknown: unknown.to_vec(),
                added: Mutex::default(),
            }),
            server: None,
        };
        radarr.serve(listener);
        radarr
    }

    /// Where it serves, as its URL setting names it.
    pub(crate) fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Stops answering, so connecting to it is refused, as for a Radarr that is down.
    pub(crate) fn go_down(&mut self) {
        if let Some(server) = self.server.take() {
            server.abort();
        }
    }

    /// Answers again on the same port.
    pub(crate) async fn come_back(&mut self) {
        let listener = TcpListener::bind(self.addr).await.unwrap();
        self.serve(listener);
    }

    /// The body of each request that added a Movie so far, in order.
    pub(crate) fn added(&self) -> Vec<Value> {
        self.state
            .added
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn serve(&mut self, listener: TcpListener) {
        let app = Router::new()
            .fallback(answer)
            .with_state(Arc::clone(&self.state));
        self.server = Some(tokio::spawn(async move {
            let _: Result<(), _> = axum::serve(listener, app).await;
        }));
    }
}

impl Drop for FakeRadarr {
    /// Stops serving: dropping a task's handle alone would leave it running.
    fn drop(&mut self) {
        self.go_down();
    }
}

async fn answer(
    State(fake): State<Arc<Fake>>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if headers.get("x-api-key").is_none_or(|key| key != API_KEY) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    match (method, uri.path()) {
        (Method::GET, "/api/v3/qualityprofile") => fixture(StatusCode::OK, "qualityprofile.json"),
        (Method::GET, "/api/v3/rootfolder") => fixture(StatusCode::OK, "rootfolder.json"),
        (Method::POST, "/api/v3/movie") => add_movie(&fake, &body),
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}

fn add_movie(fake: &Fake, body: &[u8]) -> Response {
    let movie: Value = serde_json::from_slice(body).unwrap_or_default();
    let tmdb_id = movie
        .get("tmdbId")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    if fake.has.contains(&tmdb_id) {
        fixture(StatusCode::BAD_REQUEST, "movie-exists.json")
    } else if fake.unknown.contains(&tmdb_id) {
        fixture(StatusCode::BAD_REQUEST, "movie-not-found.json")
    } else {
        fake.added
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(movie);
        fixture(StatusCode::CREATED, "movie.json")
    }
}

/// The recorded response `file`, with `status`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn fixture(status: StatusCode, file: &str) -> Response {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../plugins/radarr/fixtures");
    let body = std::fs::read(fixtures.join(file)).unwrap();
    (status, [(header::CONTENT_TYPE, "application/json")], body).into_response()
}
