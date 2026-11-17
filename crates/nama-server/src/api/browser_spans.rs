//! The path the web app posts its own Diagnostics to, which the Server forwards (ADR 0054).

use std::time::Instant;

use axum::{
    Extension, Router,
    body::Bytes,
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, StatusCode, header::CONTENT_TYPE},
    routing::post,
};

use crate::{diagnostics::BrowserSpans, users::Caller};

/// The largest body the web app may post: a batch of its spans is far smaller.
const MAX_BODY: usize = 256 * 1024;

/// Routes `/diagnostics/v1/traces` to `spans`, or nothing while the Server exports no Diagnostics,
/// so the path is then not found. `PREFIXES` in `../api.rs` lets only signed-in Devices post.
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
