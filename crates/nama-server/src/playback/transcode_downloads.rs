//! Transcodes into one file for a Device to download and play offline (#1422, ADR 0107).
//!
//! A User with `DOWNLOAD` asks for one with `POST /media/downloads/<version>/transcodes/<limit>`,
//! the limit being one below Original as the API names it, such as `PLAYBACK_LIMIT_720P`. The 202
//! names in `Location` where its Device polls it, `/media/downloads/transcodes/<id>`, whose JSON
//! says how far it has got and, once it's made, where to fetch the file, Range-capable. Only that
//! Device, acting as the User who asked, is answered about it.
//!
//! The file is in the segment folder until a response has sent its last byte, for
//! [`KEEP_UNFETCHED_FOR`] once made if none does, and until its Version is gone ([`super::Leftovers`]).
//! One still being made [`KEEP_UNFETCHED_FOR`] after it was asked for is stopped, as its ffmpeg has
//! hung.
//! What they may take, made or being made, is the budget `nama.toml` sets, and a request that would
//! go past it is refused with 507. They're kept in memory: a restart forgets them, and its sweep
//! deletes their folder.

use std::{
    collections::{HashMap, HashSet},
    fmt, io,
    net::{IpAddr, Ipv4Addr},
    path::{Path as FilePath, PathBuf},
    sync::{
        Arc, Mutex, PoisonError, Weak,
        atomic::{AtomicI64, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use axum::{
    Router,
    body::Body,
    extract::{Path, Request, State},
    http::{
        StatusCode,
        header::{CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE, LOCATION},
        response::Parts,
    },
    response::{IntoResponse, Response},
    routing::{get, post},
};
use buffa::Enumeration as _;
use futures_util::StreamExt as _;
use nama_forwarded::RequestSource;
use serde_json::json;
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use tracing::Instrument as _;

use super::{Fallback, Method, NAMED_LIMITS, Plans, Playing, direct_play::serve_file};
use crate::{
    db, diagnostics,
    ffmpeg::{Ffmpeg, RunError, Transcode},
    library::Contents,
    server_settings::PlaybackSettings,
    users::Caller,
};

/// The folder in the segment folder the files are made in, named as a plan's folder is so the
/// sweep at startup deletes it with theirs (`sweep_leftovers`).
const FOLDER: &str = "nama-plan-downloads";

/// How long a made file waits for its Device to fetch it before it's deleted.
const KEEP_UNFETCHED_FOR: Duration = Duration::from_hours(24);

/// How often the files kept past [`KEEP_UNFETCHED_FOR`] are deleted while no Device asks about
/// any, which deletes them too.
const TIDY_EVERY: Duration = Duration::from_mins(10);

/// The most a file's audio takes, counted against the budget: stereo AAC, which ffmpeg encodes at
/// less.
const AUDIO_BITS_PER_SECOND: u64 = 256_000;

/// The files being made or waiting to be fetched, by id. Clones share them.
#[derive(Clone)]
pub(crate) struct TranscodeDownloads {
    inner: Arc<Inner>,
}

impl fmt::Debug for TranscodeDownloads {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TranscodeDownloads").finish_non_exhaustive()
    }
}

struct Inner {
    ffmpeg: Arc<Ffmpeg>,
    /// For the segment folder, read as each starts.
    settings: PlaybackSettings,
    contents: Contents,
    budget_bytes: u64,
    next_id: AtomicU64,
    downloads: Mutex<HashMap<u64, Download>>,
    /// The ffmpeg runs, deletes and [`tidy`], so shutting down can wait for them.
    tasks: TaskTracker,
    /// Cancelled when the Server shuts down.
    cancel: CancellationToken,
}

/// One file, being made or made.
struct Download {
    version: i64,
    /// Who asked, which alone is answered about it: a Device may act as several Users (ADR 0106),
    /// and another of them may not see the Version.
    asker: Asker,
    file: PathBuf,
    /// What it counts against the budget: the most it can take while it's made, then its size.
    bytes: u64,
    duration_ms: i64,
    /// How far into the Version ffmpeg has got.
    made_ms: Arc<AtomicI64>,
    stage: Stage,
    /// Cancelled to stop its ffmpeg, after which its run deletes the file.
    stop: CancellationToken,
}

#[derive(Debug, Clone, Copy)]
enum Stage {
    Making { since: Instant },
    Made { at: Instant },
    Failed { at: Instant },
}

/// Why a file couldn't be started.
#[derive(Debug, thiserror::Error)]
enum StartError {
    #[error("there is no such Version")]
    NoSuchVersion,
    #[error(
        "the downloads being made or waiting to be fetched take {} of the {} MiB budget, too \
         much for this one's {} MiB; fetch one or wait for one to expire",
        mib(*kept),
        mib(*budget),
        mib(*needed)
    )]
    OverBudget { kept: u64, budget: u64, needed: u64 },
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error("the downloads' folder couldn't be created: {0}")]
    Folder(#[from] io::Error),
}

