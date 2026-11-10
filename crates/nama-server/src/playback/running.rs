//! Plans that run ffmpeg, while it runs: Remuxes, played over HLS with fMP4 segments.
//!
//! A running plan belongs to the Device it was planned for, and only that Device is served its
//! files, at `/media/plans/<id>/`: the playlist `index.m3u8`, then `init.mp4` and the segments
//! `<n>.m4s`. The playlist lists the whole Version up front, cut at keyframes where it will be
//! cut, so HLS time is the file's time and Watch State needs no conversion. ffmpeg writes a file
//! per keyframe to `data_dir/transcodes/<id>/`, and a segment joins its keyframes' files.
//!
//! A segment asked for waits for ffmpeg to write its files. One far past what ffmpeg has written
//! restarts ffmpeg at it, and ffmpeg stops once it's [`RUN_AHEAD_MS`] past the last segment asked
//! for, to start again at the next file asked for that it hasn't written.

use std::{
    collections::HashMap,
    fmt::Write as _,
    io,
    ops::Range,
    path::{Path, PathBuf},
    process::Output,
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
    time::Duration,
};

use axum::{
    Router,
    extract::{Path as UrlPath, Request, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use tokio::{process::Child, task::AbortHandle};

use super::{Method, played_audio};
use crate::{
    contents::Contents,
    ffmpeg::{Ffmpeg, Keyframe, ProbeError, remux_init},
    tracks::{Probe, Tracks},
    users::Caller,
};

const PLAYLIST: &str = "index.m3u8";
const INIT: &str = "init.mp4";

/// How long segments aim to be, in milliseconds.
const SEGMENT_MS: i64 = 6000;

/// How far past the start of the last segment asked for ffmpeg writes before it stops, in
/// milliseconds.
const RUN_AHEAD_MS: i64 = 120_000;

/// How far past what ffmpeg has written a file asked for can be and still be waited for, in
/// milliseconds. ffmpeg restarts at one further on, since a Remux writes this much in a second or
/// two but a seek far ahead would wait for everything before it.
const WAIT_AHEAD_MS: i64 = 30_000;

/// How often a request for a file ffmpeg hasn't written yet looks for it again, and how often
/// ffmpeg's progress is looked at.
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

/// Starts ffmpeg writing a plan's files, from a keyframe on, to the folder it's given.
type Run = dyn Fn(&Path, Keyframe) -> io::Result<Child> + Send + Sync;

/// The plans running ffmpeg, by id.
// ponytail: nothing stops a plan or deletes its files; cleanup comes with #350.
#[derive(Clone)]
pub(crate) struct RunningPlans {
    inner: Arc<Inner>,
}

struct Inner {
    /// Where each plan's files go, in a folder named by its id.
    dir: PathBuf,
    ffmpeg: Arc<Ffmpeg>,
    contents: Contents,
    tracks: Tracks,
    next_id: AtomicU64,
    plans: Mutex<HashMap<u64, Arc<Running>>>,
}

/// One plan running ffmpeg.
struct Running {
    id: u64,
    /// The Device it belongs to, by id.
    device: i64,
    /// So the Transcode cap (#321) can leave Remuxes out.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "the Transcode cap (#321) is the first to read it")
    )]
    method: Method,
    timeline: Timeline,
    dir: PathBuf,
    run: Box<Run>,
    /// The last segment asked for, by number.
    last_asked: AtomicUsize,
    /// The run of ffmpeg writing its files, and its task, which stops ffmpeg when aborted.
    ffmpeg: tokio::sync::Mutex<Option<(Arc<FfmpegRun>, AbortHandle)>>,
}

/// One run of ffmpeg, from one keyframe on.
struct FfmpegRun {
    /// The keyframe it started at, by number.
    from: usize,
    /// The first keyframe from `from` on whose file isn't written, as last looked.
    written_to: AtomicUsize,
    /// Whether ffmpeg has stopped, so a file not written yet never will be by this run.
    stopped: AtomicBool,
}

