use std::sync::Arc;

use axum::{
    Router,
    extract::{Path as UrlPath, Request, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use tower_http::compression::CompressionLayer;

use super::{INIT, PLAYLIST, RunningPlans, run::Running};
use crate::playback::direct_play::viewer_of;

/// Serves `GET /media/plans/<id>/<file>`: the playlist and segments of the plan `id`, to the
/// Device it belongs to, by its session or a URL signed for it, and 404 to any other. It compresses the playlist with brotli or gzip when
/// the request accepts one. Whether the caller has a session is up to the layers the router is put
/// behind.
pub(crate) fn router(plans: RunningPlans) -> Router {
    // A playlist grows a line per segment. Only its route is layered: MP4 is compressed already,
    // and the layer answers 406 to a request refusing every encoding it offers, even uncompressed.
    let playlist = get(serve_playlist).layer(CompressionLayer::new());
    Router::new()
        .route(&format!("/media/plans/{{id}}/{PLAYLIST}"), playlist)
        .route("/media/plans/{id}/{file}", get(serve_file))
        .with_state(plans)
}

async fn serve_playlist(
    State(plans): State<RunningPlans>,
    UrlPath(id): UrlPath<u64>,
    request: Request,
) -> Response {
    let Some(plan) = asked(&plans, id, &request) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let content_type = [(header::CONTENT_TYPE, "application/vnd.apple.mpegurl")];
    (content_type, plan.playlist()).into_response()
}

async fn serve_file(
    State(plans): State<RunningPlans>,
    UrlPath((id, file)): UrlPath<(u64, String)>,
    request: Request,
) -> Response {
    let Some(plan) = asked(&plans, id, &request) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let written = if file == INIT {
        plan.init().await
    } else if let Some(n) = plan.segment_number(&file) {
        plan.segment(n).await
    } else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match written {
        Ok(Some(body)) => ([(header::CONTENT_TYPE, "video/mp4")], body).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            tracing::error!(plan = id, "could not serve {file}: {err}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// The HLS files of the plan `id`, if it runs any for the caller's Device.
fn asked(plans: &RunningPlans, id: u64, request: &Request) -> Option<Arc<Running>> {
    let device = viewer_of(request)?.device;
    plans.ask(id, device)?.hls.clone()
}
