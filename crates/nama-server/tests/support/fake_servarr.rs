//! What the fake Radarr and the fake Sonarr share, as the two share Servarr's v3 API: serving on a
//! local port, going down and coming back, the API-key guard, and the quality profile, root folder
//! and queue routes. Each fake adds its own routes and state.

use std::{
    collections::HashMap,
    net::SocketAddr,
    path::PathBuf,
    sync::{
        Arc, Mutex, MutexGuard, PoisonError,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

use axum::{
    Router,
    extract::{Query, Request, State},
    http::{HeaderMap, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
};
use connectrpc::client::HttpClient;
use nama_proto::nama::v1::{
    EnablePluginRequest, PluginServiceClient, SettingValue, UpdatePluginSettingsRequest,
    setting_value,
};
use serde_json::Value;
use tokio::{net::TcpListener, task::JoinHandle};

use crate::server::TestServer;

/// Which Servarr a [`FakeServarr`] stands in for.
#[derive(Clone, Copy)]
pub(crate) struct Servarr {
    /// The API key it accepts.
    pub(crate) api_key: &'static str,
    /// The Plugin's folder of recorded responses, from the workspace root.
    pub(crate) fixtures: &'static str,
    /// The query parameter its queue needs set to `true`, such as `includeMovie`.
    pub(crate) queue_include: &'static str,
}

impl Servarr {
    /// The recorded response `file`, with `status`.
    pub(crate) fn fixture(self, status: StatusCode, file: &str) -> Response {
        let body = self.read_fixture(file);
        (status, [(header::CONTENT_TYPE, "application/json")], body).into_response()
    }

    /// The bytes of the recorded response `file`.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    pub(crate) fn read_fixture(self, file: &str) -> Vec<u8> {
        let folder = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(self.fixtures);
        std::fs::read(folder.join(file)).unwrap()
    }
}

/// A fake Servarr serving on a local port while up, until dropped.
pub(crate) struct FakeServarr {
    addr: SocketAddr,
    state: Arc<Fake>,
    /// The routes its own fake adds, answering what the shared ones don't.
    routes: Router,
    server: Option<JoinHandle<()>>,
}

struct Fake {
    servarr: Servarr,
    /// Whether it refuses its API key too, as after an Admin made it a new one.
    refuses_key: AtomicBool,
    /// The ids of the quality profiles removed from it, as when an Admin deletes one.
    removed_profiles: Mutex<Vec<i64>>,
    /// The fixture its queue answers with.
    queue: Mutex<&'static str>,
    /// How many times its queue was asked for.
    queue_calls: AtomicUsize,
}

impl FakeServarr {
    /// Starts serving `servarr` on a free local port, answering with `routes` what the shared
    /// routes don't, once its API key checked out.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    pub(crate) async fn start(servarr: Servarr, routes: Router) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut fake = Self {
            addr: listener.local_addr().unwrap(),
            state: Arc::new(Fake {
                servarr,
                refuses_key: AtomicBool::default(),
                removed_profiles: Mutex::default(),
                queue: Mutex::new("queue-empty.json"),
                queue_calls: AtomicUsize::default(),
            }),
            routes,
            server: None,
        };
        fake.serve(listener);
        fake
    }

    /// Where it serves, as its URL setting names it.
    pub(crate) fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Stops answering, so connecting to it is refused, as for a Servarr that is down.
    pub(crate) fn go_down(&mut self) {
        if let Some(server) = self.server.take() {
            server.abort();
        }
    }

    /// Answers again on the same port.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    pub(crate) async fn come_back(&mut self) {
        let listener = TcpListener::bind(self.addr).await.unwrap();
        self.serve(listener);
    }

    /// Has `server`'s Admin set the Plugin `plugin` up against this fake, with the quality profile
    /// `profile` and the root folder `root_folder`, and enable it.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    pub(crate) async fn enable_as(
        &self,
        server: &TestServer,
        plugin: &str,
        profile: &str,
        root_folder: &str,
    ) {
        let plugins = PluginServiceClient::new(HttpClient::plaintext(), server.client_config());
        let settings = [
            ("url", self.url().as_str()),
            ("api_key", self.state.servarr.api_key),
            ("quality_profile", profile),
            ("root_folder", root_folder),
        ]
        .map(|(key, value)| {
            let value = SettingValue {
                value: Some(setting_value::Value::TextValue(value.to_owned())),
                ..Default::default()
            };
            (key.to_owned(), value)
        });
        plugins
            .update_plugin_settings(UpdatePluginSettingsRequest {
                id: plugin.to_owned(),
                settings: settings.into_iter().collect(),
                ..Default::default()
            })
            .await
            .unwrap();
        plugins
            .enable_plugin(EnablePluginRequest {
                id: plugin.to_owned(),
                ..Default::default()
            })
            .await
            .unwrap();
    }

    /// Refuses every API key from now on, as when an Admin makes it a new one.
    pub(crate) fn refuse_key(&self) {
        self.state.refuses_key.store(true, Ordering::Relaxed);
    }

    /// Stops offering the quality profile `id`, as when an Admin deletes it.
    pub(crate) fn remove_profile(&self, id: i64) {
        lock(&self.state.removed_profiles).push(id);
    }

    /// Makes its queue answer with the fixture `file`, such as `queue.json`.
    pub(crate) fn set_queue(&self, file: &'static str) {
        *lock(&self.state.queue) = file;
    }

    /// How many times its queue was asked for so far.
    pub(crate) fn queue_calls(&self) -> usize {
        self.state.queue_calls.load(Ordering::Relaxed)
    }

    fn serve(&mut self, listener: TcpListener) {
        let state = Arc::clone(&self.state);
        let app = Router::new()
            .route("/api/v3/qualityprofile", get(quality_profiles))
            .route("/api/v3/rootfolder", get(root_folders))
            .route("/api/v3/queue", get(queue))
            .fallback_service(self.routes.clone())
            .layer(middleware::from_fn_with_state(
                Arc::clone(&state),
                guard_key,
            ))
            .with_state(state);
        self.server = Some(tokio::spawn(async move {
            let _: Result<(), _> = axum::serve(listener, app).await;
        }));
    }
}

