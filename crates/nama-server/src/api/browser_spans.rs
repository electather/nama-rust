//! The path the web app posts its own Diagnostics to, which the Server forwards (ADR 0054).

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

use crate::{diagnostics::use_ring_for_tls, users::Caller};

/// The largest body the web app may post: a batch of its spans is far smaller.
const MAX_BODY: usize = 256 * 1024;

/// Routes `/diagnostics/v1/traces` to `spans`, or nothing while the Server exports no Diagnostics,
/// so the path is then not found. `PREFIXES` in `access.rs` lets only signed-in Devices post.
pub(super) fn router(spans: Option<BrowserSpans>) -> Router {
    let Some(spans) = spans else {
        return Router::new();
    };
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

/// How many times one Device may post its spans in [`POSTS_WINDOW`]: its exporter sends a batch
/// every 5 seconds, or sooner once 512 spans wait, so this leaves room for a busy page's bursts.
const POSTS_PER_WINDOW: u32 = 60;

const POSTS_WINDOW: Duration = Duration::from_mins(1);

/// How long the collector has to take a post before it fails, so a stuck one holds no request.
const FORWARD_TIMEOUT: Duration = Duration::from_secs(10);

/// Forwards the web app's spans, as OTLP/HTTP, to where the Server exports its own, storing
/// nothing (ADR 0054), and limits how often each Device may post them.
#[derive(Clone)]
pub(super) struct BrowserSpans {
    traces_url: String,
    client: reqwest::Client,
    /// When each Device's current window started, and how many posts it has made in it.
    posts: Arc<Mutex<HashMap<i64, (Instant, u32)>>>,
}

impl BrowserSpans {
    /// Forwards to the OTLP `endpoint`'s traces path.
    pub(super) fn new(endpoint: &str) -> Self {
        // ponytail: ignores `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT` and `OTEL_EXPORTER_OTLP_HEADERS`,
        // which the Server's own exporter reads; read them once a collector needs a key.
        let traces_url = format!("{}/v1/traces", endpoint.trim_end_matches('/'));
        use_ring_for_tls();
        Self {
            traces_url,
            client: reqwest::Client::new(),
            posts: Arc::default(),
        }
    }

    /// Forwards `body`, of `content_type`, as it is, for the Device `device`, and answers with the
    /// collector's status: 429 instead if that Device posted too often lately, as of `now`, or 502
    /// if the collector can't be reached.
    async fn forward(
        &self,
        device: i64,
        content_type: Option<HeaderValue>,
        body: Bytes,
        now: Instant,
    ) -> StatusCode {
        if !self.admit(device, now) {
            return StatusCode::TOO_MANY_REQUESTS;
        }
        let mut request = self
            .client
            .post(&self.traces_url)
            .timeout(FORWARD_TIMEOUT)
            .body(body);
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