impl TranscodeDownloads {
    /// Makes files with `ffmpeg` of the Versions `contents` finds, in the segment folder in
    /// `settings`, taking at most `budget_bytes` at once. Until shut down, deletes those expired
    /// in the background.
    pub(crate) fn new(
        ffmpeg: Arc<Ffmpeg>,
        settings: PlaybackSettings,
        contents: Contents,
        budget_bytes: u64,
    ) -> Self {
        let inner = Arc::new(Inner {
            ffmpeg,
            settings,
            contents,
            budget_bytes,
            next_id: AtomicU64::new(1),
            downloads: Mutex::default(),
            tasks: TaskTracker::new(),
            cancel: CancellationToken::new(),
        });
        (inner.tasks).spawn(tidy(Arc::downgrade(&inner), inner.cancel.clone()));
        Self { inner }
    }

    /// Starts making a file of the Version `version` for `asker` with `transcode`,
    /// `duration_ms` long, going on with `fallback` if it runs on hardware that fails, and returns
    /// its id.
    async fn start(
        &self,
        asker: Asker,
        version: i64,
        (transcode, duration_ms, fallback): (Transcode, i64, Option<Fallback>),
    ) -> Result<u64, StartError> {
        self.delete_expired().await;
        let Some((path, _)) = self.inner.contents.version_file(version).await? else {
            return Err(StartError::NoSuchVersion);
        };
        let folder = self.inner.settings.path().join(FOLDER);
        tokio::fs::create_dir_all(&folder).await?;
        let id = self.inner.next_id.fetch_add(1, Ordering::Relaxed);
        let file = folder.join(format!("{id}.mp4"));
        let download = Download {
            version,
            asker,
            file: file.clone(),
            bytes: most_bytes(&transcode, duration_ms),
            duration_ms,
            made_ms: Arc::default(),
            stage: Stage::Making {
                since: db::instant_now(),
            },
            stop: CancellationToken::new(),
        };
        let (made_ms, stop) = (Arc::clone(&download.made_ms), download.stop.clone());
        self.reserve(id, download)?;
        let downloads = self.clone();
        let span = tracing::info_span!("transcode download", version, download = id);
        let make = async move {
            let ffmpeg = &downloads.inner.ffmpeg;
            let mut made =
                (ffmpeg.transcode_to_file(&path, &transcode, &file, &made_ms, &stop)).await;
            if let (Err(err), Some(fallback)) = (&made, fallback) {
                tracing::warn!("could not Transcode on hardware, so going on in software: {err}");
                let software = &fallback.transcode;
                made = (ffmpeg.transcode_to_file(&path, software, &file, &made_ms, &stop)).await;
            }
            downloads.finish(id, &file, made).await;
        };
        self.inner.tasks.spawn(make.instrument(span));
        Ok(id)
    }

    /// Keeps `download` as `id` if it fits in the budget beside the others.
    fn reserve(&self, id: u64, download: Download) -> Result<(), StartError> {
        let mut downloads = lock(&self.inner.downloads);
        let kept: u64 = downloads.values().map(|other| other.bytes).sum();
        let budget = self.inner.budget_bytes;
        if kept + download.bytes > budget {
            let needed = download.bytes;
            return Err(StartError::OverBudget {
                kept,
                budget,
                needed,
            });
        }
        downloads.insert(id, download);
        Ok(())
    }

