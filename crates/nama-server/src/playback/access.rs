//! Playback access: the one place that decides whether a request may have a Version's media or a
//! plan's files, and on what terms (`AGENTS.md`).
//!
//! A plan plays only for the Viewer it was planned for, and only while that Device acts as that
//! User (ADR 0105, ADR 0106). A Version's media goes only to a Viewer who sees its Library and
//! whose Maximum Age lets its titles through, and off the local network a capped User gets its file
//! only through a Direct Play planned under their cap (ADR 0031). A request with no Viewer gets
//! nothing.

use std::{net::IpAddr, sync::Arc};

use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use futures_util::{Stream, StreamExt as _};
use nama_forwarded::RequestSource;
use tokio_util::sync::CancellationToken;

use super::{
    limits::on_local_network,
    running::{PlanFiles, RunningPlans},
};
use crate::{
    db::{Database, now},
    library::Contents,
    signed_urls::{SignedGrant, Viewer},
    users::{Caller, acts_as, reach, remote_cap},
};

/// Judges who may fetch playback media, against the plans running and the Users and Library in
/// its database.
#[derive(Clone)]
pub(crate) struct PlaybackAccess {
    running: RunningPlans,
    db: Arc<Database>,
}

/// Who asks for media: a session's Viewer, and a signed playback grant whose signature the
/// Server checked. A request carrying both acts as its session.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Asker {
    pub(super) session: Option<Viewer>,
    pub(super) grant: Option<SignedGrant>,
}

/// What a request asks for.
#[derive(Debug, Clone, Copy)]
pub(super) enum Asked {
    /// The Version's file, to play.
    File(i64),
    /// The Version's subtitles, fonts or Preview frames.
    Media(i64),
    /// The plan's HLS files.
    Plan(u64),
    /// The Version's file, to keep on the Device.
    Download(i64),
}

/// What a request may have.
pub(super) enum Lease {
    /// A Version's subtitles, fonts or Preview frames, which go as they are.
    Media,
    File(FileLease),
    Plan(PlanFiles),
}

/// A Version's file, leased to a Viewer: kept alive and cut off as [`Self::send`] says.
#[derive(Clone)]
pub(super) struct FileLease {
    /// Where the file is.
    pub(super) path: String,
    version: i64,
    viewer: Viewer,
    running: RunningPlans,
    deleted: CancellationToken,
    stopped: CancellationToken,
}

impl PlaybackAccess {
    /// Judges against `running` and what `db` holds.
    pub(crate) const fn new(running: RunningPlans, db: Arc<Database>) -> Self {
        Self { running, db }
    }

    /// What `who`, asking from `client`, may have of `asked`, or the status refusing it: 401 for no
    /// Viewer, 404 for no Version or plan of theirs, 403 past their cap for remote playback,
    /// 409 for a download with no Direct Play of it, and 500 when a lookup fails.
    pub(super) async fn judge(
        &self,
        who: Asker,
        asked: Asked,
        client: Option<IpAddr>,
    ) -> Result<Lease, StatusCode> {
        let viewer = self.viewer_of(who).await?;
        match asked {
            Asked::Media(version) => self.seen_file(version, viewer).await.map(|_| Lease::Media),
            Asked::Download(version) if !self.running.has_direct_play(version, viewer) => {
                self.version_file(version).await?;
                Err(StatusCode::CONFLICT)
            }
            Asked::File(version) | Asked::Download(version) => {
                (self.lease_file(version, viewer, client).await).map(Lease::File)
            }
            Asked::Plan(id) => (self.running.ask_files(id, viewer))
                .map(Lease::Plan)
                .ok_or(StatusCode::NOT_FOUND),
        }
    }

    /// Whom `who` acts as: their session's Viewer, or else their grant's while it stands.
    async fn viewer_of(&self, who: Asker) -> Result<Viewer, StatusCode> {
        if let Some(session) = who.session {
            return Ok(session);
        }
        let Some(SignedGrant { grant, expires }) = who.grant else {
            return Err(StatusCode::UNAUTHORIZED);
        };
        let viewer = grant.viewer;
        // A player fetches under one URL for as long as it plays (ADR 0105). Plan id 0 runs none.
        let live = now() < expires || self.running.runs_for(grant.plan, viewer.device);
        if !live {
            return Err(StatusCode::UNAUTHORIZED);
        }
        // Asked even while its plan runs, since a Device that switched away can't keep playing a
        // plan that belongs to its User (ADR 0106).
        match acts_as(&self.db, viewer.device, viewer.user).await {
            Ok(true) => Ok(viewer),
            Ok(false) => Err(StatusCode::UNAUTHORIZED),
            Err(err) => Err(failed("whom a Device acts as", &err)),
        }
    }

    /// The Version `version`'s file leased to `viewer`, asking from `client`, marking their Direct
    /// Plays of it asked for.
    async fn lease_file(
        &self,
        version: i64,
        viewer: Viewer,
        client: Option<IpAddr>,
    ) -> Result<FileLease, StatusCode> {
        let path = self.seen_file(version, viewer).await?;
        // A plan made off the local network applied the cap where it could, so one running lets
        // the file through, even past the cap for a picture of unknown size (ADR 0031, #2134). One
        // made on the local network never applied the cap (#2130).
        if !self.running.ask_direct_plays(version, viewer)
            && self.capped_remotely(viewer.user, client).await?
        {
            return Err(StatusCode::FORBIDDEN);
        }
        // Taken once the Version is known to be there, so asking for made-up ids keeps nothing.
        Ok(FileLease {
            path,
            version,
            viewer,
            running: self.running.clone(),
            deleted: self.running.served(version),
            stopped: self.running.stopped(version, viewer),
        })
    }

