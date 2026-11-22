//! The path the web app posts its own Diagnostics to, which the Server forwards to its admin's
//! collector and, while the admin shares Diagnostics, to Nama's developers (ADR 0054).

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, PoisonError},
    time::{Duration, Instant},
};

use axum::{
    Extension, Router,
    body::Bytes,
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, HeaderValue, StatusCode, header::CONTENT_TYPE},
    routing::post,
};
use nama_plugin_api::diagnostics_sharer::Signal;
use tokio_util::{sync::CancellationToken, task::TaskTracker};

use crate::{
    diagnostics::use_ring_for_tls,
    plugins::{ExtensionPoints, share_diagnostics},
    server_settings::Settings,
    users::Caller,
};

/// The largest body the web app may post: a batch of its spans is far smaller.
const MAX_BODY: usize = 256 * 1024;

/// How many times one Device may post its spans in [`POSTS_WINDOW`]: its exporter sends a batch
/// every 5 seconds, or sooner once 512 spans wait, so this leaves room for a busy page's bursts.
const POSTS_PER_WINDOW: u32 = 60;

const POSTS_WINDOW: Duration = Duration::from_mins(1);

/// How long the collector has to take a post before it fails, so a stuck one holds no request.
const FORWARD_TIMEOUT: Duration = Duration::from_secs(10);

/// The only encoding a `DiagnosticsSharer` takes, as the Hub forwards what it is handed as it.
const OTLP_PROTOBUF: &str = "application/x-protobuf";

/// Routes `/diagnostics/v1/traces` to `spans`. `PREFIXES` in `access.rs` lets only signed-in
/// Devices post.
pub(super) fn router(spans: BrowserSpans) -> Router {
    Router::new()
        .route("/diagnostics/v1/traces", post(forward))
        .layer(DefaultBodyLimit::max(MAX_BODY))
        .with_state(spans)
}

async fn forward(
    State(spans): State<BrowserSpans>,
    Extension(caller): Extension<Caller>,
    headers: HeaderMap,
    body: Bytes,
) -> StatusCode {
    let content_type = headers.get(CONTENT_TYPE).cloned();
    spans
        .forward(caller.device.id, content_type, body, Instant::now())
        .await
}

/// Forwards the web app's spans, as OTLP/HTTP, to where the Server exports and shares its own,
/// storing nothing (ADR 0054), and limits how often each Device may post them.
#[derive(Clone)]
pub(crate) struct BrowserSpans {
    /// The admin's collector's traces URL, if they set an endpoint.
    collector: Option<String>,
    client: reqwest::Client,
    /// Read on every post, since the admin may turn sharing Diagnostics on or off at any time.
    settings: Settings,
    points: ExtensionPoints,
    /// The shares in flight, which hold the Extension Points, and so the database, until they end.
    shares: TaskTracker,
    stop_sharing: CancellationToken,
    /// When each Device's current window started, and how many posts it has made in it.
    posts: Arc<Mutex<HashMap<i64, (Instant, u32)>>>,
}

impl BrowserSpans {
    /// Forwards to the OTLP `endpoint`'s traces path, if any, and to the sharer among `points`
    /// while `settings` share Diagnostics.
    pub(crate) fn new(endpoint: Option<&str>, settings: Settings, points: ExtensionPoints) -> Self {
        // ponytail: ignores `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT` and `OTEL_EXPORTER_OTLP_HEADERS`,
        // which the Server's own exporter reads; read them once a collector needs a key.
        let collector =
            endpoint.map(|endpoint| format!("{}/v1/traces", endpoint.trim_end_matches('/')));
        use_ring_for_tls();
        Self {
            collector,
            client: reqwest::Client::new(),
            settings,
            points,
            shares: TaskTracker::new(),
            stop_sharing: CancellationToken::new(),
            posts: Arc::default(),
        }
    }

    /// Forwards `body`, of `content_type`, as it is, for the Device `device`, and answers with the
    /// collector's status, or 200 without one: 404 instead while it goes nowhere, 429 if that
    /// Device posted too often lately, as of `now`, 415 if it isn't protobuf and would only be
    /// shared, or 502 if the collector can't be reached.
    async fn forward(
        &self,
        device: i64,
        content_type: Option<HeaderValue>,
        body: Bytes,
        now: Instant,
    ) -> StatusCode {
        if !self.exports() {
            return StatusCode::NOT_FOUND;
        }
        if !self.admit(device, now) {
            return StatusCode::TOO_MANY_REQUESTS;
        }
        if self.sharing() {
            if content_type.as_ref().is_some_and(|t| t == OTLP_PROTOBUF) {
                self.share(body.clone());
            } else if self.collector.is_none() {
                return StatusCode::UNSUPPORTED_MEDIA_TYPE;
            }
        }
        match &self.collector {
            Some(url) => self.post(url, content_type, body).await,
            None => StatusCode::OK,
        }
    }

    /// Whether the web app's spans go anywhere: to a collector, or shared while the admin shares
    /// Diagnostics.
    pub(super) fn exports(&self) -> bool {
        self.collector.is_some() || self.sharing()
    }

    fn sharing(&self) -> bool {
        self.settings.now().sharing.diagnostics
    }

    /// Hands `batch` to the sharer in the background: the Hub may take longer than the web's
    /// exporter waits, and a post it gave up on would never reach the collector.
    fn share(&self, batch: Bytes) {
        let points = self.points.clone();
        let shared = async move { share_diagnostics(&points, Signal::Traces, batch).await };
        self.shares
            .spawn(self.stop_sharing.clone().run_until_cancelled_owned(shared));
    }

    /// Cancels the shares in flight and waits until each has ended: before the Plugins shut down,
    /// as a share holds their Extension Points.
    pub(crate) async fn shutdown(&self) {
        self.stop_sharing.cancel();
        self.shares.close();
        self.shares.wait().await;
    }

    /// Posts `body`, of `content_type`, to the collector's traces `url`, answering with its status.
    async fn post(&self, url: &str, content_type: Option<HeaderValue>, body: Bytes) -> StatusCode {
        let mut request = self.client.post(url).timeout(FORWARD_TIMEOUT).body(body);
        if let Some(content_type) = content_type {
            request = request.header(CONTENT_TYPE, content_type);
        }
        match request.send().await {
            Ok(response) => response.status(),
            Err(err) => {
                tracing::warn!("could not forward the web app's Diagnostics: {err}");
                StatusCode::BAD_GATEWAY
            }
        }
    }

    /// Counts a post by `device` at `now`, and whether it is within the limit.
    fn admit(&self, device: i64, now: Instant) -> bool {
        // Every change leaves the counts consistent, so a panic elsewhere doesn't spoil them.
        let mut posts = self.posts.lock().unwrap_or_else(PoisonError::into_inner);
        posts.retain(|_, (started, _)| now.saturating_duration_since(*started) < POSTS_WINDOW);
        let (_, count) = posts.entry(device).or_insert((now, 0));
        *count = count.saturating_add(1);
        *count <= POSTS_PER_WINDOW
    }
}
