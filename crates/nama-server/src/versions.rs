//! Direct play: a Version's file streamed as it is, over plain HTTP at `/media/versions/<id>`
//! (ADR 0007: media bytes are not RPC), to anyone with a session. A browser's `<video src>`
//! presents that session in its cookie (ADR 0014).
//!
//! Range requests let a `<video>` seek. Only Versions, files a scan stored for a Library and
//! Matched, are served, by id, so a client can never name a path.

use axum::{
    Router,
    extract::{Path, Request, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use tower_http::services::ServeFile;

use crate::contents::Contents;

/// Serves `GET /media/versions/<id>` with the file of the Version `id` in `contents`, and 404 if
/// there is none. Whether the caller has a session is up to the layers the router is put behind.
pub(crate) fn router(contents: Contents) -> Router {
    Router::new()
        .route("/media/versions/{id}", get(serve))
        .with_state(contents)
}

async fn serve(
    State(contents): State<Contents>,
    Path(id): Path<i64>,
    request: Request,
) -> Response {
    let path = match contents.version_file(id).await {
        Ok(Some((path, _))) => path,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            tracing::error!("could not look up the Version {id}: {err}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    // `ServeFile` answers `Range` (206 and `Content-Range`), sets `Accept-Ranges`, `ETag` and
    // `Last-Modified`, takes the `Content-Type` from the extension, and is 404 for a file gone
    // since the last scan.
    match ServeFile::new(&path).try_call(request).await {
        Ok(response) => response.into_response(),
        Err(err) => {
            tracing::error!("could not serve {path}: {err}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
