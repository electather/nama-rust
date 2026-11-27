use std::sync::Arc;

use axum::{
    Extension, Router,
    extract::Path as UrlPath,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use tower_http::compression::CompressionLayer;

use super::{INIT, PLAYLIST, RunningPlans, run::Running};
use crate::signed_urls::Viewer;

/// The HLS files of a plan, leased to its Viewer.
#[derive(Clone)]
pub(in crate::playback) struct PlanFiles(Arc<Running>);

impl RunningPlans {
    /// The HLS files of the plan `id`, if it runs any for `viewer`, marked asked for as
    /// [`Self::ask`] says.
    pub(in crate::playback) fn ask_files(&self, id: u64, viewer: Viewer) -> Option<PlanFiles> {
        self.ask(id, viewer)?.hls.clone().map(PlanFiles)
    }
}

/// Serves `GET /media/plans/<id>/<file>`: the playlist and segments of the plan `id`, as leased by
/// the layers the router is put behind. It compresses the playlist with brotli or gzip when the
/// request accepts one.
pub(crate) fn router() -> Router {
    // A playlist grows a line per segment. Only its route is layered: MP4 is compressed already,
    // and the layer answers 406 to a request refusing every encoding it offers, even uncompressed.
    let playlist = get(serve_playlist).layer(CompressionLayer::new());
    Router::new()
        .route(&format!("/media/plans/{{id}}/{PLAYLIST}"), playlist)
        .route("/media/plans/{id}/{file}", get(serve_file))
}

async fn serve_playlist(Extension(PlanFiles(plan)): Extension<PlanFiles>) -> Response {
    let content_type = [(header::CONTENT_TYPE, "application/vnd.apple.mpegurl")];
    (content_type, plan.playlist()).into_response()
}

async fn serve_file(
    Extension(PlanFiles(plan)): Extension<PlanFiles>,
    UrlPath((id, file)): UrlPath<(u64, String)>,
) -> Response {
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
