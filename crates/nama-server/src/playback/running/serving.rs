use axum::{
    Router,
    extract::{Path as UrlPath, Request, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use tower_http::compression::CompressionLayer;

use super::{INIT, PLAYLIST, RunningPlans};
use crate::users::Caller;

const PLAYLIST_CONTENT_TYPE: &str = "application/vnd.apple.mpegurl";

/// Serves `GET /media/plans/<id>/<file>`: the playlist and segments of the plan `id`, to the
/// Device it belongs to, and 404 to any other. It compresses the playlist with brotli or gzip when
/// the request accepts one. Whether the caller has a session is up to the layers the router is put
/// behind.
pub(crate) fn router(plans: RunningPlans) -> Router {
    // A playlist grows a line per segment; MP4 is compressed already.
    let playlist_only = |_, _, headers: &header::HeaderMap, _: &_| {
        headers
            .get(header::CONTENT_TYPE)
            .is_some_and(|value| value == PLAYLIST_CONTENT_TYPE)
    };
    Router::new()
        .route("/media/plans/{id}/{file}", get(serve))
        .layer(CompressionLayer::new().compress_when(playlist_only))
        .with_state(plans)
}

async fn serve(
    State(plans): State<RunningPlans>,
    UrlPath((id, file)): UrlPath<(u64, String)>,
    request: Request,
) -> Response {
    let device = request
        .extensions()
        .get::<Caller>()
        .map(|caller| caller.device.id);
    let Some(plan) = device.and_then(|device| plans.ask(id, device)) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Some(plan) = &plan.hls else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if file == PLAYLIST {
        let content_type = [(header::CONTENT_TYPE, PLAYLIST_CONTENT_TYPE)];
        return (content_type, plan.playlist()).into_response();
    }
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