    /// Where the Version `version`'s file is, if `viewer` may play it, as [`sees_version`] says.
    async fn seen_file(&self, version: i64, viewer: Viewer) -> Result<String, StatusCode> {
        let path = self.version_file(version).await?;
        match sees_version(&self.db, viewer.user, version).await {
            Ok(true) => Ok(path),
            Ok(false) => Err(StatusCode::NOT_FOUND),
            Err(err) => Err(failed("whether a User may play a Version", &err)),
        }
    }

    /// Where the Version `version`'s file is; 404 if there is no such Version.
    async fn version_file(&self, version: i64) -> Result<String, StatusCode> {
        match Contents::new(Arc::clone(&self.db))
            .version_file(version)
            .await
        {
            Ok(Some((path, _))) => Ok(path),
            Ok(None) => Err(StatusCode::NOT_FOUND),
            Err(err) => Err(failed("a Version", &err)),
        }
    }

    /// Whether the User `user`, asking from `client`, is off the local network with a cap for
    /// remote playback. An unknown address counts as off it.
    async fn capped_remotely(&self, user: i64, client: Option<IpAddr>) -> Result<bool, StatusCode> {
        if client.is_some_and(on_local_network) {
            return Ok(false);
        }
        match remote_cap(&self.db, user).await {
            Ok(cap) => Ok(cap.is_some()),
            Err(err) => Err(failed("a User's cap for remote playback", &err)),
        }
    }
}

impl FileLease {
    /// `file`, the parts of the file as they're sent, each marking the Viewer's Direct Plays of the
    /// Version asked for, so they're kept while it's sent, and ending once an Admin stops their
    /// play of it or deletes it.
    pub(super) fn send<S: Stream + Send + 'static>(
        self,
        file: S,
    ) -> impl Stream<Item = S::Item> + Send {
        let Self {
            version,
            viewer,
            running,
            deleted,
            stopped,
            ..
        } = self;
        let cut_off = async move {
            tokio::select! {
                () = deleted.cancelled() => {}
                () = stopped.cancelled() => {}
            }
        };
        (running.ask_direct_plays_while_sending(version, viewer, file)).take_until(cut_off)
    }
}

impl Asked {
    /// What a request for `path` asks for, if it is under a Version's or a plan's media path or a
    /// Version's download; 400 for such a path naming no id, as the routes' own ids answer.
    fn of(path: &str) -> Option<Result<Self, StatusCode>> {
        let mut parts = path.strip_prefix("/media/")?.splitn(3, '/');
        let (under, id, rest) = (parts.next()?, parts.next()?, parts.next());
        let asked = match (under, rest) {
            ("versions", None) => id.parse().ok().map(Self::File),
            ("versions", Some(_)) => id.parse().ok().map(Self::Media),
            ("plans", _) => id.parse().ok().map(Self::Plan),
            // Transcodes to download, under `downloads/` too, check who asked themselves.
            ("downloads", None) => id.parse().ok().map(Self::Download),
            _ => return None,
        };
        Some(asked.ok_or(StatusCode::BAD_REQUEST))
    }
}

/// Answers a request under a Version's or a plan's media path, or for a Version's download, with
/// the status refusing it, or passes it on with its [`FileLease`] or [`PlanFiles`]. Put in front of
/// every router serving there; it lets any other request through.
pub(crate) async fn require_lease(
    State(access): State<PlaybackAccess>,
    mut request: Request,
    next: Next,
) -> Response {
    let Some(asked) = Asked::of(request.uri().path()) else {
        return next.run(request).await;
    };
    let extensions = request.extensions();
    let who = Asker {
        session: extensions.get::<Caller>().map(viewer_of_caller),
        grant: extensions.get::<SignedGrant>().copied(),
    };
    let client = (extensions.get::<RequestSource>()).map(|source| source.client_ip);
    let judged = match asked {
        Ok(asked) => access.judge(who, asked, client).await,
        Err(refused) => Err(refused),
    };
    match judged {
        Ok(Lease::Media) => {}
        Ok(Lease::File(file)) => {
            request.extensions_mut().insert(file);
        }
        Ok(Lease::Plan(files)) => {
            request.extensions_mut().insert(files);
        }
        Err(StatusCode::CONFLICT) => {
            let why = "only a Version the Device has a Direct Play plan for downloads";
            return (StatusCode::CONFLICT, why).into_response();
        }
        Err(status) => return status.into_response(),
    }
    next.run(request).await
}

/// The Viewer a request with `caller`'s session acts as.
pub(crate) const fn viewer_of_caller(caller: &Caller) -> Viewer {
    Viewer {
        user: caller.user.id,
        device: caller.device.id,
    }
}

/// Whether each title the Version `version` holds reaches the User `user`, as it must for them to
/// play it. One there is no such Version of is for
/// the caller to refuse.
pub(super) async fn sees_version(
    db: &Arc<Database>,
    user: i64,
    version: i64,
) -> Result<bool, sqlx::Error> {
    let reach = reach(db, user).await?;
    Contents::new(Arc::clone(db))
        .lets_play(version, &reach)
        .await
}

/// The 500 for a failed lookup of `what`, logged.
fn failed(what: &str, err: &sqlx::Error) -> StatusCode {
    tracing::error!("could not look up {what}: {err}");
    StatusCode::INTERNAL_SERVER_ERROR
}

#[cfg(test)]
#[path = "access_tests.rs"]
mod tests;
