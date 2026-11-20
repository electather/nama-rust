use std::{
    io,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicI64, AtomicUsize, Ordering},
    },
};

use futures_util::future::BoxFuture;
use tokio::task::JoinHandle;

use super::{
    Method, POLL_EVERY, REMUX_WAIT_AHEAD_MS, RUN_AHEAD_MS,
    timeline::{SEGMENT_MS, Timeline, keyframe_number},
};
use crate::ffmpeg::{Keyframe, hls_init, hls_playlist, hls_run_from};

/// How ffmpeg's stderr says a write failed with ENOSPC. ffmpeg, not the Server, writes a plan's
/// files, so its words are all the Server sees of it.
const DISK_FULL: &str = "No space left on device";

/// Starts a run writing a plan's files, from a keyframe on, to the folder it's given: one file
/// per keyframe of the file, listed or not, which the playlist named by [`hls_playlist`] lists,
/// and an init file named by [`hls_init`]. The plan joins them into one per listed keyframe, named
/// by its number such as `3.m4s`.
pub(super) type Start = dyn Fn(&Path, Keyframe) -> io::Result<Run> + Send + Sync;

/// A run writing a plan's files, which ends with how it went, and stops when dropped.
pub(super) type Run = BoxFuture<'static, Outcome>;

/// How a run ended.
#[derive(Debug)]
pub(super) enum Outcome {
    /// It wrote every file it was started for.
    Finished,
    /// It failed with `status`, as ffmpeg's exit status reads, having written `stderr`.
    Failed { status: String, stderr: String },
    /// It couldn't be waited for.
    Unwaited(io::Error),
}

/// What starts a plan's runs, and how they convert.
pub(super) struct Runs {
    /// How far ahead a request waits for a run, and so the Transcode cap leaves Remuxes out.
    pub(super) method: Method,
    /// Starts each run, unless it fell back.
    pub(super) start: Box<Start>,
    /// Whether `start` works the CPU hard, which the default Transcode cap counts.
    pub(super) cpu_bound: bool,
    /// For a Transcode on hardware, what it runs once a run there fails.
    pub(super) fallback: Option<Fallback>,
}

/// What a Transcode on hardware runs instead once a run there fails.
pub(super) struct Fallback {
    /// Starts each run once it fell back.
    pub(super) start: Box<Start>,
    /// Called once, as the plan falls back, so its owner can tell Observers.
    pub(super) on_fall_back: Box<dyn Fn() + Send + Sync>,
}

/// A plan's HLS files, and the runs writing them.
pub(super) struct Running {
    /// The plan's id, which its log lines name.
    id: u64,
    method: Method,
    timeline: Timeline,
    dir: PathBuf,
    start: Box<Start>,
    /// The last segment asked for, by number.
    last_asked: AtomicUsize,
    /// Whether the plan was stopped, so no run is started again for requests still waiting.
    closed: AtomicBool,
    fallback: Option<Fallback>,
    /// Whether a run on hardware failed, so `fallback` runs from then on.
    fell_back: AtomicBool,
    /// Whether it started working the CPU hard, which the default Transcode cap counts.
    started_cpu_bound: bool,
    /// The time kept behind playback as last told, which a warning that the segment folder is
    /// full names.
    keep_behind_ms: AtomicI64,
    /// The run writing its files, and its task, which stops the run when aborted.
    ffmpeg: tokio::sync::Mutex<Option<(Arc<FfmpegRun>, JoinHandle<()>)>>,
}

/// One run, from one keyframe on.
struct FfmpegRun {
    /// The keyframe it started at, by number.
    from: usize,
    /// The first keyframe from `from` on it hadn't written, as last looked. A file behind it that's
    /// gone since doesn't move it back.
    written_to: AtomicUsize,
    /// Whether the run has stopped, so a file not written yet never will be by it.
    stopped: AtomicBool,
    /// How far its files were joined into listed keyframes' files. Locked while joining, which
    /// `watch` and a tick may both do.
    joined: tokio::sync::Mutex<Joined>,
}

/// How far a run's files were joined into listed keyframes' files.
#[derive(Default)]
struct Joined {
    /// How many of the files its playlist lists were.
    files: usize,
    /// Whether joining them failed since it started, which is logged only once.
    failed: bool,
}