    /// Marks the file `id`, at `file`, made, counting its size against the budget from now, or
    /// failed as `made` says, deleting it unless it's made and still kept.
    async fn finish(&self, id: u64, file: &FilePath, made: Result<(), RunError>) {
        let size = match made {
            Ok(()) => (tokio::fs::metadata(file).await)
                .map(|metadata| metadata.len())
                .map_err(|err| err.to_string()),
            Err(err) => Err(err.to_string()),
        };
        let kept = {
            let mut downloads = lock(&self.inner.downloads);
            let at = db::instant_now();
            match (downloads.get_mut(&id), size) {
                (Some(download), Ok(bytes)) => {
                    (download.bytes, download.stage) = (bytes, Stage::Made { at });
                    true
                }
                (Some(download), Err(why)) => {
                    tracing::warn!("could not Transcode a Version into a file to download: {why}");
                    diagnostics::failed(&tracing::Span::current(), "transcode failed");
                    (download.bytes, download.stage) = (0, Stage::Failed { at });
                    false
                }
                // Stopped, as its Version is gone or the Server shuts down.
                (None, _) => false,
            }
        };
        if !kept {
            delete(file).await;
        }
    }

    /// The file `id` as `asker` may know of it, and where it is, if it's theirs and kept.
    async fn find(&self, id: u64, asker: Option<Asker>) -> Option<(Stage, Progress, PathBuf)> {
        self.delete_expired().await;
        let downloads = lock(&self.inner.downloads);
        let download = (downloads.get(&id)).filter(|download| Some(download.asker) == asker)?;
        let progress = Progress {
            made_ms: download.made_ms.load(Ordering::Relaxed),
            duration_ms: download.duration_ms,
            bytes: download.bytes,
        };
        Some((download.stage, progress, download.file.clone()))
    }

    /// Forgets the file `id`, which its Device has fetched, at once, and deletes it in the
    /// background.
    fn fetched(&self, id: u64) {
        if let Some(download) = lock(&self.inner.downloads).remove(&id) {
            (self.inner.tasks).spawn(async move { delete(&download.file).await });
        }
    }

    /// Stops making and deletes the files of every Version but `versions`.
    pub(crate) async fn keep_only(&self, versions: &HashSet<i64>) {
        (self.stop_and_delete(|download| !versions.contains(&download.version))).await;
    }

    /// Stops making every file and deletes them all, as a restart forgets them, waiting for ffmpeg
    /// to exit.
    pub(crate) async fn shutdown(&self) {
        self.stop_and_delete(|_| true).await;
        self.inner.cancel.cancel();
        self.inner.tasks.close();
        self.inner.tasks.wait().await;
    }

    /// Stops and deletes the files asked for, made or failed more than [`KEEP_UNFETCHED_FOR`] ago.
    async fn delete_expired(&self) {
        let now = db::instant_now();
        (self.stop_and_delete(|download| match download.stage {
            Stage::Making { since: at } | Stage::Made { at } | Stage::Failed { at } => {
                now.duration_since(at) >= KEEP_UNFETCHED_FOR
            }
        }))
        .await;
    }

    /// Forgets the files `picked` picks, stops making each, whose run then deletes its file, and
    /// deletes those made.
    async fn stop_and_delete(&self, picked: impl Fn(&Download) -> bool) {
        let gone: Vec<Download> = lock(&self.inner.downloads)
            .extract_if(|_, download| picked(download))
            .map(|(_, download)| download)
            .collect();
        for download in gone {
            download.stop.cancel();
            if !matches!(download.stage, Stage::Making { .. }) {
                delete(&download.file).await;
            }
        }
    }
}

#[cfg(test)]
impl TranscodeDownloads {
    /// Files of the Versions on `db` with no ffmpeg, in the default segment folder under `folder`,
    /// for a test that only asks what's deleted.
    pub(crate) fn for_tests(db: &Arc<db::Database>, folder: &FilePath) -> Self {
        let settings =
            crate::server_settings::Settings::new(Arc::clone(db), folder.join("transcodes"));
        let contents = Contents::new(Arc::clone(db));
        Self::new(Arc::default(), settings.playback(), contents, u64::MAX)
    }
}

