//! A local stand-in for Radarr serving the responses recorded in `plugins/radarr/fixtures/`, so
//! the Radarr Plugin's tests need no Radarr. See that folder's `README.md` for which answers what.

use std::{
    net::SocketAddr,
    path::PathBuf,
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
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
    /// Whether it refuses [`API_KEY`] too, as after an Admin made Radarr a new one.
    refuses_key: AtomicBool,
    /// The TMDB ids of the Movies it already has.
    existing: Vec<i64>,
    /// The TMDB ids it can't find.
    not_found: Vec<i64>,
    /// The ids of the quality profiles removed from it, as when an Admin deletes one in Radarr.
    removed_profiles: Mutex<Vec<i64>>,
    /// The body of each request that added a Movie, in order.
    added: Mutex<Vec<Value>>,
    /// The fixture its queue answers with.
    queue: Mutex<&'static str>,
    /// How many times its queue was asked for.
    queue_calls: AtomicUsize,
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
impl FakeRadarr {
    /// Starts serving on a free local port, already having the Movies with the TMDB ids
    /// `existing`, and finding none with the TMDB ids `not_found`.
    pub(crate) async fn start(existing: &[i64], not_found: &[i64]) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut radarr = Self {
            addr: listener.local_addr().unwrap(),
            state: Arc::new(Fake {
                refuses_key: AtomicBool::default(),
                existing: existing.to_vec(),
                not_found: not_found.to_vec(),
                removed_profiles: Mutex::default(),
                added: Mutex::default(),
                queue: Mutex::new("queue-empty.json"),
                queue_calls: AtomicUsize::default(),
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

    /// Refuses every API key from now on, as when an Admin makes Radarr a new one.
    pub(crate) fn refuse_key(&self) {
        self.state.refuses_key.store(true, Ordering::Relaxed);
    }

    /// Stops offering the quality profile `id`, as when an Admin deletes it in Radarr.
    pub(crate) fn remove_profile(&self, id: i64) {
        self.state
            .removed_profiles
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(id);
    }

    /// Makes its queue answer with the fixture `file`, such as `queue.json`.
    pub(crate) fn set_queue(&self, file: &'static str) {
        *self
            .state
            .queue
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = file;
    }

    /// How many times its queue was asked for so far.
    pub(crate) fn queue_calls(&self) -> usize {
        self.state.queue_calls.load(Ordering::Relaxed)
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
    if fake.refuses_key.load(Ordering::Relaxed)
        || headers.get("x-api-key").is_none_or(|key| key != API_KEY)
    {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    match (method, uri.path()) {
        (Method::GET, "/api/v3/qualityprofile") => quality_profiles(&fake),
        (Method::GET, "/api/v3/rootfolder") => fixture(StatusCode::OK, "rootfolder.json"),
        (Method::GET, "/api/v3/movie") => movie(&fake, &uri),
        (Method::POST, "/api/v3/movie") => add_movie(&fake, &body),
        (Method::GET, "/api/v3/queue") => queue(&fake, &uri),
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}

/// Its quality profiles, but those removed.
fn quality_profiles(fake: &Fake) -> Response {
    let removed = fake
        .removed_profiles
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    let mut profiles = fixture_json("qualityprofile.json");
    if let Value::Array(profiles) = &mut profiles {
        profiles.retain(|profile| {
            profile["id"]
                .as_i64()
                .is_none_or(|id| !removed.contains(&id))
        });
    }
    json(&profiles)
}

/// The Movie with the TMDB id `?tmdbId=` names, if it has it: only Alien has a fixture.
fn movie(fake: &Fake, uri: &Uri) -> Response {
    let tmdb_id = uri
        .query()
        .and_then(|query| query.strip_prefix("tmdbId="))
        .and_then(|id| id.parse::<i64>().ok());
    match tmdb_id {
        Some(id) if fake.existing.contains(&id) => fixture(StatusCode::OK, "movie-alien.json"),
        _ => json(&Value::Array(Vec::new())),
    }
}

fn add_movie(fake: &Fake, body: &[u8]) -> Response {
    let movie: Value = serde_json::from_slice(body).unwrap_or_default();
    let tmdb_id = movie
        .get("tmdbId")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    if fake.existing.contains(&tmdb_id) {
        fixture(StatusCode::BAD_REQUEST, "movie-exists.json")
    } else if fake.not_found.contains(&tmdb_id) {
        fixture(StatusCode::BAD_REQUEST, "movie-not-found.json")
    } else {
        fake.added
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(movie);
        fixture(StatusCode::CREATED, "movie.json")
    }
}

fn queue(fake: &Fake, uri: &Uri) -> Response {
    // Without the Movie, a queue entry names it only by Radarr's own id.
    if !uri
        .query()
        .is_some_and(|query| query.contains("includeMovie=true"))
    {
        return StatusCode::BAD_REQUEST.into_response();
    }
    fake.queue_calls.fetch_add(1, Ordering::Relaxed);
    let file = *fake.queue.lock().unwrap_or_else(PoisonError::into_inner);
    fixture(StatusCode::OK, file)
}

/// The recorded response `file`, with `status`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn fixture(status: StatusCode, file: &str) -> Response {
    let body = std::fs::read(fixture_path(file)).unwrap();
    (status, [(header::CONTENT_TYPE, "application/json")], body).into_response()
}

/// `body` as a `200 OK` answer.
fn json(body: &Value) -> Response {
    let body = body.to_string();
    ([(header::CONTENT_TYPE, "application/json")], body).into_response()
}

/// The recorded response `file`, parsed.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn fixture_json(file: &str) -> Value {
    serde_json::from_slice(&std::fs::read(fixture_path(file)).unwrap()).unwrap()
}

fn fixture_path(file: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../plugins/radarr/fixtures")
        .join(file)
}
