//! Probing: runs ffprobe on stored files and stores what each holds as its Tracks (see
//! [`super::tracks`]), and decides what a failure to probe one means for trying it again. It also
//! lists a Version's keyframes when first asked for them, and keeps them, or that listing them
//! failed, for its file as it is ([`Probing::keyframes`]).
//!
//! After a Library's files change, those without current Tracks, never probed or changed or read
//! by an older probe version since, are probed in the background, a few at a time
//! ([`Probing::files_changed`]). A file ffprobe couldn't read is skipped the same way until it
//! changes, as probing it again would fail again. One that failed for a passing reason, such as a
//! timeout, is tried again on the next few passes first. A Version can also be probed right away
//! ([`Probing::current`]), which waits for a background probe of it rather than probe it twice.

mod ffprobe;

use std::{
    collections::{HashMap, hash_map::Entry},
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
};

use futures_util::StreamExt;
use nama_proto::nama::v1::ProbingFinished;
use tokio::sync::watch;
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use tracing::Instrument as _;

use super::{
    subtitles,
    tracks::{Keyframes, LastingReason, PassingReason, Probe, ProbeFailure, Tracks},
};
use crate::{
    db::Database,
    diagnostics,
    ffmpeg::Ffmpeg,
    library::{Contents, Stored},
};

/// How many files background passes probe at once, across all Libraries. Each probe reads only a
/// file's headers, but from a disk, or a network share, that playback is reading from too.
const PROBES_AT_ONCE: usize = 3;