/// The User and Device that asked for a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Asker {
    user: i64,
    device: i64,
}

impl From<&Caller> for Asker {
    fn from(caller: &Caller) -> Self {
        Self {
            user: caller.user.id,
            device: caller.device.id,
        }
    }
}

/// How far a file has got, as its Device is told.
struct Progress {
    made_ms: i64,
    duration_ms: i64,
    bytes: u64,
}

/// Every [`TIDY_EVERY`], deletes the files of `inner` expired, until it's dropped or `cancel` is
/// cancelled.
async fn tidy(inner: Weak<Inner>, cancel: CancellationToken) {
    let first = tokio::time::Instant::now() + TIDY_EVERY;
    let mut every = tokio::time::interval_at(first, TIDY_EVERY);
    while cancel.run_until_cancelled(every.tick()).await.is_some() {
        let Some(inner) = inner.upgrade() else { return };
        TranscodeDownloads { inner }.delete_expired().await;
    }
}

/// Deletes `file`, or logs why it couldn't.
async fn delete(file: &FilePath) {
    match tokio::fs::remove_file(file).await {
        Err(err) if err.kind() != io::ErrorKind::NotFound => {
            tracing::warn!("could not delete {}: {err}", file.display());
        }
        _ => {}
    }
}

/// The most a file of `transcode`, `duration_ms` long, takes: its rate, which a Transcode within a
/// Playback Limit keeps its video to, for that long.
fn most_bytes(transcode: &Transcode, duration_ms: i64) -> u64 {
    let video = (transcode.video.limit).map_or(0, |limit| u64::from(limit.bits_per_second));
    let ms = u64::try_from(duration_ms).unwrap_or_default();
    (video + AUDIO_BITS_PER_SECOND) * ms / 8 / 1000
}

/// `bytes` in whole MiB, rounded up.
const fn mib(bytes: u64) -> u64 {
    bytes.div_ceil(1024 * 1024)
}

/// Serves the files: asking for one, polling it and fetching it. Whether the caller may download
/// is up to the layers the router is put behind.
pub(crate) fn router(plans: Plans, downloads: TranscodeDownloads) -> Router {
    Router::new()
        .route("/media/downloads/{id}/transcodes/{limit}", post(ask))
        .route("/media/downloads/transcodes/{id}", get(poll))
        .route("/media/downloads/transcodes/{id}/file", get(fetch))
        .with_state((plans, downloads))
}

