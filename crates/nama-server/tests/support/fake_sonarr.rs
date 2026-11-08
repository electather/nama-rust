//! A local stand-in for Sonarr serving the responses recorded in `plugins/sonarr/fixtures/`, so
//! the Sonarr Plugin's tests need no Sonarr. See that folder's `README.md` for which answers what.
//! It keeps the Shows added to it, and changes their Seasons' monitoring as Sonarr does,
//! refresh after an add included.

use std::{
    collections::HashMap,
    net::SocketAddr,
    path::PathBuf,
    sync::{
        Arc, Mutex, MutexGuard, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use axum::{
    Router,
    body::Bytes,
    extract::{Query, State},
    http::{HeaderMap, Method, StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use serde_json::{Value, json};
use tokio::{net::TcpListener, task::JoinHandle};

/// The API key the fake Sonarr accepts.
pub(crate) const API_KEY: &str = "test-sonarr-key";

/// The TVDB id of Dark, the one Show the fake Sonarr can find (`series-lookup.json`).
pub(crate) const DARK_TVDB: i64 = 334_824;

/// How long the refresh Sonarr starts as it adds a Show takes, as one fetching it from TVDB does.
const REFRESH: Duration = Duration::from_millis(300);

/// A fake Sonarr serving on a local port while up, until dropped.
pub(crate) struct FakeSonarr {
    addr: SocketAddr,
    state: Arc<Fake>,
    server: Option<JoinHandle<()>>,
}

#[derive(Default)]
struct Fake {
    /// The Shows it has, each as it was added with its `id` and Seasons' monitoring kept current.
    shows: Mutex<Vec<Value>>,
    /// Whether the next list of its Shows by TVDB id answers as if it didn't have them.
    stale_find: AtomicBool,
    /// Whether the refreshes it starts as it adds a Show never finish.
    hold_refreshes: AtomicBool,
    /// The commands it queued, as `GET /api/v3/command` lists them.
    commands: Mutex<Vec<Value>>,
    /// The ids of the Shows whose refresh after their add finished, and so have Episodes.
    refreshed: Mutex<Vec<i64>>,
    /// The `(TVDB id, Season number)` of each Season searched for, in order.
    searches: Mutex<Vec<(i64, u64)>>,
    /// The ids of the Episodes monitored one by one, in order.
    monitored_episodes: Mutex<Vec<i64>>,
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
impl FakeSonarr {
    /// Starts serving on a free local port, having no Shows.
    pub(crate) async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut sonarr = Self {
            addr: listener.local_addr().unwrap(),
            state: Arc::default(),
            server: None,
        };
        sonarr.serve(listener);
        sonarr
    }

    /// Where it serves, as its URL setting names it.
    pub(crate) fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Stops answering, so connecting to it is refused, as for a Sonarr that is down.
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

    /// Makes the next list of its Shows by TVDB id answer as if it didn't have them, as when
    /// another hand-off adds the Show between this one's asking and its add.
    pub(crate) fn answer_next_find_stale(&self) {
        self.state.stale_find.store(true, Ordering::Relaxed);
    }

    /// Makes the refreshes it starts as it adds a Show from now on never finish.
    pub(crate) fn hold_refreshes(&self) {
        self.state.hold_refreshes.store(true, Ordering::Relaxed);
    }

    /// Returns once every refresh it started has finished, as Seasons' monitoring settles.
    pub(crate) async fn finish_refreshes(&self) {
        while lock(&self.state.commands)
            .iter()
            .any(|command| command["status"] == "started")
        {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    /// Each Show it has, as it was added and with its Seasons' monitoring as it is now.
    pub(crate) fn shows(&self) -> Vec<Value> {
        lock(&self.state.shows).clone()
    }

    /// The numbers of the Seasons of the Show `tvdb_id` that it monitors, in order.
    pub(crate) fn monitored_seasons(&self, tvdb_id: i64) -> Vec<u64> {
        let shows = lock(&self.state.shows);
        let Some(show) = shows.iter().find(|show| show["tvdbId"] == tvdb_id) else {
            return Vec::new();
        };
        seasons(show)
            .iter()
            .filter(|season| season["monitored"] == true)
            .filter_map(|season| season["seasonNumber"].as_u64())
            .collect()
    }

    /// The `(TVDB id, Season number)` of each Season searched for so far, in order.
    pub(crate) fn searches(&self) -> Vec<(i64, u64)> {
        lock(&self.state.searches).clone()
    }

    /// The ids of the Episodes monitored one by one so far, in order.
    pub(crate) fn monitored_episodes(&self) -> Vec<i64> {
        lock(&self.state.monitored_episodes).clone()
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

impl Drop for FakeSonarr {
    /// Stops serving: dropping a task's handle alone would leave it running.
    fn drop(&mut self) {
        self.go_down();
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
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
    let query: HashMap<String, String> = Query::try_from_uri(&uri)
        .map(|Query(query)| query)
        .unwrap_or_default();
    let body: Value = serde_json::from_slice(&body).unwrap_or_default();
    let param = |key: &str| query.get(key).map_or("", String::as_str);
    match (method, uri.path()) {
        (Method::GET, "/api/v3/qualityprofile") => fixture(StatusCode::OK, "qualityprofile.json"),
        (Method::GET, "/api/v3/rootfolder") => fixture(StatusCode::OK, "rootfolder.json"),
        (Method::GET, "/api/v3/series") => find(&fake, param("tvdbId")),
        (Method::GET, "/api/v3/series/lookup") => look_up(param("term")),
        (Method::POST, "/api/v3/series") => add_show(&fake, &body),
        (Method::PUT, "/api/v3/series/editor") => edit_shows(&fake, &body),
        (Method::POST, "/api/v3/seasonpass") => monitor_seasons(&fake, &body),
        (Method::GET, "/api/v3/episode") => {
            episodes(&fake, param("seriesId"), param("seasonNumber"))
        }
        (Method::PUT, "/api/v3/episode/monitor") => monitor_episodes(&fake, &body),
        (Method::GET, "/api/v3/command") => json(StatusCode::OK, &json!(*lock(&fake.commands))),
        (Method::POST, "/api/v3/command") => command(&fake, &body),
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}

/// As Sonarr lists its Shows with the TVDB id `tvdb_id`: the one it has, or `[]`.
fn find(fake: &Fake, tvdb_id: &str) -> Response {
    let stale = fake.stale_find.swap(false, Ordering::Relaxed);
    let shows = lock(&fake.shows);
    let tvdb_id = tvdb_id.parse::<i64>().ok();
    let had = shows
        .iter()
        .filter(|_| !stale)
        .find(|show| show["tvdbId"].as_i64() == tvdb_id);
    json(StatusCode::OK, &json!(had.into_iter().collect::<Vec<_>>()))
}

/// As Sonarr answers a lookup by `term` such as `tvdb:334824`: the Show as TVDB knows it, or `[]`
/// for one it can't find.
fn look_up(term: &str) -> Response {
    if term == format!("tvdb:{DARK_TVDB}") {
        fixture(StatusCode::OK, "series-lookup.json")
    } else {
        json(StatusCode::OK, &json!([]))
    }
}

fn add_show(fake: &Arc<Fake>, show: &Value) -> Response {
    let mut shows = lock(&fake.shows);
    let tvdb_id = show["tvdbId"].as_i64().unwrap_or_default();
    if shows.iter().any(|had| had["tvdbId"] == tvdb_id) {
        return fixture(StatusCode::BAD_REQUEST, "series-exists.json");
    }
    if show.pointer("/addOptions/searchForMissingEpisodes") == Some(&Value::Bool(true)) {
        lock(&fake.searches).extend(
            seasons(show)
                .iter()
                .filter(|season| season["monitored"] == true)
                .filter_map(|season| Some((tvdb_id, season["seasonNumber"].as_u64()?))),
        );
    }
    let id = i64::try_from(shows.len()).unwrap_or_default() + 1;
    let mut show = show.clone();
    if let Some(fields) = show.as_object_mut() {
        fields.insert("id".to_owned(), json!(id));
    }
    shows.push(show.clone());
    refresh(fake, id, show.get("seasons").cloned().unwrap_or_default());
    json(StatusCode::CREATED, &show)
}

/// Starts the refresh Sonarr starts as it adds the Show `id`, whose Seasons are `seasons` as it
/// reads them. Like Sonarr's, it saves them back as it read them, so it drops a Season monitored
/// meanwhile, and only then does the Show have Episodes.
fn refresh(fake: &Arc<Fake>, id: i64, seasons: Value) {
    let mut commands = lock(&fake.commands);
    let command = commands.len();
    commands.push(json!({
        "name": "RefreshSeries",
        "status": "started",
        "body": { "seriesIds": [id], "isNewSeries": true },
        "id": command + 1,
    }));
    if fake.hold_refreshes.load(Ordering::Relaxed) {
        return;
    }
    let fake = Arc::clone(fake);
    tokio::spawn(async move {
        tokio::time::sleep(REFRESH).await;
        if let Some(show) = lock(&fake.shows).iter_mut().find(|show| show["id"] == id) {
            show["seasons"] = seasons;
        }
        lock(&fake.refreshed).push(id);
        if let Some(Value::Object(fields)) = lock(&fake.commands).get_mut(command) {
            fields.insert("status".to_owned(), json!("completed"));
        }
    });
}

/// As Sonarr's series editor does for `monitorNewItems`, the one field the Plugin edits: sets it on
/// each Show named.
fn edit_shows(fake: &Fake, edit: &Value) -> Response {
    let mut shows = lock(&fake.shows);
    let ids = edit["seriesIds"].as_array().map_or(&[][..], Vec::as_slice);
    let edited: Vec<_> = shows
        .iter_mut()
        .filter(|show| ids.contains(&show["id"]))
        .map(|show| {
            show["monitorNewItems"] = edit["monitorNewItems"].clone();
            show.clone()
        })
        .collect();
    json(StatusCode::ACCEPTED, &Value::Array(edited))
}

/// As Sonarr's season pass does: sets the monitoring of only the Seasons named, of each Show.
fn monitor_seasons(fake: &Fake, pass: &Value) -> Response {
    let mut shows = lock(&fake.shows);
    for named in pass["series"].as_array().into_iter().flatten() {
        let Some(show) = shows.iter_mut().find(|show| show["id"] == named["id"]) else {
            return StatusCode::NOT_FOUND.into_response();
        };
        if let Some(monitored) = named.get("monitored") {
            show["monitored"] = monitored.clone();
        }
        for season in named["seasons"].as_array().into_iter().flatten() {
            if let Some(kept) = show["seasons"]
                .as_array_mut()
                .into_iter()
                .flatten()
                .find(|kept| kept["seasonNumber"] == season["seasonNumber"])
            {
                kept["monitored"] = season["monitored"].clone();
            }
        }
    }
    json(StatusCode::ACCEPTED, &json!({}))
}

/// Dark's Episodes in the Season `season_number` once the refresh after adding the Show
/// `show_id` finished, and none before.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn episodes(fake: &Fake, show_id: &str, season_number: &str) -> Response {
    let refreshed = show_id
        .parse()
        .is_ok_and(|id| lock(&fake.refreshed).contains(&id));
    let all: Vec<Value> = if refreshed {
        serde_json::from_slice(&read_fixture("episode.json")).unwrap()
    } else {
        Vec::new()
    };
    let season: Vec<_> = all
        .into_iter()
        .filter(|episode| episode["seasonNumber"].as_u64() == season_number.parse().ok())
        .collect();
    json(StatusCode::OK, &Value::Array(season))
}

fn monitor_episodes(fake: &Fake, monitor: &Value) -> Response {
    if monitor["monitored"] == true {
        lock(&fake.monitored_episodes).extend(
            monitor["episodeIds"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_i64),
        );
    }
    json(StatusCode::ACCEPTED, &json!([]))
}

fn command(fake: &Fake, command: &Value) -> Response {
    if command["name"] != "SeasonSearch" {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let shows = lock(&fake.shows);
    let Some(show) = shows.iter().find(|show| show["id"] == command["seriesId"]) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let searched = (
        show["tvdbId"].as_i64().unwrap_or_default(),
        command["seasonNumber"].as_u64().unwrap_or_default(),
    );
    lock(&fake.searches).push(searched);
    fixture(StatusCode::CREATED, "command.json")
}

fn seasons(show: &Value) -> &[Value] {
    show["seasons"].as_array().map_or(&[], Vec::as_slice)
}

/// The recorded response `file`, with `status`.
fn fixture(status: StatusCode, file: &str) -> Response {
    let body = read_fixture(file);
    (status, [(header::CONTENT_TYPE, "application/json")], body).into_response()
}

fn json(status: StatusCode, body: &Value) -> Response {
    let body = body.to_string();
    (status, [(header::CONTENT_TYPE, "application/json")], body).into_response()
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn read_fixture(file: &str) -> Vec<u8> {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../plugins/sonarr/fixtures");
    std::fs::read(fixtures.join(file)).unwrap()
}