/// Why probing a file failed.
#[derive(Debug, thiserror::Error)]
pub(crate) enum ProbeError {
    #[error("the Server has no ffprobe")]
    Unavailable,
    #[error("ffprobe couldn't be run: {0}")]
    Run(#[from] std::io::Error),
    #[error("ffprobe took too long")]
    TimedOut(#[from] tokio::time::error::Elapsed),
    #[error("ffprobe failed: {0}")]
    Failed(String),
    #[error("ffprobe's output couldn't be read: {0}")]
    Output(#[from] serde_json::Error),
    /// Kept from an earlier listing of the keyframes, which isn't tried again until the file
    /// changes or [`super::tracks::PROBE_FAILURE_VERSION`] is bumped (#850).
    #[error("listing the keyframes failed or timed out before, and the file hasn't changed since")]
    UnlistedBefore,
}

/// Why probing a Version right away failed.
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    /// ffprobe couldn't read it, or the Server has no ffprobe ([`ProbeError::Unavailable`]).
    /// Shared, as whoever waited for the same probe gets it too.
    #[error(transparent)]
    Probe(Arc<ProbeError>),
    #[error("there is no Version {0}")]
    NoSuchVersion(i64),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// Why probing a claimed file failed, and what that means for trying it again.
#[derive(Debug)]
enum Unprobed {
    Probe(Arc<ProbeError>, ProbeFailure),
    Database(sqlx::Error),
}

impl From<sqlx::Error> for Unprobed {
    fn from(err: sqlx::Error) -> Self {
        Self::Database(err)
    }
}

impl From<Unprobed> for Error {
    fn from(unprobed: Unprobed) -> Self {
        match unprobed {
            Unprobed::Probe(err, _) => Self::Probe(err),
            Unprobed::Database(err) => Self::Database(err),
        }
    }
}

/// What probing a file found, or why it couldn't, as handed to whoever waited for it.
type Probed = Result<Probe, Arc<ProbeError>>;

/// A file's keyframes, or why they couldn't be listed, as handed to whoever waited for them.
type Listed = Result<Vec<i64>, Arc<ProbeError>>;

/// The Versions some task holds a [`Claim`] on, and where it sends what it found.
type Claims<T> = Mutex<HashMap<i64, watch::Receiver<Option<T>>>>;

/// Probes stored files, in the background after their Library changes or right away for one
/// Version.
#[derive(Debug, Clone)]
pub(crate) struct Probing {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    contents: Contents,
    tracks: Tracks,
    ffmpeg: Arc<Ffmpeg>,
    subtitles: subtitles::Cache,
    /// Background passes, so shutting down can wait for them.
    tasks: TaskTracker,
    /// Cancelled when the Server shuts down.
    cancel: CancellationToken,
    /// A permit per background probe running, [`PROBES_AT_ONCE`] in all.
    running: tokio::sync::Semaphore,
    /// The Versions being probed, each by one [`Claim`] holder, and where it sends what it found,
    /// so a later pass over their Library leaves them to it and [`Probing::current`] waits for
    /// its result, rather than probe them twice.
    claimed: Claims<Probed>,
    /// The Versions whose keyframes are being listed, as `claimed` is for probing.
    listing: Claims<Listed>,
}

impl Probing {
    /// Probes with `ffmpeg`'s ffprobe, or not at all without one, and throws away what
    /// `subtitles` caches for files gone or changed.
    pub(crate) fn new(db: Arc<Database>, ffmpeg: Arc<Ffmpeg>, subtitles: subtitles::Cache) -> Self {
        Self {
            inner: Arc::new(Inner {
                tracks: Tracks::new(Arc::clone(&db)),
                contents: Contents::new(db),
                ffmpeg,
                subtitles,
                tasks: TaskTracker::new(),
                cancel: CancellationToken::new(),
                running: tokio::sync::Semaphore::new(PROBES_AT_ONCE),
                claimed: Mutex::default(),
                listing: Mutex::default(),
            }),
        }
    }

    /// The current Tracks of the Version `version`, probing it now if they aren't current, without
    /// waiting for a background pass to reach it. If something is probing it already, waits for
    /// that and returns what it found instead, or probes it itself if that gave up without finding
    /// anything.
    pub(crate) async fn current(&self, version: i64) -> Result<Probe, Error> {
        let (path, file) = (self.inner.contents.version_file(version).await?)
            .ok_or(Error::NoSuchVersion(version))?;
        // ponytail: not held to PROBES_AT_ONCE, so a play press never queues; limit it apart if
        // many at once swamp the disk.
        loop {
            let mut prober = match try_claim(&self.inner.claimed, file) {
                Ok(claim) => return Ok(self.probe_claimed(&path, claim).await?),
                Err(prober) => prober,
            };
            // An error means the prober gave up without a result, as when it was cancelled
            // midway, so it's probed here instead.
            if let Ok(found) = prober.wait_for(Option::is_some).await
                && let Some(found) = Option::clone(&found)
            {
                return found.map_err(Error::Probe);
            }
        }
    }

    /// When each keyframe of the Version `version`'s first video falls, in milliseconds from its
    /// start, in order: as kept for its file as it is now, or else listed, which reads the whole
    /// file, and kept. If something is listing them already, waits for that and returns what it
    /// found instead, or lists them itself if that gave up without finding anything. They're kept
    /// only while the Version's Tracks were read from its file as it is now.
    pub(super) async fn keyframes(&self, version: i64) -> Result<Vec<i64>, Error> {
        let (path, file) = (self.inner.contents.version_file(version).await?)
            .ok_or(Error::NoSuchVersion(version))?;
        // ponytail: not held to PROBES_AT_ONCE, as `current` isn't; limit it apart if many
        // whole-file reads at once swamp the disk.
        loop {
            let mut lister = match try_claim(&self.inner.listing, file) {
                Ok(claim) => return self.list_claimed(&path, claim).await,
                Err(lister) => lister,
            };
            // An error means the lister sent nothing, having found them kept or given up, so
            // they're read or listed here instead.
            if let Ok(found) = lister.wait_for(Option::is_some).await
                && let Some(found) = Option::clone(&found)
            {
                return found.map_err(Error::Probe);
            }
        }
    }

    /// Probes the Library `library`'s files without current Tracks in the background, unless the
    /// Server has no ffprobe or is shutting down. Calls `finished` with what the pass found once
    /// Probing has caught up, or not at all if the pass is cancelled or fails.
    pub(crate) fn files_changed(
        &self,
        library: i64,
        finished: impl FnOnce(ProbingFinished) + Send + 'static,
    ) {
        // Closing the tracker doesn't refuse spawns, so a pass after shutdown is refused here.
        if self.inner.cancel.is_cancelled() {
            return;
        }
        if !self.inner.ffmpeg.can_probe() {
            finished(ProbingFinished {
                library_id: library,
                ..Default::default()
            });
            return;
        }
        let probing = self.clone();
        // In the trace of the scan that changed the files, so their probes are in it too.
        let pass_span = diagnostics::following_current(tracing::info_span!(
            parent: None,
            "probe pass",
            library.id = library
        ));
        self.inner.tasks.spawn(
            async move {
                let pass = probing.probe_stale(library);
                match probing.inner.cancel.run_until_cancelled(pass).await {
                    Some(Ok(probed)) => finished(probed),
                    Some(Err(err)) => {
                        tracing::error!(library, "probing a Library's files failed: {err}");
                    }
                    None => {}
                }
            }
            .instrument(pass_span),
        );
    }

    /// Cancels the probes running, waits for them to stop, and starts no more passes.
    pub(crate) async fn shutdown(&self) {
        self.inner.cancel.cancel();
        self.inner.tasks.close();
        self.inner.tasks.wait().await;
    }

    /// Throws away the subtitles cached for files gone, such as a deleted Library's, or changed
    /// since they were cached, or logs why it couldn't.
    pub(crate) async fn clean_subtitles(&self) {
        // ponytail: reads every stored file after each scan; clean only the Library scanned once
        // Libraries get big.
        let cleaned = match self.inner.contents.every_stored().await {
            Ok(stored) => self.inner.subtitles.keep_only(&stored).await,
            Err(err) => {
                tracing::error!("could not read which files to keep subtitles for: {err}");
                return;
            }
        };
        if let Err(err) = cleaned {
            tracing::error!("could not throw away subtitles of files gone or changed: {err}");
        }
    }

    /// Probes the Library `library`'s files without current Tracks and stores what each holds,
    /// and waits for those another prober holds. A file that can't be probed is logged, and for a
    /// lasting reason skipped by later passes until it changes. Returns how many files were
    /// probed and how many ffprobe couldn't read.
    ///
    /// # Errors
    ///
    /// If the database can't be read, or what a file holds can't be stored, once every other file
    /// is probed: the Library hasn't caught up.
    async fn probe_stale(&self, library: i64) -> Result<ProbingFinished, sqlx::Error> {
        let stored = self.inner.contents.stored(library).await?;
        let files: Vec<Stored> = stored.values().copied().collect();
        let current = self.inner.tracks.current(&files).await?;
        let stale = stored
            .into_iter()
            .filter(|(_, file)| !current.contains(&file.id));
        let ffprobe_runs = AtomicBool::new(true);
        let mut outcomes = futures_util::stream::iter(stale)
            .map(|(path, file)| self.probe(path, file, &ffprobe_runs))
            .buffer_unordered(PROBES_AT_ONCE);
        let mut probed = ProbingFinished {
            library_id: library,
            ..Default::default()
        };
        let mut unstored = None;
        while let Some(outcome) = outcomes.next().await {
            match outcome {
                Ok(Some(Ok(_))) => probed.probed += 1,
                // ffprobe didn't run, so the file is left for the next scan.
                Ok(Some(Err(err)))
                    if matches!(*err, ProbeError::Unavailable | ProbeError::Run(_)) => {}
                Ok(Some(Err(_))) => probed.failed += 1,
                Ok(None) => {}
                Err(err) => unstored = Some(err),
            }
        }
        unstored.map_or(Ok(probed), Err)
    }

    /// Probes the file at `path`, stored as `file`, and stores what it holds, or logs why not and
    /// forgets what it held before. If something else is probing it, waits for what that found, or
    /// probes it after all if that gave up without finding anything. Once a failure stops the
    /// pass, clears `ffprobe_runs` and leaves this and the pass's other files for the next.
    /// Returns what was found, or `None` if nothing was.
    ///
    /// # Errors
    ///
    /// If the database can't be read or what the file holds can't be stored, which is logged.
    async fn probe(
        &self,
        path: String,
        file: Stored,
        ffprobe_runs: &AtomicBool,
    ) -> Result<Option<Probed>, sqlx::Error> {
        loop {
            // The semaphore is never closed.
            let Ok(running) = self.inner.running.acquire().await else {
                return Ok(None);
            };
            if !ffprobe_runs.load(Ordering::Relaxed) {
                return Ok(None);
            }
            // Claimed only now, so `current` doesn't wait for a file still queued here.
            let mut prober = match try_claim(&self.inner.claimed, file) {
                Ok(claim) => return self.probe_logged(&path, claim, ffprobe_runs).await,
                Err(prober) => prober,
            };
            drop(running);
            // An error means the prober gave up without a result, as when it was cancelled
            // midway, so it's probed here instead.
            if let Ok(found) = prober.wait_for(Option::is_some).await
                && let Some(found) = Option::clone(&found)
            {
                return Ok(Some(found));
            }
        }
    }

    /// Probes the file at `path` as [`Self::probe`] does, once `claim` is held for it.
    async fn probe_logged(
        &self,
        path: &str,
        claim: Claim<'_, Probed>,
        ffprobe_runs: &AtomicBool,
    ) -> Result<Option<Probed>, sqlx::Error> {
        match self.probe_claimed(path, claim).await {
            Ok(probe) => Ok(Some(Ok(probe))),
            Err(Unprobed::Database(err)) => {
                tracing::error!("could not read or store what {path} holds: {err}");
                Err(err)
            }
            Err(Unprobed::Probe(err, ProbeFailure::StopsPass)) => {
                // It would fail the same way on every other file, logging each.
                if ffprobe_runs.swap(false, Ordering::Relaxed) {
                    tracing::error!("stopped probing a Library's files until its next scan: {err}");
                }
                Ok(Some(Err(err)))
            }
            Err(Unprobed::Probe(
                err,
                ProbeFailure::Retried | ProbeFailure::Lasting(_) | ProbeFailure::Passing(_),
            )) => {
                tracing::warn!("could not probe {path}: {err}");
                Ok(Some(Err(err)))
            }
        }
    }

    /// Probes the file at `path`, stored as the file `claim` is for, like
    /// [`Self::probe_and_store`], and sends what it found to whoever waited for it.
    async fn probe_claimed(&self, path: &str, claim: Claim<'_, Probed>) -> Result<Probe, Unprobed> {
        let probed = self.probe_and_store(path, claim.file).await;
        let found = match &probed {
            Ok(probe) => Ok(probe.clone()),
            Err(Unprobed::Probe(err, _)) => Err(Arc::clone(err)),
            // Nothing was found, so a waiter probes it itself.
            Err(Unprobed::Database(_)) => return probed,
        };
        claim.found.send_replace(Some(found));
        probed
    }

    /// Probes the file at `path`, stored as `file`, and stores and returns what it holds, unless
    /// its Tracks became current since whoever claimed it found them stale. If ffprobe can't read
    /// it, stores that and why, and forgets what it held before.
    #[tracing::instrument(name = "probe", skip_all, fields(file.id = file.id))]
    async fn probe_and_store(&self, path: &str, file: Stored) -> Result<Probe, Unprobed> {
        if self.inner.tracks.current(&[file]).await?.contains(&file.id)
            && let Some(probe) = self.inner.tracks.of(&[file]).await?.remove(&file.id)
        {
            return Ok(probe);
        }
        match ffprobe::probe(&self.inner.ffmpeg, path).await {
            Ok(probe) => {
                self.inner.tracks.store(file, probe.clone()).await?;
                Ok(probe)
            }
            Err(err) => {
                let failure = failure(&err, path).await;
                self.inner.tracks.store_failure(file, failure).await?;
                Err(Unprobed::Probe(Arc::new(err), failure))
            }
        }
    }

    /// The keyframes of the file at `path`, stored as the file `claim` is for, as kept, or else
    /// listed, sent to whoever waited for them, and kept. A listing that failed for a reason
    /// that says something about the file is kept too, and returned as
    /// [`ProbeError::UnlistedBefore`] until the file changes or the probe failure version is
    /// bumped. Unlike a probe, a timeout isn't tried again: each try reads the whole file for up
    /// to 10 minutes while a Play waits (#850).
    async fn list_claimed(&self, path: &str, claim: Claim<'_, Listed>) -> Result<Vec<i64>, Error> {
        match self.inner.tracks.keyframes(claim.file).await? {
            Some(Keyframes::Listed(keyframes)) => return Ok(keyframes),
            Some(Keyframes::Unlisted) => {
                return Err(Error::Probe(Arc::new(ProbeError::UnlistedBefore)));
            }
            None => {}
        }
        let listed = ffprobe::keyframes(&self.inner.ffmpeg, path)
            .await
            .map_err(Arc::new);
        claim.found.send_replace(Some(listed.clone()));
        let kept = match &listed {
            Ok(keyframes) => Some(Keyframes::Listed(keyframes.clone())),
            Err(err) => matches!(
                failure(err, path).await,
                ProbeFailure::Lasting(_) | ProbeFailure::Passing(_)
            )
            .then_some(Keyframes::Unlisted),
        };
        if let Some(kept) = kept {
            self.inner.tracks.store_keyframes(claim.file, &kept).await?;
        }
        listed.map_err(Error::Probe)
    }
}

/// What failing to probe the file at `path` with `err` means for trying it again.
async fn failure(err: &ProbeError, path: &str) -> ProbeFailure {
    match err {
        // ffprobe never reached the file, so counting it against the file would skip a whole
        // Library after one bad pass, such as during a package upgrade.
        ProbeError::Run(_) => ProbeFailure::StopsPass,
        // Nothing was read, so what's stored isn't known to be wrong, and a Server without
        // ffprobe keeps it, as it never probes in the background.
        ProbeError::Unavailable => ProbeFailure::Retried,
        // ffprobe fails fast on a file it can't open, as on a share that dropped or an NFS handle
        // gone stale since the scan, which may read fine once it's back unchanged.
        ProbeError::Failed(_) if !readable(path).await => ProbeFailure::Retried,
        // ffprobe read the file and refused it, or did before and it hasn't changed since, or
        // said something it can't parse: it will again.
        ProbeError::Failed(_) | ProbeError::UnlistedBefore => {
            ProbeFailure::Lasting(LastingReason::Failed)
        }
        ProbeError::Output(_) => ProbeFailure::Lasting(LastingReason::UnreadableOutput),
        // A slow or unmounted share, or a busy disk, may not be slow next time.
        ProbeError::TimedOut(_) => ProbeFailure::Passing(PassingReason::TimedOut),
    }
}

/// Whether the file at `path` can be opened and read from now.
async fn readable(path: &str) -> bool {
    use tokio::io::AsyncReadExt;

    match tokio::fs::File::open(path).await {
        Ok(mut file) => file.read(&mut [0; 1]).await.is_ok(),
        Err(_) => false,
    }
}

/// The right to probe one stored file, or list its keyframes, held by one task at a time and
/// given up when dropped, even by a probe cancelled midway.
#[derive(Debug)]
struct Claim<'a, T> {
    claims: &'a Claims<T>,
    file: Stored,
    /// Where what was found goes to whoever waits for it. Dropped with nothing sent, it tells
    /// them to do it themselves.
    found: watch::Sender<Option<T>>,
}

impl<T> Drop for Claim<'_, T> {
    fn drop(&mut self) {
        lock(self.claims).remove(&self.file.id);
    }
}

/// Claims the stored file `file` in `claims`, or if some task holds it, hands back where that will
/// send what it found.
fn try_claim<T>(
    claims: &Claims<T>,
    file: Stored,
) -> Result<Claim<'_, T>, watch::Receiver<Option<T>>> {
    match lock(claims).entry(file.id) {
        Entry::Occupied(holder) => Err(holder.get().clone()),
        Entry::Vacant(unclaimed) => {
            let (found, waiting) = watch::channel(None);
            unclaimed.insert(waiting);
            Ok(Claim {
                claims,
                file,
                found,
            })
        }
    }
}

/// Locks `mutex`, which no code panics while holding.
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
impl Probing {
    /// Waits until no background pass is running.
    pub(crate) async fn idle(&self) {
        self.inner.tasks.close();
        self.inner.tasks.wait().await;
        self.inner.tasks.reopen();
    }
}

#[cfg(test)]
#[path = "probing_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "probing/retry_tests.rs"]
mod retry_tests;

#[cfg(test)]
#[path = "probing/keyframes_tests.rs"]
mod keyframes_tests;