/// Where a Version's keyframes fall, and the segments its playlist lists, made of them.
struct Timeline {
    /// When each keyframe falls, in milliseconds, in order.
    keyframes: Vec<i64>,
    segments: Vec<Segment>,
}

/// One segment a playlist lists.
struct Segment {
    /// The keyframe it starts at, by number.
    first_keyframe: usize,
    /// How long it lasts, in milliseconds.
    ms: i64,
}

impl RunningPlans {
    /// Runs plans with `ffmpeg`, writing their files under `dir`.
    pub(crate) fn new(
        dir: PathBuf,
        ffmpeg: Arc<Ffmpeg>,
        contents: Contents,
        tracks: Tracks,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                dir,
                ffmpeg,
                contents,
                tracks,
                next_id: AtomicU64::new(0),
                plans: Mutex::default(),
            }),
        }
    }

    /// Whether plans can run at all (ADR 0026).
    pub(crate) fn can_run(&self) -> bool {
        self.inner.ffmpeg.can_remux()
    }

    /// Starts Remuxing the Version `version`, which probing found holds `probe`, for the Device
    /// `device`, with its audio converted to AAC with `aac_channels` channels, or else copied,
    /// and returns its playlist's URL. Lists the Version's keyframes the first time and keeps
    /// them. `None` if there is no such Version.
    pub(crate) async fn remux(
        &self,
        version: i64,
        device: i64,
        probe: &Probe,
        aac_channels: Option<i64>,
    ) -> Result<Option<String>, StartError> {
        let Some((path, _)) = self.inner.contents.version_file(version).await? else {
            return Ok(None);
        };
        let keyframes = self.keyframes(version, &path).await?;
        let timeline = Timeline::new(keyframes, probe.duration_ms.unwrap_or_default());
        let audio = played_audio(probe);
        let ffmpeg = Arc::clone(&self.inner.ffmpeg);
        let run = move |dir: &Path, from| ffmpeg.remux(&path, audio, aac_channels, dir, from);
        let id = self
            .start(device, Method::Remux, timeline, Box::new(run))
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

    /// Starts a plan for the Device `device`, cut as `timeline` says, whose files `run` starts
    /// ffmpeg writing, and returns the plan's id.
    async fn start(
        &self,
        device: i64,
        method: Method,
        timeline: Timeline,
        run: Box<Run>,
    ) -> io::Result<u64> {
        let id = self.inner.next_id.fetch_add(1, Ordering::Relaxed);
        let dir = self.inner.dir.join(id.to_string());
        // Ids start again with each run, so an earlier run's segments may be there (#350 sweeps
        // them at startup).
        match tokio::fs::remove_dir_all(&dir).await {
            Err(err) if err.kind() != io::ErrorKind::NotFound => return Err(err),
            _ => {}
        }
        tokio::fs::create_dir_all(&dir).await?;
        let running = Arc::new(Running {
            id,
            device,
            method,
            timeline,
            dir,
            run,
            last_asked: AtomicUsize::new(0),
            ffmpeg: tokio::sync::Mutex::default(),
        });
        *running.ffmpeg.lock().await = Some(running.run_from(0)?);
        lock(&self.inner.plans).insert(id, running);
        Ok(id)
    }

    /// The plan `id`, if it's running for the Device `device`.
    fn of_device(&self, id: u64, device: i64) -> Option<Arc<Running>> {
        let plans = lock(&self.inner.plans);
        plans.get(&id).filter(|plan| plan.device == device).cloned()
    }
}