impl Running {
    /// Starts the plan `id`'s first run writing its files, cut as `timeline` says, to `dir`, from
    /// the first keyframe of the segment playing at `start_ms`, and returns the plan. A warning
    /// that the segment folder, `dir`'s parent, is full names `keep_behind_ms`, until
    /// [`Self::delete_unkept`] is told another.
    #[expect(
        clippy::too_many_arguments,
        reason = "each is a different part of the plan, and `start_ms` only says where its first \
                  run starts"
    )]
    pub(super) async fn start(
        id: u64,
        timeline: Timeline,
        dir: PathBuf,
        start_ms: i64,
        runs: Runs,
        keep_behind_ms: i64,
    ) -> io::Result<Arc<Self>> {
        // Its segment's first keyframe, not the one nearest: a run starting partway into a
        // segment is restarted as soon as the player asks for that segment.
        let start_segment = timeline.segment_at(start_ms);
        let first_keyframe = timeline.keyframes_of(start_segment).start;
        let running = Arc::new(Self {
            id,
            method: runs.method,
            timeline,
            dir,
            start: runs.start,
            last_asked: AtomicUsize::new(start_segment),
            closed: AtomicBool::new(false),
            fallback: runs.fallback,
            fell_back: AtomicBool::new(false),
            started_cpu_bound: runs.cpu_bound,
            keep_behind_ms: AtomicI64::new(keep_behind_ms),
            ffmpeg: tokio::sync::Mutex::default(),
        });
        *running.ffmpeg.lock().await = Some(running.run_from(first_keyframe)?);
        Ok(running)
    }

    /// Whether it Remuxes or Transcodes.
    pub(super) const fn method(&self) -> Method {
        self.method
    }

    /// Whether it works the CPU hard now: it started so, or fell back to software.
    pub(super) fn cpu_bound(&self) -> bool {
        self.started_cpu_bound || self.fell_back()
    }

    /// Whether a run on hardware failed, so it runs its fallback from then on.
    pub(super) fn fell_back(&self) -> bool {
        self.fell_back.load(Ordering::Acquire)
    }

    /// The HLS playlist, listing every segment.
    pub(super) fn playlist(&self) -> String {
        self.timeline.playlist()
    }

    /// Stops its run and deletes its files.
    pub(super) async fn stop(&self) {
        // Under the lock a restart takes, so none starts ffmpeg after this.
        let mut ffmpeg = self.ffmpeg.lock().await;
        self.closed.store(true, Ordering::Release);
        if let Some((run, task)) = ffmpeg.take() {
            task.abort();
            // Awaited so the run is dropped, which stops it, before its folder is deleted.
            let _: Result<(), _> = task.await;
            run.stopped.store(true, Ordering::Release);
        }
        drop(ffmpeg);
        if let Err(err) = tokio::fs::remove_dir_all(&self.dir).await {
            tracing::warn!(
                plan = self.id,
                "could not delete {}: {err}",
                self.dir.display()
            );
        }
    }

    /// Deletes the files of keyframes ending more than `keep_behind_ms` before the start of the
    /// last segment asked for, or starting [`RUN_AHEAD_MS`] or more past it, such as those a seek
    /// back left (ADR 0062), and the files every run but the current one wrote itself. Those behind
    /// that the current run may still be writing are kept, and while it runs, those less than
    /// [`RUN_AHEAD_MS`] past where it started, since a seek ahead started it there. A warning that
    /// the segment folder is full names `keep_behind_ms` from then on.
    pub(super) async fn delete_unkept(&self, keep_behind_ms: i64) -> io::Result<()> {
        self.keep_behind_ms.store(keep_behind_ms, Ordering::Relaxed);
        // Under the lock a restart takes, so the current run is the one whose init is kept.
        let ffmpeg = self.ffmpeg.lock().await;
        // `stop` takes the run under it, so a stopped plan has none.
        let Some((run, task)) = ffmpeg.as_ref() else {
            return Ok(());
        };
        let running = !run.stopped.load(Ordering::Acquire);
        // A running run's walk in `note_written` stops at the first file gone, so what it may
        // write after it last looked is kept, unless it's past where `written_far_ahead` stops
        // the run, where the walk stopping does too.
        let still_writing_from = if running {
            self.note_written(run).await
        } else {
            usize::MAX
        };
        let asked = (self.timeline).segment_start_ms(self.last_asked.load(Ordering::Relaxed));
        let ahead_from_ms = if running {
            self.run_ahead_from_ms(run)
        } else {
            asked
        };
        let keep_from_ms = asked - keep_behind_ms;
        let keep_to_ms = ahead_from_ms + RUN_AHEAD_MS;
        let mut deleted_unwalked = false;
        let mut saw_walk_end = false;
        let mut entries = tokio::fs::read_dir(&self.dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let unkept = match keyframe_number(&name) {
                Some(n) => {
                    let far_ahead = self.timeline.keyframe_ms(n) >= keep_to_ms;
                    deleted_unwalked |= far_ahead && n >= still_writing_from;
                    saw_walk_end |= n == still_writing_from;
                    far_ahead
                        || (n < still_writing_from
                            && self.timeline.keyframe_ms(n + 1) < keep_from_ms)
                }
                None => hls_run_from(&name).is_some_and(|from| from != run.from),
            };
            // Going on past one, which Windows refuses while a request reads it.
            if unkept && let Err(err) = tokio::fs::remove_file(entry.path()).await {
                tracing::warn!(
                    plan = self.id,
                    "could not delete {}: {err}",
                    entry.path().display()
                );
            }
        }
        // ffmpeg writes its files in order, so it wrote one deleted past where the walk stopped
        // only if it has written the file there since, which is still there or was seen and maybe
        // deleted above. Its walk would then wait at the deleted one for good, so the run is
        // stopped, and the request for it starts ffmpeg again; so it is too if that can't be told.
        if deleted_unwalked
            && (saw_walk_end
                || !matches!(
                    tokio::fs::try_exists(self.keyframe_path(still_writing_from)).await,
                    Ok(false)
                ))
        {
            task.abort();
            self.note_written(run).await;
            run.stopped.store(true, Ordering::Release);
        }
        Ok(())
    }

    /// The number of the segment a request for `asked` gets, if the playlist lists it.
    pub(super) fn segment_number(&self, asked: &str) -> Option<usize> {
        let n: usize = asked.strip_suffix(".m4s")?.parse().ok()?;
        (n < self.timeline.segments.len()).then_some(n)
    }

    /// Segment `n`, its keyframes' files joined, once ffmpeg has written them, or `None` if it
    /// stopped first.
    // ponytail: a segment is read whole into memory; stream it if big Versions strain memory.
    pub(super) async fn segment(self: &Arc<Self>, n: usize) -> io::Result<Option<Vec<u8>>> {
        self.last_asked.store(n, Ordering::Relaxed);
        let mut segment = vec![];
        for keyframe in self.timeline.keyframes_of(n) {
            let Some(file) = self.keyframe_file(keyframe, n).await? else {
                return Ok(None);
            };
            segment.extend(file);
        }
        Ok(Some(segment))
    }

    /// Keyframe `n`'s file, for segment `segment`, once ffmpeg has written it, or `None` if it
    /// stopped first. One gone, even between being found written and read, is waited for as one
    /// not written yet. ffmpeg writes each under another name and renames it once whole
    /// (`-hls_flags temp_file`).
    async fn keyframe_file(
        self: &Arc<Self>,
        n: usize,
        segment: usize,
    ) -> io::Result<Option<Vec<u8>>> {
        loop {
            if let Some(file) = self.read_keyframe(n).await? {
                return Ok(Some(file));
            }
            if !self.will_write(n, segment).await? {
                // It may have been written just before ffmpeg stopped.
                return self.read_keyframe(n).await;
            }
            tokio::time::sleep(POLL_EVERY).await;
        }
    }

    /// Keyframe `n`'s file, or `None` if it isn't there.
    async fn read_keyframe(&self, n: usize) -> io::Result<Option<Vec<u8>>> {
        match tokio::fs::read(self.keyframe_path(n)).await {
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
            read => read.map(Some),
        }
    }

    /// The init segment, once the current run of ffmpeg has written it whole, as it has once it
    /// wrote its first file, or `None` if it stopped first.
    pub(super) async fn init(&self) -> io::Result<Option<Vec<u8>>> {
        loop {
            let ffmpeg = self.ffmpeg.lock().await;
            let Some((run, _)) = ffmpeg.as_ref() else {
                return Ok(None);
            };
            // `stopped` first, as a run notes what it wrote last before it's marked so.
            let stopped = run.stopped.load(Ordering::Acquire);
            // Not whether its first file is there, which may be gone since.
            if run.written_to.load(Ordering::Relaxed) > run.from {
                // Read under the lock, as once a seek replaces this run a tick deletes its init.
                return tokio::fs::read(self.dir.join(hls_init(run.from)))
                    .await
                    .map(Some);
            }
            drop(ffmpeg);
            if stopped {
                return Ok(None);
            }
            tokio::time::sleep(POLL_EVERY).await;
        }
    }

    fn keyframe_path(&self, n: usize) -> PathBuf {
        self.dir.join(format!("{n}.m4s"))
    }

    /// How far past what ffmpeg has written a file asked for can be and still be waited for, in
    /// milliseconds. A software Transcode writes about as fast as it plays, so it waits only for
    /// the segment ffmpeg is on and the next.
    const fn wait_ahead_ms(&self) -> i64 {
        match self.method {
            Method::Remux | Method::DirectPlay => REMUX_WAIT_AHEAD_MS,
            Method::Transcode => SEGMENT_MS,
        }
    }

    /// Whether ffmpeg is on its way to keyframe `n`'s file, for segment `segment`, restarting it
    /// there unless it's at most [`Self::wait_ahead_ms`] short of it. `false` once a run started
    /// there has stopped without writing it, or once a later request asked for a segment more
    /// than that away, so ffmpeg isn't started over and over.
    async fn will_write(self: &Arc<Self>, n: usize, segment: usize) -> io::Result<bool> {
        let mut ffmpeg = self.ffmpeg.lock().await;
        if self.closed.load(Ordering::Acquire) {
            return Ok(false);
        }
        // Under the lock, since a run started at a written file would write its init again.
        if tokio::fs::try_exists(self.keyframe_path(n)).await? {
            return Ok(true);
        }
        if let Some((run, task)) = &*ffmpeg {
            let stopped = run.stopped.load(Ordering::Acquire);
            let written_to = run.written_to.load(Ordering::Relaxed);
            if stopped && run.from == n && written_to == n {
                return Ok(false);
            }
            // One it wrote that's gone since is restarted at, as it won't write it again.
            let short = (self.timeline.keyframe_ms(n))
                .saturating_sub(self.timeline.keyframe_ms(written_to));
            if !stopped && written_to <= n && short <= self.wait_ahead_ms() {
                return Ok(true);
            }
            // A player fetching the next segment alongside this one mustn't leave it behind.
            let last_asked = self.last_asked.load(Ordering::Relaxed);
            let apart = (self.timeline.segment_start_ms(last_asked))
                .abs_diff(self.timeline.segment_start_ms(segment));
            if apart > self.wait_ahead_ms().unsigned_abs() {
                return Ok(false);
            }
            task.abort();
            // An aborted `watch` never notes what it wrote last or marks it stopped, and the
            // restart below may fail, leaving it for `init`.
            self.note_written(run).await;
            run.stopped.store(true, Ordering::Release);
        }
        *ffmpeg = Some(self.run_from(n)?);
        Ok(true)
    }

    /// Starts a run writing this plan's files from keyframe `n` on.
    fn run_from(self: &Arc<Self>, n: usize) -> io::Result<(Arc<FfmpegRun>, JoinHandle<()>)> {
        let start = match &self.fallback {
            Some(fallback) if self.fell_back() => &fallback.start,
            _ => &self.start,
        };
        let ffmpeg = start(&self.dir, self.timeline.keyframe(n))?;
        let run = Arc::new(FfmpegRun {
            from: n,
            written_to: AtomicUsize::new(n),
            stopped: AtomicBool::new(false),
            joined: tokio::sync::Mutex::default(),
        });
        let task = tokio::spawn(Arc::clone(self).watch(ffmpeg, Arc::clone(&run)));
        Ok((run, task))
    }

    /// Waits for `ffmpeg`, doing the run `run`, to end, or stops it once it has written far
    /// enough ahead, then logs why if it failed and marks it stopped.
    async fn watch(self: Arc<Self>, ffmpeg: Run, run: Arc<FfmpegRun>) {
        let worth_falling_back = tokio::select! {
            outcome = ffmpeg => self.log_failure(outcome),
            // Dropping the run stops it.
            () = self.written_far_ahead(&run) => false,
        };
        // What it wrote since last looked, for its fallback, `will_write` and `init`, which reads
        // it once marked stopped, so before.
        self.note_written(&run).await;
        if worth_falling_back {
            self.fall_back(&run).await;
        }
        run.stopped.store(true, Ordering::Release);
    }

    /// Once `run`, the plan's current run of ffmpeg, failed on hardware, starts its fallback from
    /// the first file it hadn't written, and tells its owner so. Only the first failure does.
    async fn fall_back(self: &Arc<Self>, run: &Arc<FfmpegRun>) {
        let Some(fallback) = &self.fallback else {
            return;
        };
        // Before waiting for the lock, so a seek that restarts ffmpeg meanwhile, aborting this
        // task, restarts it in software.
        if self.fell_back.swap(true, Ordering::AcqRel) {
            return;
        }
        tracing::warn!(
            plan = self.id,
            "hardware acceleration failed partway, so the rest Transcodes in software"
        );
        (fallback.on_fall_back)();
        // Under the lock a restart takes, so it doesn't race a seek's.
        let mut ffmpeg = self.ffmpeg.lock().await;
        let current = (ffmpeg.as_ref()).is_some_and(|(current, _)| Arc::ptr_eq(current, run));
        if !current || self.closed.load(Ordering::Acquire) {
            return;
        }
        let from = run.written_to.load(Ordering::Relaxed);
        if from < self.timeline.keyframes.len() {
            run.stopped.store(true, Ordering::Release);
            match self.run_from(from) {
                // Replaces this task's own handle, which leaves it to finish.
                Ok(software) => *ffmpeg = Some(software),
                Err(err) => tracing::warn!(plan = self.id, "could not start ffmpeg again: {err}"),
            }
        }
    }

    /// Logs why ffmpeg failed, if it did, and says whether it failed in a way a run in software
    /// might get past.
    fn log_failure(&self, outcome: Outcome) -> bool {
        match outcome {
            Outcome::Finished => false,
            Outcome::Failed { status, stderr } => {
                tracing::warn!(
                    plan = self.id,
                    "ffmpeg failed ({status}): {}",
                    stderr.trim()
                );
                if stderr.contains(DISK_FULL) {
                    self.warn_disk_full();
                    // Software would write the same files to the same full folder.
                    return false;
                }
                true
            }
            Outcome::Unwaited(err) => {
                tracing::warn!(plan = self.id, "could not wait for ffmpeg: {err}");
                false
            }
        }
    }

    /// Warns that the segment folder is full, with what tells an Admin it's too small, such as a
    /// tmpfs holding the files kept behind playback.
    fn warn_disk_full(&self) {
        let folder = self.dir.parent().unwrap_or(&self.dir);
        let free = free_bytes(folder).map_or_else(
            || "an unknown amount".to_owned(),
            |bytes| format!("{} MiB", bytes / (1024 * 1024)),
        );
        let kept_behind_minutes = self.keep_behind_ms.load(Ordering::Relaxed) / 60_000;
        tracing::warn!(
            plan = self.id,
            "the segment folder, {}, is full, with {free} free while plans keep \
             {kept_behind_minutes} minutes behind playback: give it more room, or keep less \
             behind in the Playback settings",
            folder.display()
        );
    }

    /// Returns once `run` has written [`RUN_AHEAD_MS`] past the start of the last segment asked
    /// for, or of where it started if later, keeping track of how far it has written meanwhile.
    async fn written_far_ahead(&self, run: &FfmpegRun) {
        loop {
            tokio::time::sleep(POLL_EVERY).await;
            let written_to = self.note_written(run).await;
            let from_ms = self.run_ahead_from_ms(run);
            if self
                .timeline
                .keyframe_ms(written_to)
                .saturating_sub(from_ms)
                >= RUN_AHEAD_MS
            {
                return;
            }
        }
    }

    /// Where `run`'s [`RUN_AHEAD_MS`] counts from: the start of the last segment asked for, or
    /// where it started if later.
    // An earlier segment asked for just after a seek mustn't stop, or delete what's written by,
    // the run it started. `delete_unkept` relies on `written_far_ahead` using the same bound.
    fn run_ahead_from_ms(&self, run: &FfmpegRun) -> i64 {
        let asked = (self.timeline).segment_start_ms(self.last_asked.load(Ordering::Relaxed));
        asked.max(self.timeline.keyframe_ms(run.from))
    }

    /// Joins the files `run` wrote into listed keyframes' files, then notes in `run` how far it
    /// has written, and returns it. Walks on from where it last looked, not from where it started,
    /// whose files may be gone since.
    // A file deleted before it's looked at stops the walk there, so `delete_unkept` keeps those,
    // but for those past where `written_far_ahead` stops the run anyway.
    async fn note_written(&self, run: &FfmpegRun) -> usize {
        self.join_written(run).await;
        let mut written_to = run.written_to.load(Ordering::Relaxed);
        while written_to < self.timeline.keyframes.len()
            && tokio::fs::try_exists(self.keyframe_path(written_to))
                .await
                .unwrap_or_default()
        {
            written_to += 1;
        }
        // The most, as the tick may walk alongside `watch` and delete what the slower walk is on.
        run.written_to
            .fetch_max(written_to, Ordering::Relaxed)
            .max(written_to)
    }

    /// Joins the files `run` wrote whole into one per listed keyframe, as its playlist lists them,
    /// and deletes them, or logs why it couldn't, once.
    async fn join_written(&self, run: &FfmpegRun) {
        let mut joined = run.joined.lock().await;
        let playlist = match tokio::fs::read_to_string(self.dir.join(hls_playlist(run.from))).await
        {
            Ok(playlist) => playlist,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return,
            Err(err) => {
                tracing::warn!(plan = self.id, "could not read ffmpeg's playlist: {err}");
                return;
            }
        };
        let mut listed = 0;
        for (n, files) in self.timeline.written_by_keyframe(run.from, &playlist) {
            listed += files.len();
            if listed <= joined.files {
                continue;
            }
            match self.join(n, &files).await {
                Ok(true) => joined.files = listed,
                Ok(false) => return,
                Err(err) => {
                    self.warn_join_failed(&mut joined, n, &err);
                    return;
                }
            }
        }
    }

    /// Warns that keyframe `n`'s files couldn't be joined, as the segment folder being full, which
    /// a copy needs room for, or otherwise, unless `joined` says it did since its run started.
    fn warn_join_failed(&self, joined: &mut Joined, n: usize, err: &io::Error) {
        if std::mem::replace(&mut joined.failed, true) {
            return;
        }
        if err.kind() == io::ErrorKind::StorageFull {
            self.warn_disk_full();
        } else {
            tracing::warn!(plan = self.id, "could not join keyframe {n}'s files: {err}");
        }
    }

    /// Joins `files`, which ffmpeg wrote whole, into keyframe `n`'s file, unless it's there, and
    /// deletes them. `false`, touching none, while one isn't there.
    // One isn't when the playlist is a stopped run's that started at the same keyframe, until the
    // run started there again writes its own.
    async fn join(&self, n: usize, files: &[&str]) -> io::Result<bool> {
        let path = self.keyframe_path(n);
        if !tokio::fs::try_exists(&path).await? {
            let outcome = match files {
                [file] => tokio::fs::rename(self.dir.join(file), &path).await,
                _ => self.concatenate(files, &path).await,
            };
            match outcome {
                Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(false),
                outcome => outcome?,
            }
        }
        for file in files {
            match tokio::fs::remove_file(self.dir.join(file)).await {
                Err(err) if err.kind() != io::ErrorKind::NotFound => return Err(err),
                _ => {}
            }
        }
        Ok(true)
    }

    /// Writes `files` one after another to `path`, which appears only once it's whole.
    async fn concatenate(&self, files: &[&str], path: &Path) -> io::Result<()> {
        let mut bytes = vec![];
        for file in files {
            bytes.extend(tokio::fs::read(self.dir.join(file)).await?);
        }
        let whole = path.with_extension("m4s.tmp");
        tokio::fs::write(&whole, bytes).await?;
        tokio::fs::rename(whole, path).await
    }
}

#[cfg(unix)]
fn free_bytes(folder: &Path) -> Option<u64> {
    let stat = rustix::fs::statvfs(folder).ok()?;
    Some(stat.f_bavail.saturating_mul(stat.f_frsize))
}

// ponytail: a Windows Server's warning gives no free space; ask GetDiskFreeSpaceExW once one runs.
#[cfg(not(unix))]
const fn free_bytes(_folder: &Path) -> Option<u64> {
    None
}

#[cfg(test)]
#[path = "run_tests.rs"]
mod tests;
