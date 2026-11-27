//! Direct Play: serving a Version's file as it is.

use std::{io, sync::Arc};

use axum::{
    Extension, Router,
    body::Body,
    extract::Request,
    http::{
        StatusCode,
        header::{ETAG, IF_RANGE, LAST_MODIFIED, RANGE},
    },
    response::{IntoResponse, Response},
    routing::get,
};
use tower_http::services::ServeFile;

use super::{WholeFileReads, access::FileLease, subtitles};
use crate::{db::Database, ffmpeg::Ffmpeg};

pub(super) fn direct_play_url(version: i64) -> String {
    format!("/media/versions/{version}")
}

/// Serves `GET /media/versions/<id>`, and `GET /media/downloads/<id>` to keep, with the file of the
/// Version `id` as leased by the layers the router is put behind, and the subtitles plans offer,
/// found in `db`, read with `ffmpeg`, those in a Version's file in a background pass on `reads`,
/// and cached in `cache`.
pub(crate) fn router(
    db: &Arc<Database>,
    ffmpeg: Arc<Ffmpeg>,
    cache: subtitles::Cache,
    reads: WholeFileReads,
) -> Router {
    let subtitles = subtitles::router(subtitles::Subtitles::new(db, ffmpeg, cache, reads));
    Router::new()
        .route("/media/versions/{id}", get(serve))
        .route("/media/downloads/{id}", get(serve))
        .merge(subtitles)
}

async fn serve(Extension(lease): Extension<FileLease>, request: Request) -> Response {
    let path = lease.path.clone();
    // `ServeFile` answers `Range` (206 and `Content-Range`), sets `Accept-Ranges`, `ETag` and
    // `Last-Modified`, and takes the `Content-Type` from the extension. It ignores `If-Range`,
    // which `serve_file` answers, and fails with `NotFound` for a file gone since the last scan.
    match serve_file(&path, request).await {
        Ok(response) => {
            let (parts, body) = response.into_parts();
            let sent = lease.send(body.into_data_stream());
            Response::from_parts(parts, Body::from_stream(sent))
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            tracing::error!("could not serve {path}: {err}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Serves the file at `path` as `request` asks, but whole, with a 200, when its `If-Range` names
/// the file as it was before it was replaced (an upgrade, a re-encode), so a `<video>` resuming it
/// isn't sent the new file's bytes to splice into the old one's (RFC 9110 §13.1.5).
pub(super) async fn serve_file(
    path: impl AsRef<std::path::Path>,
    request: Request,
) -> io::Result<Response> {
    let path = path.as_ref();
    let (mut parts, body) = request.into_parts();
    let if_range = parts.headers.remove(IF_RANGE);
    let first = Request::from_parts(parts.clone(), body);
    let response = ServeFile::new(path).try_call(first).await?.into_response();
    let Some(if_range) = if_range.filter(|_| parts.headers.contains_key(RANGE)) else {
        return Ok(response);
    };
    // Checked against the reply's own validators, so a file replaced in between can't slip by.
    // Exact bytes are the strong comparison `If-Range` asks for: a weak `W/` tag never matches.
    // ponytail: a date matches even if the file changed again within that second, which only the
    // ETag tells apart; also check the date is a second older than the reply once a client resumes
    // by date alone.
    let names_the_file = [ETAG, LAST_MODIFIED]
        .iter()
        .any(|validator| response.headers().get(validator) == Some(&if_range));
    if names_the_file {
        return Ok(response);
    }
    parts.headers.remove(RANGE);
    let whole = Request::from_parts(parts, Body::empty());
    Ok(ServeFile::new(path).try_call(whole).await?.into_response())
}