impl Drop for FakeServarr {
    /// Stops serving: dropping a task's handle alone would leave it running.
    fn drop(&mut self) {
        self.go_down();
    }
}

/// Locks `mutex`, even one a panicking test poisoned.
pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// `body` as an answer with `status`.
pub(crate) fn json(status: StatusCode, body: &Value) -> Response {
    let body = body.to_string();
    (status, [(header::CONTENT_TYPE, "application/json")], body).into_response()
}

async fn guard_key(
    State(fake): State<Arc<Fake>>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Response {
    if fake.refuses_key.load(Ordering::Relaxed)
        || headers
            .get("x-api-key")
            .is_none_or(|key| key != fake.servarr.api_key)
    {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    next.run(request).await
}

/// Its quality profiles, but those removed.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn quality_profiles(State(fake): State<Arc<Fake>>) -> Response {
    let removed = lock(&fake.removed_profiles).clone();
    let mut profiles: Value =
        serde_json::from_slice(&fake.servarr.read_fixture("qualityprofile.json")).unwrap();
    if let Value::Array(profiles) = &mut profiles {
        profiles.retain(|profile| {
            profile["id"]
                .as_i64()
                .is_none_or(|id| !removed.contains(&id))
        });
    }
    json(StatusCode::OK, &profiles)
}

async fn root_folders(State(fake): State<Arc<Fake>>) -> Response {
    fake.servarr.fixture(StatusCode::OK, "rootfolder.json")
}

async fn queue(
    State(fake): State<Arc<Fake>>,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    // Without the title, a queue entry names it only by the Servarr's own id.
    if query
        .get(fake.servarr.queue_include)
        .is_none_or(|include| include != "true")
    {
        return StatusCode::BAD_REQUEST.into_response();
    }
    fake.queue_calls.fetch_add(1, Ordering::Relaxed);
    let file = *lock(&fake.queue);
    fake.servarr.fixture(StatusCode::OK, file)
}
