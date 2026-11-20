//! Direct Play: serving a Version's file as it is.

use std::sync::Arc;

use axum::{
    Router,
    body::Body,
    extract::{Path, Request, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use futures_util::StreamExt as _;
use tower_http::services::ServeFile;

use super::{Probing, StoredTracks, running::RunningPlans, subtitles};
use crate::{db::Database, ffmpeg::Ffmpeg, library::Contents, users::Caller};

pub(super) fn direct_play_url(version: i64) -> String {
    format!("/media/versions/{version}")
}

/// Serves `GET /media/versions/<id>` with the file of the Version `id` in `contents`, and 404 if
/// there is none, marking the caller's Direct Plays of it in `running` asked for as the request
/// arrives and with each part of the file sent, and the subtitles plans offer, found in `db`, read
/// with `ffmpeg`, a Version's text ones in a background pass on `probing`, and cached in its
/// subtitle cache. Whether the caller has a session is up to the layers the router is put behind.
pub(crate) fn router(
    contents: Contents,
    running: RunningPlans,
    db: &Arc<Database>,
    ffmpeg: Arc<Ffmpeg>,
    probing: Probing,
) -> Router {
    let subtitles = subtitles::router(subtitles::Subtitles {
        contents: contents.clone(),
        tracks: StoredTracks::new(Arc::clone(db)),
        ffmpeg,
        cache: probing.subtitle_cache(),
        probing,
    });
    Router::new()
        .route("/media/versions/{id}", get(serve))
        .with_state((contents, running))
        .merge(subtitles)
}

async fn serve(
    State((contents, running)): State<(Contents, RunningPlans)>,
    Path(id): Path<i64>,
    request: Request,
) -> Response {
    let device = (request.extensions().get::<Caller>()).map(|caller| caller.device.id);
    if let Some(device) = device {
        running.ask_direct_plays(id, device);
    }
    let path = match contents.version_file(id).await {
        Ok(Some((path, _))) => path,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            tracing::error!("could not look up the Version {id}: {err}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    // Taken once the Version is known to be there, so asking for made-up ids keeps nothing.
    let (deleted, stopped) = (running.served(id), running.stopped(id, device));
    // `ServeFile` answers `Range` (206 and `Content-Range`), sets `Accept-Ranges`, `ETag` and
    // `Last-Modified`, takes the `Content-Type` from the extension, and is 404 for a file gone
    // since the last scan.
    match ServeFile::new(&path).try_call(request).await {
        Ok(response) => {
            let (parts, body) = response.into_response().into_parts();
            let sent = running.ask_direct_plays_while_sending(id, device, body.into_data_stream());
            let cut_off = async move {
                tokio::select! {
                    () = deleted.cancelled() => {}
                    () = stopped.cancelled() => {}
                }
            };
            let sent = sent.take_until(cut_off);
            Response::from_parts(parts, Body::from_stream(sent))
        }
        Err(err) => {
            tracing::error!("could not serve {path}: {err}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
