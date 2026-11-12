//! A local stand-in for Radarr serving the responses recorded in `plugins/radarr/fixtures/`, so
//! the Radarr Plugin's tests need no Radarr. See that folder's `README.md` for which answers what.

use std::{
    ops::{Deref, DerefMut},
    path::PathBuf,
    sync::{Arc, Mutex},
};

use crate::fake_servarr::{FakeServarr, Servarr, json, lock};
use axum::{
    Router,
    body::Bytes,
    extract::State,
    http::{Method, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use serde_json::Value;

use crate::server::TestServer;

/// The API key the fake Radarr accepts.
pub(crate) const API_KEY: &str = "test-radarr-key";

const RADARR: Servarr = Servarr {
    api_key: API_KEY,
    fixtures: "plugins/radarr/fixtures",
    queue_include: "includeMovie",
};

/// A fake Radarr serving on a local port while up, until dropped. What it shares with the fake
/// Sonarr, such as going down or its queue, it does as its [`FakeServarr`].
pub(crate) struct FakeRadarr {
    servarr: FakeServarr,
    state: Arc<Fake>,
}

struct Fake {
    /// The TMDB ids of the Movies it already has.
    existing: Vec<i64>,
    /// The TMDB ids it can't find.
    not_found: Vec<i64>,
    /// The body of each request that added a Movie, in order.
    added: Mutex<Vec<Value>>,
    /// Each request but a `GET`, in order.
    changes: Mutex<Vec<Changed>>,
    /// The file [`Changed::watched_was_there`] looks for.
    watched: Mutex<Option<PathBuf>>,
}

/// A request to the fake Radarr that may change something: any but a `GET`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Changed {
    /// Its method and path, such as `PUT /api/v3/movie/editor`.
    pub(crate) request: String,
    pub(crate) body: Value,
    /// Whether the file [`FakeRadarr::watch`] names was still there when it came.
    pub(crate) watched_was_there: bool,
}

impl FakeRadarr {
    /// Starts serving on a free local port, already having the Movies with the TMDB ids
    /// `existing`, and finding none with the TMDB ids `not_found`.
    pub(crate) async fn start(existing: &[i64], not_found: &[i64]) -> Self {
        let state = Arc::new(Fake {
            existing: existing.to_vec(),
            not_found: not_found.to_vec(),
            added: Mutex::default(),
            changes: Mutex::default(),
            watched: Mutex::default(),
        });
        let routes = Router::new()
            .fallback(answer)
            .with_state(Arc::clone(&state));
        Self {
            servarr: FakeServarr::start(RADARR, routes).await,
            state,
        }
    }

    /// The body of each request that added a Movie so far, in order.
    pub(crate) fn added(&self) -> Vec<Value> {
        lock(&self.state.added).clone()
    }

    /// Each request so far but a `GET`, in order.
    pub(crate) fn changes(&self) -> Vec<Changed> {
        lock(&self.state.changes).clone()
    }

    /// Has each change from now on say whether the file at `path` was there when it came.
    pub(crate) fn watch(&self, path: PathBuf) {
        *lock(&self.state.watched) = Some(path);
    }

    /// Has `server`'s Admin set the Radarr Plugin up against this fake, with the quality profile
    /// `profile` and the root folder `/movies`, and enable it.
    pub(crate) async fn enable_on(&self, server: &TestServer, profile: &str) {
        self.servarr
            .enable_as(server, "radarr", profile, "/movies")
            .await;
    }
}

impl Deref for FakeRadarr {
    type Target = FakeServarr;

    fn deref(&self) -> &FakeServarr {
        &self.servarr
    }
}

impl DerefMut for FakeRadarr {
    fn deref_mut(&mut self) -> &mut FakeServarr {
        &mut self.servarr
    }
}

async fn answer(State(fake): State<Arc<Fake>>, method: Method, uri: Uri, body: Bytes) -> Response {
    if method != Method::GET {
        let watched_was_there = lock(&fake.watched)
            .as_ref()
            .is_some_and(|path| path.exists());
        lock(&fake.changes).push(Changed {
            request: format!("{method} {}", uri.path()),
            body: serde_json::from_slice(&body).unwrap_or_default(),
            watched_was_there,
        });
    }
    match (method, uri.path()) {
        (Method::GET, "/api/v3/movie") => movie(&fake, &uri),
        (Method::POST, "/api/v3/movie") => add_movie(&fake, &body),
        // Radarr answers with the Movies it changed, which the Plugin doesn't read.
        (Method::PUT, "/api/v3/movie/editor") => {
            json(StatusCode::ACCEPTED, &Value::Array(Vec::new()))
        }
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}

/// The Movie with the TMDB id `?tmdbId=` names, if it has it: only Alien has a fixture.
fn movie(fake: &Fake, uri: &Uri) -> Response {
    let tmdb_id = uri
        .query()
        .and_then(|query| query.strip_prefix("tmdbId="))
        .and_then(|id| id.parse::<i64>().ok());
    match tmdb_id {
        Some(id) if fake.existing.contains(&id) => {
            RADARR.fixture(StatusCode::OK, "movie-alien.json")
        }
        _ => json(StatusCode::OK, &Value::Array(Vec::new())),
    }
}

fn add_movie(fake: &Fake, body: &[u8]) -> Response {
    let movie: Value = serde_json::from_slice(body).unwrap_or_default();
    let tmdb_id = movie
        .get("tmdbId")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    if fake.existing.contains(&tmdb_id) {
        RADARR.fixture(StatusCode::BAD_REQUEST, "movie-exists.json")
    } else if fake.not_found.contains(&tmdb_id) {
        RADARR.fixture(StatusCode::BAD_REQUEST, "movie-not-found.json")
    } else {
        lock(&fake.added).push(movie);
        RADARR.fixture(StatusCode::CREATED, "movie.json")
    }
}