/// Starts a file of the Version `version` within the Playback Limit `limit` for the caller's
/// Device: 202 naming where to poll it, 400 for no limit below Original, 404 for no Version the
/// caller sees, 409 saying why it can't be Transcoded so, and 507 past the budget.
async fn ask(
    State((plans, downloads)): State<(Plans, TranscodeDownloads)>,
    Path((version, limit)): Path<(i64, String)>,
    request: Request,
) -> Response {
    let Some(caller) = request.extensions().get::<Caller>() else {
        return StatusCode::FORBIDDEN.into_response();
    };
    let Some((limit, _)) = NAMED_LIMITS
        .into_iter()
        .find(|(_, named)| named.proto_name() == limit)
    else {
        let text = "name a Playback Limit below Original, such as PLAYBACK_LIMIT_720P";
        return (StatusCode::BAD_REQUEST, text).into_response();
    };
    // Unknown counts as off the local network, as for Direct Play.
    let client = (request.extensions().get::<RequestSource>())
        .map_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED), |source| source.client_ip);
    let plan = match plans.plan_download(caller, version, limit, client).await {
        Ok(Some(plan)) => plan,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            tracing::error!(version, "could not plan a Transcode to download: {err}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let Some(Playing::Transcode {
        transcode,
        duration_ms,
        fallback,
    }) = plan.playing
    else {
        let why: Vec<&str> = (plan.reasons.iter())
            .filter(|reason| reason.rules_out.contains(&Method::Transcode))
            .map(|reason| reason.text.as_str())
            .collect();
        return (StatusCode::CONFLICT, why.join(". ")).into_response();
    };
    match (downloads.start(
        Asker::from(caller),
        version,
        (transcode, duration_ms, fallback),
    ))
    .await
    {
        Ok(id) => (StatusCode::ACCEPTED, [(LOCATION, poll_url(id))]).into_response(),
        Err(StartError::NoSuchVersion) => StatusCode::NOT_FOUND.into_response(),
        Err(err @ StartError::OverBudget { .. }) => {
            (StatusCode::INSUFFICIENT_STORAGE, err.to_string()).into_response()
        }
        Err(err) => {
            tracing::error!(version, "could not start a Transcode to download: {err}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// What the caller's Device's file `id` is at, as JSON: `MAKING` with `progressPercent`, `MADE`
/// with `sizeBytes` and the `url` to fetch it from, or `FAILED`. 404 once it's gone.
async fn poll(
    State((_, downloads)): State<(Plans, TranscodeDownloads)>,
    Path(id): Path<u64>,
    request: Request,
) -> Response {
    let Some((stage, progress, _)) = downloads.find(id, asker_of(&request)).await else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let body = match stage {
        Stage::Making { .. } => {
            let percent = progress.made_ms * 100 / progress.duration_ms.max(1);
            json!({ "state": "MAKING", "progressPercent": percent.clamp(0, 99) })
        }
        Stage::Made { .. } => {
            let url = format!("{}/file", poll_url(id));
            json!({ "state": "MADE", "sizeBytes": progress.bytes, "url": url })
        }
        Stage::Failed { .. } => json!({ "state": "FAILED" }),
    };
    ([(CONTENT_TYPE, "application/json")], body.to_string()).into_response()
}

/// Serves the caller's Device's made file `id` as Direct Play serves a Version's, Range and all,
/// deleting it once a response has sent its last byte; 409 while it's not made.
async fn fetch(
    State((_, downloads)): State<(Plans, TranscodeDownloads)>,
    Path(id): Path<u64>,
    request: Request,
) -> Response {
    let asker = asker_of(&request);
    let Some((stage, _, file)) = downloads.find(id, asker).await else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if !matches!(stage, Stage::Made { .. }) {
        return (StatusCode::CONFLICT, "the file isn't made").into_response();
    }
    let response = match serve_file(&file, request).await {
        Ok(response) => response,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            return StatusCode::NOT_FOUND.into_response();
        }
        Err(err) => {
            tracing::error!("could not serve {}: {err}", file.display());
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let (parts, body) = response.into_parts();
    let Some(length) = length_to_the_end(&parts) else {
        return Response::from_parts(parts, body);
    };
    // Counted, as hyper stops polling a body once it has sent its `Content-Length`, and a client
    // that hangs up first never gets this far.
    let mut left = length;
    let sent = body.into_data_stream().inspect(move |chunk| {
        let Ok(chunk) = chunk else { return };
        let before = left;
        left = left.saturating_sub(chunk.len() as u64);
        if before > 0 && left == 0 {
            downloads.fetched(id);
        }
    });
    Response::from_parts(parts, Body::from_stream(sent))
}

/// Who `request` is from, as its session says.
fn asker_of(request: &Request) -> Option<Asker> {
    request.extensions().get::<Caller>().map(Asker::from)
}

/// How many bytes a response with `parts` sends if they end with a file's last byte: a whole
/// one, or a range to its end.
fn length_to_the_end(parts: &Parts) -> Option<u64> {
    let length = (parts.headers.get(CONTENT_LENGTH))
        .and_then(|length| length.to_str().ok()?.parse().ok())?;
    if parts.status == StatusCode::OK {
        return Some(length);
    }
    // `bytes <first>-<last>/<length>`
    let range = parts.headers.get(CONTENT_RANGE)?.to_str().ok()?;
    let (last, whole) = range.split_once('-')?.1.split_once('/')?;
    let ends = last.parse::<u64>().ok()? + 1 == whole.parse::<u64>().ok()?;
    (parts.status == StatusCode::PARTIAL_CONTENT && ends).then_some(length)
}

fn poll_url(id: u64) -> String {
    format!("/media/downloads/transcodes/{id}")
}

/// Locks `mutex`, which no code panics while holding.
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
#[path = "transcode_downloads_tests.rs"]
mod tests;
