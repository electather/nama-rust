//! A local stand-in for Radarr serving the responses recorded in `plugins/radarr/fixtures/`, so
//! the Radarr Plugin's tests need no Radarr. See that folder's `README.md` for which answers what.

#[path = "fake_servarr.rs"]
mod fake_servarr;

use std::{
    ops::{Deref, DerefMut},
    sync::{Arc, Mutex},
};

use axum::{
    Router,
    body::Bytes,
    extract::State,
    http::{Method, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use fake_servarr::{FakeServarr, Servarr, json, lock};
use serde_json::Value;

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
}

impl FakeRadarr {
    /// Starts serving on a free local port, already having the Movies with the TMDB ids
    /// `existing`, and finding none with the TMDB ids `not_found`.
    pub(crate) async fn start(existing: &[i64], not_found: &[i64]) -> Self {
        let state = Arc::new(Fake {
            existing: existing.to_vec(),
            not_found: not_found.to_vec(),
            added: Mutex::default(),
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
    match (method, uri.path()) {
        (Method::GET, "/api/v3/movie") => movie(&fake, &uri),
        (Method::POST, "/api/v3/movie") => add_movie(&fake, &body),
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
