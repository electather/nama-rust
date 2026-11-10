//! Plans that run ffmpeg, while it runs: Remuxes, played over HLS with fMP4 segments.
//!
//! A running plan belongs to the Device it was planned for, and only that Device is served its
//! files, at `/media/plans/<id>/`: the playlist `index.m3u8`, then `init.mp4` and the segments
//! `<n>.m4s` that ffmpeg writes to `nama-plan-<id>/` in the segment folder. The playlist lists the
//! whole Version up front, cut where ffmpeg will cut it, so HLS time is the file's time and Watch
//! State needs no conversion. A segment asked for before ffmpeg has written it waits for it.
//!
//! A plan is stopped and its folder deleted [`STOP_UNUSED_AFTER`] after it was last asked for, and
//! [`sweep_leftovers`] deletes the folders of plans a run left behind when the next starts.

use std::{
    collections::HashMap,
    fmt::Write as _,
    io,
    path::{Path, PathBuf},
    process::Output,
    sync::{
        Arc, Mutex, PoisonError, Weak,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use axum::{
    Router,
    extract::{Path as UrlPath, Request, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use tokio::{process::Child, task::JoinHandle};
use tower_http::services::ServeFile;

use super::{Method, played_audio};
use crate::{
    contents::Contents,
    ffmpeg::{Ffmpeg, HLS_INIT, HLS_TIME_MS, ProbeError},
    server_settings::SegmentFolder,
    tracks::{Probe, Tracks},
    users::Caller,
};

const PLAYLIST: &str = "index.m3u8";

/// What a plan's folder in the segment folder is named, before its id. The sweep deletes only
/// folders named so, since the admin may set a segment folder that holds other files.
const PLAN_FOLDER: &str = "nama-plan-";

/// How long after a plan was last asked for it's stopped and its files deleted, so a closed tab's
/// plan goes within about a minute.
const STOP_UNUSED_AFTER: Duration = Duration::from_secs(60);

const CHECK_UNUSED_EVERY: Duration = Duration::from_secs(10);

/// How often a request for a file ffmpeg hasn't written yet looks for it again.
const POLL_EVERY: Duration = Duration::from_millis(100);

/// Why a plan couldn't start running.
#[derive(Debug, thiserror::Error)]
pub(crate) enum StartError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error("the Version's keyframes couldn't be listed: {0}")]
    Keyframes(#[from] ProbeError),
    #[error("ffmpeg couldn't be started: {0}")]
    Run(#[from] io::Error),
}

/// The plans running ffmpeg, by id.
// ponytail: ffmpeg runs once through the whole Version unless the plan goes unused; seeking and
// stopping when far ahead come with #348.
#[derive(Clone)]
pub(crate) struct RunningPlans {
    inner: Arc<Inner>,
}

struct Inner {
    /// Where each plan's files go, in a folder named by its id.
    folder: SegmentFolder,
    ffmpeg: Arc<Ffmpeg>,
    contents: Contents,
    tracks: Tracks,
    next_id: AtomicU64,
    plans: Mutex<HashMap<u64, Arc<Running>>>,
}

/// One plan running ffmpeg.
struct Running {
    /// The Device it belongs to, by id.
    device: i64,
    /// So the Transcode cap (#321) can leave Remuxes out.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "the Transcode cap (#321) is the first to read it")
    )]
    method: Method,
    /// How long each segment is, in milliseconds, in order.
    segment_ms: Vec<i64>,
    dir: PathBuf,
    /// Whether ffmpeg has stopped, so a file not written yet never will be.
    stopped: Arc<AtomicBool>,
    /// The task waiting for ffmpeg, which owns it, until the plan is stopped.
    ffmpeg: Mutex<Option<JoinHandle<()>>>,
    last_asked: Mutex<Instant>,
}

impl RunningPlans {
    /// Runs plans with `ffmpeg`, writing their files to the segment folder `folder` as it is
    /// when each starts. Until these are dropped, stops the plans gone unused in the background.
    pub(crate) fn new(
        folder: SegmentFolder,
        ffmpeg: Arc<Ffmpeg>,
        contents: Contents,
        tracks: Tracks,
    ) -> Self {
        let inner = Arc::new(Inner {
            folder,
            ffmpeg,
            contents,
            tracks,
            next_id: AtomicU64::new(0),
            plans: Mutex::default(),
        });
        tokio::spawn(stop_unused_plans(Arc::downgrade(&inner)));
        Self { inner }
    }

    /// Whether plans can run at all (ADR 0026).
    pub(crate) fn can_run(&self) -> bool {
        self.inner.ffmpeg.can_remux()
    }

    /// Starts Remuxing the Version `version`, which probing found holds `probe`, for the Device
    /// `device`, and returns its playlist's URL. Lists the Version's keyframes the first time
    /// and keeps them. `None` if there is no such Version.
    pub(crate) async fn remux(
        &self,
        version: i64,
        device: i64,
        probe: &Probe,
    ) -> Result<Option<String>, StartError> {
        let Some((path, _)) = self.inner.contents.version_file(version).await? else {
            return Ok(None);
        };
        let keyframes = self.keyframes(version, &path).await?;
        let segment_ms = segments(&keyframes, probe.duration_ms.unwrap_or_default());
        let audio = played_audio(probe);
        let id = self
            .start(device, Method::Remux, segment_ms, |dir| {
                self.inner.ffmpeg.remux(&path, audio, dir)
            })
            .await?;
        Ok(Some(format!("/media/plans/{id}/{PLAYLIST}")))
    }

    /// The Version `version`'s keyframes, as kept, or else listed from its file at `path` and
    /// kept for next time.
    async fn keyframes(&self, version: i64, path: &str) -> Result<Vec<i64>, StartError> {
        if let Some(keyframes) = self.inner.tracks.keyframes(version).await? {
            return Ok(keyframes);
        }
        let keyframes = self.inner.ffmpeg.keyframes(path).await?;
        self.inner
            .tracks
            .store_keyframes(version, &keyframes)
            .await?;
        Ok(keyframes)
    }

    /// Starts a plan for the Device `device`, whose ffmpeg `run` starts writing `segment_ms`'s
    /// segments to the folder it's given, and returns the plan's id.
    async fn start(
        &self,
        device: i64,
        method: Method,
        segment_ms: Vec<i64>,
        run: impl FnOnce(&Path) -> io::Result<Child>,
    ) -> io::Result<u64> {
        let id = self.inner.next_id.fetch_add(1, Ordering::Relaxed);
        let dir = self.inner.folder.path().join(format!("{PLAN_FOLDER}{id}"));
        // Ids start again with each run, and the sweep at startup misses a segment folder an
        // earlier run had set but this one doesn't, so its segments may be there.
        match tokio::fs::remove_dir_all(&dir).await {
            Err(err) if err.kind() != io::ErrorKind::NotFound => return Err(err),
            _ => {}
        }
        tokio::fs::create_dir_all(&dir).await?;
        let ffmpeg = run(&dir)?;
        let stopped = Arc::new(AtomicBool::new(false));
        let ffmpeg = tokio::spawn(log_when_stopped(ffmpeg, id, Arc::clone(&stopped)));
        let running = Running {
            device,
            method,
            segment_ms,
            dir,
            stopped,
            ffmpeg: Mutex::new(Some(ffmpeg)),
            last_asked: Mutex::new(Instant::now()),
        };
        lock(&self.inner.plans).insert(id, Arc::new(running));
        Ok(id)
    }

    /// The plan `id`, if it's running for the Device `device`.
    fn of_device(&self, id: u64, device: i64) -> Option<Arc<Running>> {
        let plans = lock(&self.inner.plans);
        plans.get(&id).filter(|plan| plan.device == device).cloned()
    }

    /// Stops every plan not asked for in the last `unused_for`, and deletes its files.
    async fn stop_unused(&self, unused_for: Duration) {
        // Taken out under the one lock, so a plan asked for after the check isn't stopped.
        let unused: Vec<(u64, Arc<Running>)> = lock(&self.inner.plans)
            .extract_if(|_, plan| lock(&plan.last_asked).elapsed() >= unused_for)
            .collect();
        for (id, plan) in unused {
            stop(id, &plan).await;
        }
    }
}

/// Stops the plan `id`'s ffmpeg and deletes its files.
async fn stop(id: u64, plan: &Running) {
    let ffmpeg = lock(&plan.ffmpeg).take();
    if let Some(ffmpeg) = ffmpeg {
        ffmpeg.abort();
        // Awaited so ffmpeg is dropped, which kills it, before its folder is deleted.
        let _: Result<(), _> = ffmpeg.await;
    }
    plan.stopped.store(true, Ordering::Release);
    if let Err(err) = tokio::fs::remove_dir_all(&plan.dir).await {
        tracing::warn!(plan = id, "could not delete {}: {err}", plan.dir.display());
    }
}

/// Every [`CHECK_UNUSED_EVERY`], stops the plans unused for [`STOP_UNUSED_AFTER`], until `inner`
/// is dropped.
async fn stop_unused_plans(inner: Weak<Inner>) {
    let mut every = tokio::time::interval(CHECK_UNUSED_EVERY);
    loop {
        every.tick().await;
        let Some(inner) = inner.upgrade() else {
            return;
        };
        RunningPlans { inner }.stop_unused(STOP_UNUSED_AFTER).await;
    }
}

/// Deletes the plan folders an earlier run left in the segment folder `folder` and the default.
/// Runs before any plan starts, whose folder it would delete too.
pub(crate) async fn sweep_leftovers(folder: &SegmentFolder) {
    for path in folder.leftover_paths() {
        if let Err(err) = sweep(&path).await {
            tracing::warn!(
                "could not delete the segment files left in {}: {err}",
                path.display()
            );
        }
    }
}

/// Deletes the plan folders in `path`.
async fn sweep(path: &Path) -> io::Result<()> {
    let mut entries = match tokio::fs::read_dir(path).await {
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        entries => entries?,
    };
    while let Some(entry) = entries.next_entry().await? {
        if entry.file_name().to_string_lossy().starts_with(PLAN_FOLDER) {
            tokio::fs::remove_dir_all(entry.path()).await?;
        }
    }
    Ok(())
}

/// Waits for `ffmpeg` to stop, logs why if it failed, and marks it `stopped`.
async fn log_when_stopped(ffmpeg: Child, plan: u64, stopped: Arc<AtomicBool>) {
    match ffmpeg.wait_with_output().await {
        Ok(Output { status, .. }) if status.success() => {}
        Ok(Output { status, stderr, .. }) => {
            let stderr = String::from_utf8_lossy(&stderr);
            tracing::warn!(plan, "ffmpeg failed ({status}): {}", stderr.trim());
        }
        Err(err) => tracing::warn!(plan, "could not wait for ffmpeg: {err}"),
    }
    stopped.store(true, Ordering::Release);
}

/// How long each segment of a video with `keyframes`, in milliseconds, lasts when cut where
/// ffmpeg's HLS muxer cuts a copied video: at the first keyframe at or after each multiple of
/// [`HLS_TIME_MS`] past the last cut's multiple. The last ends at `duration_ms`.
fn segments(keyframes: &[i64], duration_ms: i64) -> Vec<i64> {
    let mut segment_ms = vec![];
    let (mut start, mut cut_after) = (0, HLS_TIME_MS);
    for &keyframe in keyframes {
        if keyframe >= cut_after {
            segment_ms.push(keyframe - start);
            start = keyframe;
            cut_after += HLS_TIME_MS;
        }
    }
    segment_ms.push((duration_ms - start).max(0));
    segment_ms
}

/// The HLS playlist listing segments `0.m4s`, `1.m4s` and so on lasting `segment_ms`.
fn playlist(segment_ms: &[i64]) -> String {
    let longest = segment_ms.iter().max().copied().unwrap_or_default();
    let mut playlist = format!(
        "#EXTM3U\n#EXT-X-VERSION:7\n#EXT-X-TARGETDURATION:{}\n#EXT-X-PLAYLIST-TYPE:VOD\n\
         #EXT-X-MEDIA-SEQUENCE:0\n#EXT-X-INDEPENDENT-SEGMENTS\n#EXT-X-MAP:URI=\"{HLS_INIT}\"\n",
        // Whole seconds, rounded up, as HLS asks.
        (longest + 999) / 1000,
    );
    for (n, ms) in segment_ms.iter().enumerate() {
        // Writing to a `String` never fails.
        let _ = write!(
            playlist,
            "#EXTINF:{}.{:03},\n{n}.m4s\n",
            ms / 1000,
            ms % 1000
        );
    }
    playlist.push_str("#EXT-X-ENDLIST\n");
    playlist
}

impl Running {
    /// Marks the plan asked for now, which keeps it running another [`STOP_UNUSED_AFTER`].
    fn asked(&self) {
        *lock(&self.last_asked) = Instant::now();
    }

    /// The file a request for `asked` gets: the init segment, or a segment the playlist lists,
    /// named as ffmpeg names it.
    fn file_name(&self, asked: &str) -> Option<String> {
        if asked == HLS_INIT {
            return Some(HLS_INIT.to_owned());
        }
        let n: usize = asked.strip_suffix(".m4s")?.parse().ok()?;
        (n < self.segment_ms.len()).then(|| format!("{n}.m4s"))
    }

    /// Waits until ffmpeg has written `path`, and says whether it did before it stopped. ffmpeg
    /// writes each under another name and renames it once whole (`-hls_flags temp_file`).
    async fn written(&self, path: &Path) -> io::Result<bool> {
        loop {
            // Read before looking, so a file written just before ffmpeg stopped is still found.
            let stopped = self.stopped.load(Ordering::Acquire);
            if tokio::fs::try_exists(path).await? {
                return Ok(true);
            }
            if stopped {
                return Ok(false);
            }
            tokio::time::sleep(POLL_EVERY).await;
        }
    }
}

/// Serves `GET /media/plans/<id>/<file>`: the playlist and segments of the plan `id`, to the
/// Device it belongs to, and 404 to any other. Whether the caller has a session is up to the
/// layers the router is put behind.
pub(crate) fn router(plans: RunningPlans) -> Router {
    Router::new()
        .route("/media/plans/{id}/{file}", get(serve))
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
    let Some(plan) = device.and_then(|device| plans.of_device(id, device)) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    plan.asked();
    if file == PLAYLIST {
        let content_type = [(header::CONTENT_TYPE, "application/vnd.apple.mpegurl")];
        return (content_type, playlist(&plan.segment_ms)).into_response();
    }
    let Some(name) = plan.file_name(&file) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let path = plan.dir.join(name);
    match plan.written(&path).await {
        Ok(true) => {}
        Ok(false) => return StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            tracing::error!("could not look for {}: {err}", path.display());
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    }
    match ServeFile::new(&path).try_call(request).await {
        Ok(response) => response.into_response(),
        Err(err) => {
            tracing::error!("could not serve {}: {err}", path.display());
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Locks `mutex`, which no code panics while holding.
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path, sync::Arc, time::Duration};

    use super::{Method, RunningPlans, segments, sweep_leftovers};
    use crate::{
        contents::Contents, db::Database, ffmpeg::Ffmpeg, server_settings::SegmentFolder,
        tracks::Tracks,
    };

    fn plans_in(db: &Arc<Database>, folder: SegmentFolder) -> RunningPlans {
        RunningPlans::new(
            folder,
            Arc::new(Ffmpeg::default()),
            Contents::new(Arc::clone(db)),
            Tracks::new(Arc::clone(db)),
        )
    }

    async fn database(data: &Path) -> Arc<Database> {
        Arc::new(Database::open(data).await.unwrap())
    }

    /// Starts a plan for the Device `device` whose ffmpeg is `program` run with `args`, and
    /// returns its id.
    async fn start(plans: &RunningPlans, device: i64, program: &str, args: &[&str]) -> u64 {
        plans
            .start(device, Method::Remux, vec![6000], |_| {
                tokio::process::Command::new(program)
                    .args(args)
                    .kill_on_drop(true)
                    .spawn()
            })
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn a_running_plan_is_marked_with_its_method_and_found_only_by_its_device() {
        let data = tempfile::tempdir().unwrap();
        let db = database(data.path()).await;
        let plans = plans_in(&db, SegmentFolder::new(data.path().join("transcodes")));
        let (device, another) = (7, 8);

        let id = start(&plans, device, "true", &[]).await;

        assert_eq!(plans.of_device(id, device).unwrap().method, Method::Remux);
        assert!(plans.of_device(id, another).is_none());
    }

    // The stand-in ffmpeg is `sleep`.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_plan_unused_for_long_enough_is_stopped_and_its_files_deleted() {
        let data = tempfile::tempdir().unwrap();
        let db = database(data.path()).await;
        let plans = plans_in(&db, SegmentFolder::new(data.path().join("transcodes")));
        let device = 7;
        let (used, unused) = (
            start(&plans, device, "sleep", &["60"]).await,
            start(&plans, device, "sleep", &["60"]).await,
        );
        let dir = |id| plans.of_device(id, device).unwrap().dir.clone();
        let (used_dir, unused_dir) = (dir(used), dir(unused));
        let stopped = Arc::clone(&plans.of_device(unused, device).unwrap().stopped);
        tokio::time::sleep(Duration::from_millis(100)).await;
        plans.of_device(used, device).unwrap().asked();

        plans.stop_unused(Duration::from_millis(50)).await;

        assert!(plans.of_device(unused, device).is_none());
        assert!(stopped.load(std::sync::atomic::Ordering::Acquire));
        assert!(!unused_dir.exists());
        assert!(plans.of_device(used, device).is_some());
        assert!(used_dir.exists());
    }

    #[tokio::test]
    async fn a_new_plan_writes_to_the_segment_folder_set_since_the_last() {
        let data = tempfile::tempdir().unwrap();
        let folder = SegmentFolder::new(data.path().join("transcodes"));
        let db = database(data.path()).await;
        let plans = plans_in(&db, folder.clone());
        let before = start(&plans, 7, "true", &[]).await;
        let tmpfs = data.path().join("tmpfs");
        folder
            .save(&db, tmpfs.to_str().unwrap().to_owned())
            .await
            .unwrap();

        let after = start(&plans, 7, "true", &[]).await;

        assert!(
            plans
                .of_device(before, 7)
                .unwrap()
                .dir
                .starts_with(data.path().join("transcodes"))
        );
        assert!(plans.of_device(after, 7).unwrap().dir.starts_with(&tmpfs));
    }

    #[tokio::test]
    async fn the_sweep_deletes_plan_folders_in_the_set_and_default_folders_and_nothing_else() {
        let data = tempfile::tempdir().unwrap();
        let (default, set) = (data.path().join("transcodes"), data.path().join("tmpfs"));
        let leftovers = [default.join("nama-plan-0"), set.join("nama-plan-3")];
        for leftover in &leftovers {
            fs::create_dir_all(leftover).unwrap();
            fs::write(leftover.join("0.m4s"), "segment").unwrap();
        }
        let others = set.join("someone else's");
        fs::create_dir_all(&others).unwrap();
        let folder = SegmentFolder::new(default);
        let db = database(data.path()).await;
        folder
            .save(&db, set.to_str().unwrap().to_owned())
            .await
            .unwrap();

        sweep_leftovers(&folder).await;

        for leftover in leftovers {
            assert!(!leftover.exists(), "{}", leftover.display());
        }
        assert!(others.exists());
    }

    #[test]
    fn segments_are_cut_at_the_first_keyframe_past_each_six_seconds() {
        let every_two_and_a_half_seconds: Vec<i64> = (0..12).map(|n| n * 2500).collect();

        let segment_ms = segments(&every_two_and_a_half_seconds, 30_020);

        // ffmpeg 7.0 cut a 30-second H.264 file with these keyframes at 7.5, 12.5, 20 and 25 s.
        assert_eq!(segment_ms, [7500, 5000, 7500, 5000, 5020]);
    }
}
