//! Direct Play: serving a Version's file as it is.

use std::{io, net::IpAddr, sync::Arc};

use axum::{
    Router,
    body::Body,
    extract::{Path, Request, State},
    http::{
        StatusCode,
        header::{ETAG, IF_RANGE, LAST_MODIFIED, RANGE},
    },
    middleware::Next,
    response::{IntoResponse, Response},
    routing::get,
};
use futures_util::StreamExt as _;
use nama_forwarded::RequestSource;
use tower_http::services::ServeFile;

use super::{WholeFileReads, limits::on_local_network, running::RunningPlans, subtitles};
use crate::{
    db::Database,
    ffmpeg::Ffmpeg,
    library::Contents,
    signed_urls::Viewer,
    users::{Caller, library_access, maximum_age, remote_cap},
};

pub(super) fn direct_play_url(version: i64) -> String {
    format!("/media/versions/{version}")
}

/// Serves `GET /media/versions/<id>`, and `GET /media/downloads/<id>` as [`download`] says, with
/// the file of the Version `id` in `db`, and 404 if there is none, marking the caller's Direct
/// Plays of it in `running` asked for as the request arrives and with each part of the file sent,
/// and the subtitles plans offer, found in `db`, read with `ffmpeg`, a Version's text ones in a
/// background pass on `reads`, and cached in `cache`.
/// A caller off the local network whose User has a cap for remote playback gets 403 unless that
/// User has a plan on their Device, made off the local network, that Direct Plays the Version.
/// Whether the caller has a session is up to the layers the router is put behind.
pub(crate) fn router(
    running: RunningPlans,
    db: &Arc<Database>,
    ffmpeg: Arc<Ffmpeg>,
    cache: subtitles::Cache,
    reads: WholeFileReads,
) -> Router {
    let subtitles = subtitles::router(subtitles::Subtitles::new(db, ffmpeg, cache, reads));
    Router::new()
        .route("/media/versions/{id}", get(serve))
        .route("/media/downloads/{id}", get(download))
        .with_state((Contents::new(Arc::clone(db)), running, Arc::clone(db)))
        .merge(subtitles)
}