impl Timeline {
    /// A video with `keyframes`, in milliseconds, cut at the first keyframe at or after each
    /// multiple of [`SEGMENT_MS`] past the last cut's multiple. The last segment ends at
    /// `duration_ms`.
    fn new(keyframes: Vec<i64>, duration_ms: i64) -> Self {
        let mut segments = vec![];
        let (mut first_keyframe, mut start, mut cut_after) = (0, 0, SEGMENT_MS);
        for (n, &keyframe) in keyframes.iter().enumerate() {
            if keyframe >= cut_after {
                let ms = keyframe - start;
                segments.push(Segment { first_keyframe, ms });
                (first_keyframe, start) = (n, keyframe);
                cut_after += SEGMENT_MS;
            }
        }
        let ms = (duration_ms - start).max(0);
        segments.push(Segment { first_keyframe, ms });
        Self {
            keyframes,
            segments,
        }
    }

    /// The keyframes whose files segment `n` joins, by number.
    fn keyframes_of(&self, n: usize) -> Range<usize> {
        let first = self
            .segments
            .get(n)
            .map_or(0, |segment| segment.first_keyframe);
        let next = self.segments.get(n + 1);
        first..next.map_or(self.keyframes.len(), |next| next.first_keyframe)
    }

    /// When keyframe `n` falls, in milliseconds, or the end of time for one past the last.
    fn keyframe_ms(&self, n: usize) -> i64 {
        self.keyframes.get(n).copied().unwrap_or(i64::MAX)
    }

    /// When segment `n` starts, in milliseconds.
    fn segment_start_ms(&self, n: usize) -> i64 {
        self.keyframe_ms(self.keyframes_of(n).start)
    }

    /// Keyframe `n`, for ffmpeg to start at.
    fn keyframe(&self, n: usize) -> Keyframe {
        let previous_ms = n
            .checked_sub(1)
            .and_then(|previous| self.keyframes.get(previous));
        Keyframe {
            n,
            ms: self.keyframes.get(n).copied().unwrap_or_default(),
            previous_ms: previous_ms.copied(),
        }
    }

