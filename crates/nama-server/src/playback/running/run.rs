use std::{
    io,
    path::{Path, PathBuf},
    process::Output,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

use nama_proto::nama::v1::{Playback, PlaybackChanged, PlaybackReason};
use tokio::{process::Child, task::JoinHandle};

use super::{
    Method, POLL_EVERY, REMUX_WAIT_AHEAD_MS, RUN_AHEAD_MS,
    timeline::{SEGMENT_MS, Timeline, keyframe_number},
};
use crate::{
    events::{EventBus, event},
    ffmpeg::{Keyframe, hls_init, hls_init_from},
    server_settings::PlaybackSettings,
};

/// How ffmpeg's stderr says a write failed with ENOSPC. ffmpeg, not the Server, writes a plan's
/// files, so its words are all the Server sees of it.
const DISK_FULL: &str = "No space left on device";

/// Starts ffmpeg writing a plan's files, from a keyframe on, to the folder it's given.
pub(super) type Run = dyn Fn(&Path, Keyframe) -> io::Result<Child> + Send + Sync;

/// What starts ffmpeg writing a plan's files: `run`, or once a run on hardware fails, `fallback`.
pub(super) struct Runs {
    pub(super) run: Box<Run>,
    pub(super) fallback: Option<FallbackRun>,
    /// Whether `run` works the CPU hard, which the default Transcode cap counts.
    pub(super) cpu_bound: bool,
}

/// What a Transcode on hardware runs instead once a run there fails.
pub(super) struct FallbackRun {
    pub(super) run: Box<Run>,
    /// Its plan's reasons from then on, which say why.
    pub(super) reasons: Vec<PlaybackReason>,
}

/// A plan's HLS files, and the ffmpeg writing them.
pub(super) struct Running {
    pub(super) id: u64,
    /// How far ahead a request waits for ffmpeg, and so the Transcode cap leaves Remuxes out.
    pub(super) method: Method,
    pub(super) timeline: Timeline,
    pub(super) dir: PathBuf,
    pub(super) run: Box<Run>,
    /// The last segment asked for, by number.
    pub(super) last_asked: AtomicUsize,
    /// Whether the plan was stopped, so no ffmpeg is started again for requests still waiting.
    pub(super) closed: AtomicBool,
    /// For a Transcode on hardware, what it runs once a run there fails.
    pub(super) fallback: Option<FallbackRun>,
    /// Whether a run on hardware failed, so `fallback` runs from then on.
    pub(super) fell_back: AtomicBool,
    /// Whether it started working the CPU hard, which the default Transcode cap counts.
    pub(super) started_cpu_bound: bool,
    /// Tells Observers how the plan plays now, once it fell back.
    pub(super) events: EventBus,
    /// As it started, for telling Observers how it plays once it fell back.
    pub(super) playback: Playback,
    /// For the time kept behind playback, which a warning that the segment folder is full names.
    pub(super) settings: PlaybackSettings,
    /// The run of ffmpeg writing its files, and its task, which stops ffmpeg when aborted.
    pub(super) ffmpeg: tokio::sync::Mutex<Option<(Arc<FfmpegRun>, JoinHandle<()>)>>,
}

/// One run of ffmpeg, from one keyframe on.
pub(super) struct FfmpegRun {
    /// The keyframe it started at, by number.
    from: usize,
    /// The first keyframe from `from` on it hadn't written, as last looked. A file behind it that's
    /// gone since doesn't move it back.
    written_to: AtomicUsize,
    /// Whether ffmpeg has stopped, so a file not written yet never will be by this run.
    stopped: AtomicBool,
}

impl Running {
    /// Whether it works the CPU hard now: it started so, or fell back to software.
    pub(super) fn cpu_bound(&self) -> bool {
        self.started_cpu_bound || self.fell_back.load(Ordering::Acquire)
    }

    /// Stops the plan `id`'s ffmpeg and deletes its files.
    pub(super) async fn stop(&self, id: u64) {
        // Under the lock a restart takes, so none starts ffmpeg after this.
        let mut ffmpeg = self.ffmpeg.lock().await;
        self.closed.store(true, Ordering::Release);
        if let Some((run, task)) = ffmpeg.take() {
            task.abort();
            // Awaited so ffmpeg is dropped, which kills it, before its folder is deleted.
            let _: Result<(), _> = task.await;
            run.stopped.store(true, Ordering::Release);
        }
        drop(ffmpeg);
        if let Err(err) = tokio::fs::remove_dir_all(&self.dir).await {
            tracing::warn!(plan = id, "could not delete {}: {err}", self.dir.display());
        }
    }

    /// Deletes the files of keyframes ending more than `keep_behind_ms` before the start of the
    /// last segment asked for, or starting [`RUN_AHEAD_MS`] or more past it, such as those a seek
    /// back left (ADR 0062), and the init files of every run but the current one. Those behind
    /// that the current run may still be writing are kept, and while it runs, those less than
    /// [`RUN_AHEAD_MS`] past where it started, since a seek ahead started it there.
    pub(super) async fn delete_unkept(&self, keep_behind_ms: i64) -> io::Result<()> {
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
        // A running run writes past where it started even once an earlier segment is asked for.
        let ahead_from_ms = if running {
            asked.max(self.timeline.keyframe_ms(run.from))
        } else {
            asked
        };
        let keep_from_ms = asked - keep_behind_ms;
        let keep_to_ms = ahead_from_ms + RUN_AHEAD_MS;
        let mut deleted_unwalked = false;
        let mut entries = tokio::fs::read_dir(&self.dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let unkept = match keyframe_number(&name) {
                Some(n) => {
                    let far_ahead = self.timeline.keyframe_ms(n) >= keep_to_ms;
                    deleted_unwalked |= far_ahead && n >= still_writing_from;
                    far_ahead
                        || (n < still_writing_from
                            && self.timeline.keyframe_ms(n + 1) < keep_from_ms)
                }
                None => hls_init_from(&name).is_some_and(|from| from != run.from),
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
        // only if it has written the file there since. Its walk would then wait at the deleted one
        // for good, so the run is stopped, and the request for it starts ffmpeg again.
        if deleted_unwalked && tokio::fs::try_exists(self.keyframe_path(still_writing_from)).await?
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

    /// How it plays now: as it started, with its fallback's reasons if it fell back.
    pub(super) fn playback_now(&self) -> Playback {
        let mut playback = self.playback.clone();
        if let Some(FallbackRun { reasons, .. }) =
            (self.fallback.as_ref()).filter(|_| self.fell_back.load(Ordering::Acquire))
        {
            playback.reasons.clone_from(reasons);
        }
        playback
    }

    /// Starts ffmpeg writing this plan's files from keyframe `n` on.
    pub(super) fn run_from(
        self: &Arc<Self>,
        n: usize,
    ) -> io::Result<(Arc<FfmpegRun>, JoinHandle<()>)> {
        let start = match &self.fallback {
            Some(fallback) if self.fell_back.load(Ordering::Acquire) => &fallback.run,
            _ => &self.run,
        };
        let ffmpeg = start(&self.dir, self.timeline.keyframe(n))?;
        let run = Arc::new(FfmpegRun {
            from: n,
            written_to: AtomicUsize::new(n),
            stopped: AtomicBool::new(false),
        });
        let task = tokio::spawn(Arc::clone(self).watch(ffmpeg, Arc::clone(&run)));
        Ok((run, task))
    }

    /// Waits for `ffmpeg`, doing the run `run`, to stop, or stops it once it has written far
    /// enough ahead, then logs why if it failed and marks it stopped.
    async fn watch(self: Arc<Self>, ffmpeg: Child, run: Arc<FfmpegRun>) {
        let failed = tokio::select! {
            output = ffmpeg.wait_with_output() => self.log_failure(output),
            // Dropping the child stops ffmpeg.
            () = self.written_far_ahead(&run) => false,
        };
        // What it wrote since last looked, for its fallback, `will_write` and `init`, which reads
        // it once marked stopped, so before.
        self.note_written(&run).await;
        if failed {
            self.fall_back(&run).await;
        }
        run.stopped.store(true, Ordering::Release);
    }

    /// Once `run`, the plan's current run of ffmpeg, failed on hardware, starts its fallback from
    /// the first file it hadn't written, and tells Observers so. Only the first failure does.
    async fn fall_back(self: &Arc<Self>, run: &Arc<FfmpegRun>) {
        if self.fallback.is_none() {
            return;
        }
        // Before waiting for the lock, so a seek that restarts ffmpeg meanwhile, aborting this
        // task, restarts it in software.
        if self.fell_back.swap(true, Ordering::AcqRel) {
            return;
        }
        tracing::warn!(
            plan = self.id,
            "hardware acceleration failed partway, so the rest Transcodes in software"
        );
        let changed = PlaybackChanged {
            playback: self.playback_now().into(),
            ..Default::default()
        };
        self.events.publish(vec![event(changed)]);
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
                Ok(fallback) => *ffmpeg = Some(fallback),
                Err(err) => tracing::warn!(plan = self.id, "could not start ffmpeg again: {err}"),
            }
        }
    }

    /// Logs why ffmpeg failed, if it did, and says whether it did.
    fn log_failure(&self, output: io::Result<Output>) -> bool {
        match output {
            Ok(Output { status, .. }) if status.success() => false,
            Ok(Output { status, stderr, .. }) => {
                let stderr = String::from_utf8_lossy(&stderr);
                tracing::warn!(
                    plan = self.id,
                    "ffmpeg failed ({status}): {}",
                    stderr.trim()
                );
                if stderr.contains(DISK_FULL) {
                    self.warn_disk_full();
                }
                true
            }
            Err(err) => {
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
        let kept_behind_minutes = self.settings.keep_behind_ms() / 60_000;
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
            let asked = (self.timeline).segment_start_ms(self.last_asked.load(Ordering::Relaxed));
            // An earlier segment asked for just after a seek mustn't stop the run it started.
            let asked = asked.max(self.timeline.keyframe_ms(run.from));
            if self.timeline.keyframe_ms(written_to).saturating_sub(asked) >= RUN_AHEAD_MS {
                return;
            }
        }
    }

    /// Notes in `run` how far it has written, and returns it. Walks on from where it last looked,
    /// not from where it started, whose files may be gone since.
    // A file deleted before it's looked at stops the walk there, so `delete_unkept` keeps those,
    // but for those past where `written_far_ahead` stops the run anyway.
    async fn note_written(&self, run: &FfmpegRun) -> usize {
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