async fn serve(
    State((contents, running, db)): State<(Contents, RunningPlans, Arc<Database>)>,
    Path(id): Path<i64>,
    request: Request,
) -> Response {
    let viewer = viewer_of(&request);
    let user = viewer.map(|viewer| viewer.user);
    let client = (request.extensions().get::<RequestSource>()).map(|source| source.client_ip);
    let planned_under_remote_cap =
        viewer.is_some_and(|viewer| running.ask_direct_plays(id, viewer));
    let path = match version_file(&contents, id).await {
        Ok(path) => path,
        Err(status) => return status.into_response(),
    };
    // A plan made off the local network applied the cap where it could, so one running lets the
    // file through, even past the cap for a picture of unknown size (ADR 0031, #2134). One made
    // on the local network never applied the cap (#2130).
    if !planned_under_remote_cap {
        match capped_remotely(&db, user, client).await {
            Ok(false) => {}
            Ok(true) => return StatusCode::FORBIDDEN.into_response(),
            Err(err) => {
                tracing::error!("could not look up a User's cap for remote playback: {err}");
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        }
    }
    // Taken once the Version is known to be there, so asking for made-up ids keeps nothing.
    let (deleted, stopped) = (running.served(id), running.stopped(id, viewer));
    // `ServeFile` answers `Range` (206 and `Content-Range`), sets `Accept-Ranges`, `ETag` and
    // `Last-Modified`, and takes the `Content-Type` from the extension. It ignores `If-Range`,
    // which `serve_file` answers, and fails with `NotFound` for a file gone since the last scan.
    match serve_file(&path, request).await {
        Ok(response) => {
            let (parts, body) = response.into_parts();
            let sent = running.ask_direct_plays_while_sending(id, viewer, body.into_data_stream());
            let cut_off = async move {
                tokio::select! {
                    () = deleted.cancelled() => {}
                    () = stopped.cancelled() => {}
                }
            };
            let sent = sent.take_until(cut_off);
            Response::from_parts(parts, Body::from_stream(sent))
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            tracing::error!("could not serve {path}: {err}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Answers 404 to a request under `/media/versions/<id>`, for the Version's file, subtitles,
/// fonts or Preview frames alike, from a caller who may not play the Version `id`, as
/// [`sees_version`] judges. Put in front of every router serving there.
pub(crate) async fn seen_versions_only(
    State(db): State<Arc<Database>>,
    request: Request,
    next: Next,
) -> Response {
    let version = (request.uri().path().strip_prefix("/media/versions/"))
        .and_then(|rest| rest.split('/').next())
        .and_then(|id| id.parse().ok());
    let user = viewer_of(&request).map(|viewer| viewer.user);
    if let (Some(version), Some(user)) = (version, user) {
        match sees_version(&db, user, version).await {
            Ok(true) => {}
            Ok(false) => return StatusCode::NOT_FOUND.into_response(),
            Err(err) => {
                tracing::error!("could not look up whether a User may play a Version: {err}");
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        }
    }
    next.run(request).await
}

/// Who a media request is for: its session's User and Device, or else those its signed URL was
/// signed for.
pub(super) fn viewer_of(request: &Request) -> Option<Viewer> {
    let extensions = request.extensions();
    let session = extensions.get::<Caller>().map(viewer_of_caller);
    session.or_else(|| extensions.get::<Viewer>().copied())
}

/// The Viewer a request with `caller`'s session acts as.
pub(crate) fn viewer_of_caller(caller: &Caller) -> Viewer {
    Viewer {
        user: caller.user.id,
        device: caller.device.id,
    }
}

/// Whether the User `user` sees the Library of the Version `version`, and their Maximum Age lets
/// through each title it holds, as they must to play it. One there is no such Version of is for
/// the caller to refuse.
pub(super) async fn sees_version(
    db: &Arc<Database>,
    user: i64,
    version: i64,
) -> Result<bool, sqlx::Error> {
    let contents = Contents::new(Arc::clone(db));
    let Some(library) = contents.library_of_version(version).await? else {
        return Ok(true);
    };
    if !library_access(db, user).await?.sees(library) {
        return Ok(false);
    }
    let max_age = maximum_age(db, user).await?;
    let ratings = contents.version_content_ratings(version).await?;
    Ok(ratings
        .iter()
        .all(|rating| max_age.allows(rating.as_deref())))
}

/// Serves the Version `id`'s file to keep on the caller's Device as [`serve`] does, but only while
/// the caller has a plan on that Device Direct Playing it, so it plays offline as it would now;
/// 409 otherwise (#1422). Whether the caller may download is up to the layers the router is put
/// behind.
async fn download(
    State((contents, running, db)): State<(Contents, RunningPlans, Arc<Database>)>,
    Path(id): Path<i64>,
    request: Request,
) -> Response {
    let viewer = request.extensions().get::<Caller>().map(viewer_of_caller);
    if viewer.is_some_and(|viewer| running.has_direct_play(id, viewer)) {
        return serve(State((contents, running, db)), Path(id), request).await;
    }
    match version_file(&contents, id).await {
        Ok(_) => (
            StatusCode::CONFLICT,
            "only a Version the Device has a Direct Play plan for downloads",
        )
            .into_response(),
        Err(status) => status.into_response(),
    }
}

/// Where the Version `id`'s file is, or the 404 or 500 to answer with instead.
async fn version_file(contents: &Contents, id: i64) -> Result<String, StatusCode> {
    match contents.version_file(id).await {
        Ok(Some((path, _))) => Ok(path),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(err) => {
            tracing::error!("could not look up the Version {id}: {err}");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Whether the User `user`, asking from `client`, is off the local network with a cap for remote
/// playback. An unknown address counts as off it.
async fn capped_remotely(
    db: &Database,
    user: Option<i64>,
    client: Option<IpAddr>,
) -> Result<bool, sqlx::Error> {
    match user {
        Some(user) if !client.is_some_and(on_local_network) => {
            Ok(remote_cap(db, user).await?.is_some())
        }
        _ => Ok(false),
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