    /// The HLS playlist listing segments `0.m4s`, `1.m4s` and so on.
    fn playlist(&self) -> String {
        let longest = self.segments.iter().map(|segment| segment.ms).max();
        let mut playlist = format!(
            "#EXTM3U\n#EXT-X-VERSION:7\n#EXT-X-TARGETDURATION:{}\n#EXT-X-PLAYLIST-TYPE:VOD\n\
             #EXT-X-MEDIA-SEQUENCE:0\n#EXT-X-INDEPENDENT-SEGMENTS\n#EXT-X-MAP:URI=\"{INIT}\"\n",
            // Whole seconds, rounded up, as HLS asks.
            (longest.unwrap_or_default() + 999) / 1000,
        );
        for (n, Segment { ms, .. }) in self.segments.iter().enumerate() {
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
}

impl Running {
    /// The number of the segment a request for `asked` gets, if the playlist lists it.
    fn segment_number(&self, asked: &str) -> Option<usize> {
        let n: usize = asked.strip_suffix(".m4s")?.parse().ok()?;
        (n < self.timeline.segments.len()).then_some(n)
    }

    /// Segment `n`, its keyframes' files joined, once ffmpeg has written them, or `None` if it
    /// stopped first.
    // ponytail: a segment is read whole into memory; stream it if big Versions strain memory.
    async fn segment(self: &Arc<Self>, n: usize) -> io::Result<Option<Vec<u8>>> {
        self.last_asked.store(n, Ordering::Relaxed);
        let mut segment = vec![];
        for keyframe in self.timeline.keyframes_of(n) {
            if !self.keyframe_written(keyframe, n).await? {
                return Ok(None);
            }
            segment.extend(tokio::fs::read(self.keyframe_path(keyframe)).await?);
        }
        Ok(Some(segment))
    }

    /// The init segment, once ffmpeg has written it whole, or `None` if it stopped first.
    async fn init(&self) -> io::Result<Option<Vec<u8>>> {
        loop {
            let Some((from, stopped)) = (self.ffmpeg.lock().await)
                .as_ref()
                .map(|(run, _)| (run.from, run.stopped.load(Ordering::Acquire)))
            else {
                return Ok(None);
            };
            // Read `stopped` before looking, so a file written just before ffmpeg stopped is
            // still found. A run is only started at a keyframe whose file isn't written.
            if tokio::fs::try_exists(self.keyframe_path(from)).await? {
                return tokio::fs::read(self.dir.join(remux_init(from)))
                    .await
                    .map(Some);
            }
            if stopped {
                return Ok(None);
            }
            tokio::time::sleep(POLL_EVERY).await;
        }
    }

    fn keyframe_path(&self, n: usize) -> PathBuf {
        self.dir.join(format!("{n}.m4s"))
    }

    /// Waits until ffmpeg has written keyframe `n`'s file, for segment `segment`, and says
    /// whether it did. ffmpeg writes each under another name and renames it once whole
    /// (`-hls_flags temp_file`).
    async fn keyframe_written(self: &Arc<Self>, n: usize, segment: usize) -> io::Result<bool> {
        let path = self.keyframe_path(n);
        while !tokio::fs::try_exists(&path).await? {
            if !self.will_write(n, segment).await? {
                // It may have been written just before ffmpeg stopped.
                return tokio::fs::try_exists(&path).await;
            }
            tokio::time::sleep(POLL_EVERY).await;
        }
        Ok(true)
    }

    /// Whether ffmpeg is on its way to keyframe `n`'s file, for segment `segment`, restarting it
    /// there unless it's at most [`WAIT_AHEAD_MS`] short of it. `false` once a run started there
    /// has stopped, or once a later request asked for another segment, so ffmpeg isn't started
    /// over and over.
    async fn will_write(self: &Arc<Self>, n: usize, segment: usize) -> io::Result<bool> {
        let mut ffmpeg = self.ffmpeg.lock().await;
        // Under the lock, since a run started at a written file would write its init again.
        if tokio::fs::try_exists(self.keyframe_path(n)).await? {
            return Ok(true);
        }
        if let Some((run, task)) = &*ffmpeg {
            let stopped = run.stopped.load(Ordering::Acquire);
            if stopped && run.from == n {
                return Ok(false);
            }
            let written_to = (self.timeline).keyframe_ms(run.written_to.load(Ordering::Relaxed));
            let short = self.timeline.keyframe_ms(n).saturating_sub(written_to);
            if !stopped && run.from <= n && short <= WAIT_AHEAD_MS {
                return Ok(true);
            }
            if self.last_asked.load(Ordering::Relaxed) != segment {
                return Ok(false);
            }
            task.abort();
            // An aborted `watch` never marks it, and the restart below may fail.
            run.stopped.store(true, Ordering::Release);
        }
        *ffmpeg = Some(self.run_from(n)?);
        Ok(true)
    }

    /// Starts ffmpeg writing this plan's files from keyframe `n` on.
    fn run_from(self: &Arc<Self>, n: usize) -> io::Result<(Arc<FfmpegRun>, AbortHandle)> {
        let ffmpeg = (self.run)(&self.dir, self.timeline.keyframe(n))?;
        let run = Arc::new(FfmpegRun {
            from: n,
            written_to: AtomicUsize::new(n),
            stopped: AtomicBool::new(false),
        });
        let task = tokio::spawn(Arc::clone(self).watch(ffmpeg, Arc::clone(&run)));
        Ok((run, task.abort_handle()))
    }

    /// Waits for `ffmpeg`, doing the run `run`, to stop, or stops it once it has written far
    /// enough ahead, then logs why if it failed and marks it stopped.
    async fn watch(self: Arc<Self>, ffmpeg: Child, run: Arc<FfmpegRun>) {
        tokio::select! {
            output = ffmpeg.wait_with_output() => log_failure(output, self.id),
            // Dropping the child stops ffmpeg.
            () = self.written_far_ahead(&run) => {}
        }
        run.stopped.store(true, Ordering::Release);
    }

    /// Returns once `run` has written [`RUN_AHEAD_MS`] past the start of the last segment asked
    /// for, or of where it started if later, keeping track of how far it has written meanwhile.
    async fn written_far_ahead(&self, run: &FfmpegRun) {
        let mut written_to = run.from;
        loop {
            tokio::time::sleep(POLL_EVERY).await;
            while written_to < self.timeline.keyframes.len()
                && tokio::fs::try_exists(self.keyframe_path(written_to))
                    .await
                    .unwrap_or_default()
            {
                written_to += 1;
            }
            run.written_to.store(written_to, Ordering::Relaxed);
            let asked = (self.timeline).segment_start_ms(self.last_asked.load(Ordering::Relaxed));
            // An earlier segment asked for just after a seek mustn't stop the run it started.
            let asked = asked.max(self.timeline.keyframe_ms(run.from));
            if self.timeline.keyframe_ms(written_to).saturating_sub(asked) >= RUN_AHEAD_MS {
                return;
            }
        }
    }
}

/// Logs why ffmpeg, running for the plan `plan`, failed, if it did.
fn log_failure(output: io::Result<Output>, plan: u64) {
    match output {
        Ok(Output { status, .. }) if status.success() => {}
        Ok(Output { status, stderr, .. }) => {
            let stderr = String::from_utf8_lossy(&stderr);
            tracing::warn!(plan, "ffmpeg failed ({status}): {}", stderr.trim());
        }
        Err(err) => tracing::warn!(plan, "could not wait for ffmpeg: {err}"),
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
    if file == PLAYLIST {
        let content_type = [(header::CONTENT_TYPE, "application/vnd.apple.mpegurl")];
        return (content_type, plan.timeline.playlist()).into_response();
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

/// Locks `mutex`, which no code panics while holding.
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use std::{
        path::Path,
        sync::{
            Arc, Mutex,
            atomic::{AtomicUsize, Ordering::Relaxed},
        },
        time::Duration,
    };

    use super::{Method, POLL_EVERY, RunningPlans, Timeline, lock};
    use crate::{
        contents::Contents,
        db::Database,
        ffmpeg::{Ffmpeg, Keyframe},
        tracks::Tracks,
    };

    async fn running_plans(data: &Path) -> RunningPlans {
        let db = Arc::new(Database::open(data).await.unwrap());
        RunningPlans::new(
            data.join("transcodes"),
            Arc::new(Ffmpeg::default()),
            Contents::new(Arc::clone(&db)),
            Tracks::new(db),
        )
    }

    /// A Version 1000 seconds long with a keyframe every 2, so each segment is 3 keyframes long.
    fn every_two_seconds() -> Timeline {
        Timeline::new((0..500).map(|n| n * 2000).collect(), 1_000_000)
    }

    /// Starts a plan for the Device 7 whose ffmpeg writes nothing, and returns its id with the
    /// keyframes each run of ffmpeg started at.
    async fn start_writing_nothing(plans: &RunningPlans) -> (u64, Arc<Mutex<Vec<usize>>>) {
        let starts = Arc::new(Mutex::new(vec![]));
        let started = Arc::clone(&starts);
        let run = move |_: &Path, from: Keyframe| {
            lock(&started).push(from.n);
            tokio::process::Command::new("sleep")
                .arg("600")
                .kill_on_drop(true)
                .spawn()
        };
        let id = plans
            .start(7, Method::Remux, every_two_seconds(), Box::new(run))
            .await
            .unwrap();
        (id, starts)
    }

    /// Writes the files of `keyframes` for the plan `id` as ffmpeg would, each holding its number.
    fn write_keyframes(data: &Path, id: u64, keyframes: std::ops::Range<usize>) {
        let dir = data.join("transcodes").join(id.to_string());
        for n in keyframes {
            std::fs::write(dir.join(format!("{n}.m4s")), n.to_string()).unwrap();
        }
    }

    /// What `request` answers, failing the test if it doesn't within 10 seconds.
    async fn answer<T>(request: tokio::task::JoinHandle<T>) -> T {
        let answered = tokio::time::timeout(Duration::from_secs(10), request).await;
        answered.unwrap().unwrap()
    }

    /// Waits until `starts` holds `count` runs.
    async fn until_started(starts: &Mutex<Vec<usize>>, count: usize) -> Vec<usize> {
        let started = async {
            while lock(starts).len() < count {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        };
        tokio::time::timeout(Duration::from_secs(10), started)
            .await
            .unwrap();
        lock(starts).clone()
    }

    // The fake ffmpeg is `sleep`.
    #[cfg(unix)]
    #[tokio::test]
    async fn seeking_past_what_ffmpeg_has_written_restarts_it_at_the_segment_asked_for() {
        let data = tempfile::tempdir().unwrap();
        let plans = running_plans(data.path()).await;
        let (id, starts) = start_writing_nothing(&plans).await;
        let plan = plans.of_device(id, 7).unwrap();

        let eighty_percent = tokio::spawn(async move { plan.segment(133).await });

        assert_eq!(until_started(&starts, 2).await, [0, 399]);
        write_keyframes(data.path(), id, 399..402);
        let segment = answer(eighty_percent).await.unwrap().unwrap();
        assert_eq!(segment, b"399400401");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_request_left_waiting_by_a_later_seek_gives_up_rather_than_restart_ffmpeg() {
        let data = tempfile::tempdir().unwrap();
        let plans = running_plans(data.path()).await;
        let (id, starts) = start_writing_nothing(&plans).await;
        let plan = plans.of_device(id, 7).unwrap();
        let first = Arc::clone(&plan);
        let eighty_percent = tokio::spawn(async move { first.segment(133).await });
        until_started(&starts, 2).await;

        let ten_percent = tokio::spawn(async move { plan.segment(10).await });

        assert_eq!(until_started(&starts, 3).await, [0, 399, 30]);
        assert_eq!(answer(eighty_percent).await.unwrap(), None);
        tokio::time::sleep(POLL_EVERY * 3).await;
        assert_eq!(*lock(&starts), [0, 399, 30]);
        write_keyframes(data.path(), id, 30..33);
        assert_eq!(answer(ten_percent).await.unwrap().unwrap(), b"303132");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_restart_that_fails_leaves_the_next_request_to_start_ffmpeg_again() {
        let data = tempfile::tempdir().unwrap();
        let plans = running_plans(data.path()).await;
        let runs = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&runs);
        let second_fails = move |_: &Path, _: Keyframe| match counted.fetch_add(1, Relaxed) {
            1 => Err(std::io::Error::other("too many open files")),
            _ => tokio::process::Command::new("sleep")
                .arg("600")
                .kill_on_drop(true)
                .spawn(),
        };
        let id = plans
            .start(
                7,
                Method::Remux,
                every_two_seconds(),
                Box::new(second_fails),
            )
            .await
            .unwrap();
        let plan = plans.of_device(id, 7).unwrap();
        plan.segment(133).await.unwrap_err();

        let near_the_start = tokio::spawn(async move { plan.segment(1).await });

        tokio::time::sleep(POLL_EVERY * 3).await;
        assert_eq!(runs.load(Relaxed), 3);
        write_keyframes(data.path(), id, 3..6);
        assert_eq!(answer(near_the_start).await.unwrap().unwrap(), b"345");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_segment_made_earlier_asked_for_after_a_seek_leaves_ffmpeg_running_there() {
        let data = tempfile::tempdir().unwrap();
        let plans = running_plans(data.path()).await;
        let (id, starts) = start_writing_nothing(&plans).await;
        let plan = plans.of_device(id, 7).unwrap();
        write_keyframes(data.path(), id, 0..30);
        let first = Arc::clone(&plan);
        let eighty_percent = tokio::spawn(async move { first.segment(133).await });
        until_started(&starts, 2).await;

        plan.segment(2).await.unwrap().unwrap();

        tokio::time::sleep(POLL_EVERY * 3).await;
        write_keyframes(data.path(), id, 399..402);
        assert_eq!(answer(eighty_percent).await.unwrap().unwrap(), b"399400401");
        assert_eq!(*lock(&starts), [0, 399]);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn segments_already_made_are_served_without_restarting_ffmpeg() {
        let data = tempfile::tempdir().unwrap();
        let plans = running_plans(data.path()).await;
        let (id, starts) = start_writing_nothing(&plans).await;
        let plan = plans.of_device(id, 7).unwrap();
        write_keyframes(data.path(), id, 0..30);
        write_keyframes(data.path(), id, 300..330);

        let back = plan.segment(2).await.unwrap().unwrap();
        let later = plan.segment(101).await.unwrap().unwrap();

        assert_eq!(back, b"678");
        assert_eq!(later, b"303304305");
        assert_eq!(*lock(&starts), [0]);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn ffmpeg_two_minutes_past_the_last_segment_asked_for_stops_and_playing_on_restarts_it() {
        let data = tempfile::tempdir().unwrap();
        let plans = running_plans(data.path()).await;
        let (id, starts) = start_writing_nothing(&plans).await;
        let plan = plans.of_device(id, 7).unwrap();
        write_keyframes(data.path(), id, 0..60);
        plan.segment(0).await.unwrap().unwrap();
        // Long enough for ffmpeg's progress to be looked at.
        tokio::time::sleep(POLL_EVERY * 3).await;

        // Waited for rather than restarted at, were ffmpeg still running.
        let next = tokio::spawn(async move { plan.segment(20).await });

        assert_eq!(until_started(&starts, 2).await, [0, 60]);
        write_keyframes(data.path(), id, 60..63);
        assert_eq!(answer(next).await.unwrap().unwrap(), b"606162");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn the_init_segment_is_served_once_ffmpeg_has_written_its_first_file() {
        let data = tempfile::tempdir().unwrap();
        let plans = running_plans(data.path()).await;
        let (id, _) = start_writing_nothing(&plans).await;
        let plan = plans.of_device(id, 7).unwrap();
        let dir = data.path().join("transcodes").join(id.to_string());
        // ffmpeg makes it empty as it starts, and fills it in as the first file ends.
        std::fs::write(dir.join("init-0.mp4"), "").unwrap();

        let init = tokio::spawn(async move { plan.init().await });

        tokio::time::sleep(POLL_EVERY * 3).await;
        assert!(!init.is_finished());
        std::fs::write(dir.join("init-0.mp4"), "init").unwrap();
        write_keyframes(data.path(), id, 0..1);
        assert_eq!(init.await.unwrap().unwrap().unwrap(), b"init");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_running_plan_is_marked_with_its_method_and_found_only_by_its_device() {
        let data = tempfile::tempdir().unwrap();
        let plans = running_plans(data.path()).await;

        let (id, _) = start_writing_nothing(&plans).await;

        assert_eq!(plans.of_device(id, 7).unwrap().method, Method::Remux);
        assert!(plans.of_device(id, 8).is_none());
    }

    #[test]
    fn segments_are_cut_at_the_first_keyframe_past_each_six_seconds() {
        let every_two_and_a_half_seconds: Vec<i64> = (0..12).map(|n| n * 2500).collect();

        let timeline = Timeline::new(every_two_and_a_half_seconds, 30_020);

        let segment_ms: Vec<i64> = timeline.segments.iter().map(|segment| segment.ms).collect();
        assert_eq!(segment_ms, [7500, 5000, 7500, 5000, 5020]);
        assert_eq!(timeline.keyframes_of(1), 3..5);
        assert_eq!(timeline.keyframes_of(4), 10..12);
    }
}
