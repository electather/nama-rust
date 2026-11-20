//! Probing: runs ffprobe on stored files and stores what each holds as its Tracks (see
//! [`super::tracks`]), with a Matroska or MP4 file's keyframes when its index lists them, and
//! decides what a failure to probe one means for trying it again. It lists any other Version's keyframes in the
//! background when first asked for them, and keeps them, or that listing them failed, for its
//! file as it is ([`Probing::keyframes`]).
//!
//! After a Library's files change, those without current Tracks, never probed or changed or read
//! by an older probe version since, are probed in the background, a few at a time
//! ([`Probing::files_changed`]). A file ffprobe couldn't read is skipped the same way until it
//! changes, as probing it again would fail again. One that failed for a passing reason, such as a
//! timeout, is tried again on the next few passes first. A Version can also be probed right away
//! ([`Probing::current`]), which waits for a background probe of it rather than probe it twice.

mod cues;
mod ffprobe;
mod mp4;

use std::{
    collections::{HashMap, HashSet, hash_map::Entry},
    io,
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use futures_util::StreamExt;
use nama_proto::nama::v1::ProbingFinished;
use tokio::sync::{Notify, watch};
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use tracing::Instrument as _;

use super::{
    StoredTracks, subtitles,
    tracks::{
        Keyframes, LastingReason, ListedKeyframes, PassingReason, Probe, ProbeFailure, Tracks,
    },
};
use crate::{
    db::Database,
    diagnostics,
    ffmpeg::{Ffmpeg, RunError},
    images::{DOWNLOADED_SUBTITLES_DIR, ImageStore, PREVIEW_FRAMES_DIR},
    library::{Contents, Stored},
};

/// How many files background passes probe at once, across all Libraries. Each probe reads only a
/// file's headers, but from a disk, or a network share, that playback is reading from too.
const PROBES_AT_ONCE: usize = 3;

/// How many whole files are read at once, to list a Version's keyframes or read its text
/// subtitles out ([`Probing::read_whole_file_in_background`]), from a disk that playback is
/// reading from too.
const WHOLE_FILE_READS_AT_ONCE: usize = 2;

/// How long a plan waits for a Version's keyframes before it Transcodes instead, well short of
/// a reverse proxy's timeout (nginx's is 60 s), which would drop the plan request (#866). Short,
/// as reading a whole big file takes minutes, and a Matroska file's Cues usually spare it (#1122).
const LISTING_WAIT: Duration = Duration::from_secs(5);

/// Why probing a file failed.
#[derive(Debug, thiserror::Error)]
pub(crate) enum ProbeError {
    #[error(transparent)]
    Run(#[from] RunError),
    #[error("ffprobe's output couldn't be read: {0}")]
    Output(#[from] serde_json::Error),
    /// Kept from an earlier listing of the keyframes, which isn't tried again until the file
    /// changes or [`super::tracks::PROBE_FAILURE_VERSION`] is bumped (#850).
    #[error("listing the keyframes failed or timed out before, and the file hasn't changed since")]
    UnlistedBefore,
    /// The keyframes weren't listed within [`LISTING_WAIT`]; the listing carries on and keeps
    /// them for a later plan.
    #[error("the keyframes aren't listed yet")]
    NotListedYet,
}

/// Why probing a Version right away failed.
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    /// ffprobe couldn't read it, or the Server has no ffprobe ([`RunError::Unavailable`]).
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
type Listed = Result<ListedKeyframes, Arc<ProbeError>>;

/// The Versions some task holds a [`Claim`] on, and where it sends what it found. Shared, so a
/// listing's claim can outlive the plan that started it.
type Claims<T> = Arc<Mutex<HashMap<i64, watch::Receiver<Option<T>>>>>;

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
    stored_tracks: StoredTracks,
    ffmpeg: Arc<Ffmpeg>,
    subtitles: subtitles::Cache,
    /// Where downloaded subtitles and Preview frames are kept, from which it deletes those of
    /// Versions gone.
    images: ImageStore,
    /// Told each time a background pass has probed what it found stale.
    passed: Notify,
    /// Background passes and whole-file reads, so shutting down can wait for them.
    tasks: TaskTracker,
    /// Cancelled when the Server shuts down.
    cancel: CancellationToken,
    /// A permit per background probe running, [`PROBES_AT_ONCE`] in all.
    running: tokio::sync::Semaphore,
    /// A permit per whole-file read running, [`WHOLE_FILE_READS_AT_ONCE`] in all.
    whole_file_reads: tokio::sync::Semaphore,
    /// The Versions being probed, each by one [`Claim`] holder, and where it sends what it found,
    /// so a later pass over their Library leaves them to it and [`Probing::current`] waits for
    /// its result, rather than probe them twice.
    claimed: Claims<Probed>,
    /// The Versions whose keyframes are being listed, as `claimed` is for probing.
    listing: Claims<Listed>,
    /// Notified when a background pass has caught up, for Segments. Not `passed`: `notify_one`
    /// wakes only one waiter, so each follower of a pass needs its own.
    passed_for_segments: Arc<Notify>,
}

impl Probing {
    /// Probes with `ffmpeg`'s ffprobe, or not at all without one, throws away what `subtitles`
    /// caches for files gone or changed, and deletes from `images` the subtitles downloaded for
    /// Versions gone.
    pub(crate) fn new(
        db: Arc<Database>,
        ffmpeg: Arc<Ffmpeg>,
        subtitles: subtitles::Cache,
        images: ImageStore,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                tracks: Tracks::new(Arc::clone(&db)),
                stored_tracks: StoredTracks::new(Arc::clone(&db)),
                contents: Contents::new(db),
                ffmpeg,
                subtitles,
                images,
                tasks: TaskTracker::new(),
                cancel: CancellationToken::new(),
                running: tokio::sync::Semaphore::new(PROBES_AT_ONCE),
                whole_file_reads: tokio::sync::Semaphore::new(WHOLE_FILE_READS_AT_ONCE),
                claimed: Claims::default(),
                listing: Claims::default(),
                passed: Notify::new(),
                passed_for_segments: Arc::default(),
            }),
        }
    }

    /// The current Tracks of the Version `version`, sidecars included, probing it now if they
    /// aren't current, without waiting for a background pass to reach it. If something is probing
    /// it already, waits for that instead, or probes it itself if that gave up without finding
    /// anything.
    pub(crate) async fn current(&self, version: i64) -> Result<Probe, Error> {
        loop {
            let file = self.probed(version).await?;
            // Nothing is stored for a Version removed while it was probed, nor for this `file`
            // when the probe waited for was of the file as it was before a scan changed it, so
            // it's looked up and probed again.
            if let Some(probe) = self.inner.stored_tracks.of(&[file]).await?.remove(&version) {
                return Ok(probe);
            }
        }
    }

    /// The other Versions of the title the Version `version` is of, in its Library, by path, each
    /// with its current Tracks, sidecars included. One without current Tracks is left out, never
    /// probed.
    pub(super) async fn stored_other_versions(
        &self,
        version: i64,
    ) -> sqlx::Result<Vec<(i64, Probe)>> {
        let others = self.inner.contents.other_versions(version).await?;
        let mut probes = self.inner.stored_tracks.of(&others).await?;
        Ok((others.iter())
            .filter_map(|other| Some((other.id, probes.remove(&other.id)?)))
            .collect())
    }

    /// The Version `version`'s file as stored, once a probe of it has finished. A probe it waited
    /// for may have read the file as it was before a scan changed it, so its Tracks may not be
    /// stored for the file returned.
    async fn probed(&self, version: i64) -> Result<Stored, Error> {
        let (path, file) = (self.inner.contents.version_file(version).await?)
            .ok_or(Error::NoSuchVersion(version))?;
        // ponytail: not held to PROBES_AT_ONCE, so a play press never queues; limit it apart if
        // many at once swamp the disk.
        loop {
            let mut prober = match try_claim(&self.inner.claimed, file) {
                Ok(claim) => {
                    self.probe_claimed(&path, claim).await?;
                    return Ok(file);
                }
                Err(prober) => prober,
            };
            // An error means the prober gave up without a result, as when it was cancelled
            // midway, so it's probed here instead.
            if let Ok(found) = prober.wait_for(Option::is_some).await
                && let Some(found) = found.as_ref()
            {
                return found
                    .as_ref()
                    .map(|_| file)
                    .map_err(|err| Error::Probe(Arc::clone(err)));
            }
        }
    }

    /// The subtitle cache it cleans, where a burn-in reads what's cached instead of the file.
    pub(super) fn subtitles(&self) -> &subtitles::Cache {
        &self.inner.subtitles
    }

    /// When the Version `version`'s file starts and each keyframe of its first video falls: as
    /// kept for its file as it is now, by probing or an earlier listing, or else listed in the
    /// background, which reads the whole file, and kept. If something is listing them already,
    /// waits for that instead. Waits at most [`LISTING_WAIT`] for a listing, then returns
    /// [`ProbeError::NotListedYet`] while it carries on. They're kept only while the Version's
    /// Tracks were read from its file as it is now.
    pub(super) async fn keyframes(&self, version: i64) -> Result<ListedKeyframes, Error> {
        let (path, file) = (self.inner.contents.version_file(version).await?)
            .ok_or(Error::NoSuchVersion(version))?;
        loop {
            let mut lister = match try_claim(&self.inner.listing, file) {
                Ok(claim) => match self.inner.tracks.keyframes(file).await? {
                    Some(Keyframes::Listed(keyframes)) => return Ok(keyframes),
                    Some(Keyframes::Unlisted) => {
                        return Err(Error::Probe(Arc::new(ProbeError::UnlistedBefore)));
                    }
                    None => self
                        .list_in_background(path.clone(), claim)
                        .ok_or_else(|| Error::Probe(Arc::new(ProbeError::NotListedYet)))?,
                },
                Err(lister) => lister,
            };
            let waited = tokio::time::timeout(LISTING_WAIT, lister.wait_for(Option::is_some))
                .await
                .map_err(|_elapsed| Error::Probe(Arc::new(ProbeError::NotListedYet)))?;
            // An error means the lister gave up without listing them, as when shutting down
            // cancelled it, so they're read or listed here instead.
            if let Ok(found) = waited
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
                    Some(Ok(probed)) => {
                        probing.inner.passed.notify_one();
                        probing.inner.passed_for_segments.notify_one();
                        finished(probed);
                    }
                    Some(Err(err)) => {
                        tracing::error!(library, "probing a Library's files failed: {err}");
                    }
                    None => {}
                }
            }
            .instrument(pass_span),
        );
    }

    /// What's notified each time a background pass has caught up with a Library.
    pub(crate) fn passed(&self) -> Arc<Notify> {
        Arc::clone(&self.inner.passed_for_segments)
    }

    /// The subtitle cache, from which it throws away what's cached for files gone or changed.
    pub(super) fn subtitle_cache(&self) -> subtitles::Cache {
        self.inner.subtitles.clone()
    }

    /// Waits until a background pass has probed what it found stale, or returns at once if one
    /// has since the last call.
    pub(super) async fn pass_finished(&self) {
        self.inner.passed.notified().await;
    }

    /// Cancels the probes running, waits for them to stop, and starts no more passes.
    pub(crate) async fn shutdown(&self) {
        self.inner.cancel.cancel();
        self.inner.tasks.close();
        self.inner.tasks.wait().await;
    }

    /// Throws away the subtitles cached for files gone, such as a deleted Library's, or changed
    /// since they were cached, and deletes the subtitles downloaded for Versions gone and their
    /// Preview frames, or logs why it couldn't.
    pub(crate) async fn clean_leftovers(&self) {
        // ponytail: reads every stored file, and lists every downloaded subtitle, after each scan;
        // clean only the Library scanned once Libraries get big.
        let stored = match self.inner.contents.every_stored().await {
            Ok(stored) => stored,
            Err(err) => {
                tracing::error!("could not read which files to keep subtitles for: {err}");
                return;
            }
        };
        if let Err(err) = self.inner.subtitles.keep_only(&stored).await {
            tracing::error!("could not throw away subtitles of files gone or changed: {err}");
        }
        let images = &self.inner.images;
        for dir in [DOWNLOADED_SUBTITLES_DIR, PREVIEW_FRAMES_DIR] {
            if let Err(err) = keep_only(images, dir, &stored).await {
                tracing::error!("could not list {dir} to delete those of Versions gone: {err}");
            }
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
                    if matches!(
                        *err,
                        ProbeError::Run(RunError::Unavailable(_) | RunError::Run(..))
                    ) => {}
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
        claim: Claim<Probed>,
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
    async fn probe_claimed(&self, path: &str, claim: Claim<Probed>) -> Result<Probe, Unprobed> {
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
            Ok((probe, starts)) => {
                let indexed = match probe.container.as_str() {
                    "matroska,webm" => cues::listed(path, starts.file_ms, probe.duration_ms).await,
                    "mov,mp4,m4a,3gp,3g2,mj2" => mp4::listed(path, starts, probe.duration_ms).await,
                    _ => None,
                };
                self.inner.tracks.store(file, probe.clone()).await?;
                // Without a usable index, the first Remux lists them by reading the whole file.
                if let Some(indexed) = indexed {
                    (self.inner.tracks)
                        .store_keyframes(file, &Keyframes::Listed(indexed))
                        .await?;
                }
                Ok(probe)
            }
            Err(err) => {
                let failure = failure(&err, path).await;
                self.inner.tracks.store_failure(file, failure).await?;
                Err(Unprobed::Probe(Arc::new(err), failure))
            }
        }
    }

    /// Runs `read`, which reads a whole file, in `span` as a background task, so a request given
    /// up on doesn't stop it (#866), once fewer than [`WHOLE_FILE_READS_AT_ONCE`] such reads run.
    /// Shutting down cancels it. Returns `false`, dropping `read`, once the Server is shutting
    /// down.
    pub(super) fn read_whole_file_in_background(
        &self,
        span: tracing::Span,
        read: impl Future<Output = ()> + Send + 'static,
    ) -> bool {
        // A read spawned after shutdown would be cancelled at once, and whoever waited for it
        // would spawn it again, round and round.
        if self.inner.cancel.is_cancelled() {
            return false;
        }
        let probing = self.clone();
        let read = async move {
            // The semaphore is never closed.
            if let Ok(_reading) = probing.inner.whole_file_reads.acquire().await {
                read.await;
            }
        };
        let cancel = self.inner.cancel.clone();
        (self.inner.tasks)
            .spawn(async move { cancel.run_until_cancelled(read).await }.instrument(span));
        true
    }

    /// Lists the keyframes of the file at `path` as [`Self::list_claimed`] does, as a background
    /// task, so a plan request given up on doesn't stop it (#866), and hands back where it sends
    /// them. `None` once the Server is shutting down.
    fn list_in_background(
        &self,
        path: String,
        claim: Claim<Listed>,
    ) -> Option<watch::Receiver<Option<Listed>>> {
        let found = claim.found.subscribe();
        let listing_span = diagnostics::following_current(tracing::info_span!(
            parent: None,
            "keyframe listing",
            file.id = claim.file.id
        ));
        let probing = self.clone();
        let listing = async move {
            if let Err(err) = probing.list_claimed(&path, claim).await {
                tracing::error!("could not keep the keyframes listed for {path}: {err}");
            }
        };
        self.read_whole_file_in_background(listing_span, listing)
            .then_some(found)
    }

    /// Lists the keyframes of the file at `path`, stored as the file `claim` is for, keeps them,
    /// and sends them to whoever waited for them. A listing that failed for a reason that says
    /// something about the file is kept too, and returned as [`ProbeError::UnlistedBefore`] until
    /// the file changes or the probe failure version is bumped. Unlike a probe, a timeout isn't
    /// tried again: each try reads the whole file for up to `WHOLE_FILE_TIMEOUT` (#850).
    async fn list_claimed(&self, path: &str, claim: Claim<Listed>) -> Result<(), sqlx::Error> {
        let listed = ffprobe::keyframes(&self.inner.ffmpeg, path)
            .await
            .map_err(Arc::new);
        let kept = match &listed {
            Ok(keyframes) => Some(Keyframes::Listed(keyframes.clone())),
            Err(err) => {
                // The plan that started it may have stopped waiting, and so won't log it.
                tracing::warn!("could not list the keyframes of {path}: {err}");
                matches!(
                    failure(err, path).await,
                    ProbeFailure::Lasting(_) | ProbeFailure::Passing(_)
                )
                .then_some(Keyframes::Unlisted)
            }
        };
        let stored = match kept {
            Some(kept) => self.inner.tracks.store_keyframes(claim.file, &kept).await,
            None => Ok(()),
        };
        // Sent once kept, so whoever got them finds them kept when they next ask.
        claim.found.send_replace(Some(listed));
        stored
    }
}

/// Whether `listed`, read from a container's index, is a list a whole-file listing could give:
/// non-empty, increasing, the first at `first_ms` within rounding to the millisecond, and ending
/// by `duration_ms`.
fn plausible(listed: &ListedKeyframes, first_ms: i64, duration_ms: Option<i64>) -> bool {
    let keyframes = &listed.keyframes_ms;
    keyframes
        .first()
        .is_some_and(|first| (first - first_ms).abs() <= 1)
        && keyframes.is_sorted_by(|earlier, later| earlier < later)
        && (keyframes.last().zip(duration_ms)).is_some_and(|(last, duration)| *last <= duration)
}

/// What failing to probe the file at `path` with `err` means for trying it again.
async fn failure(err: &ProbeError, path: &str) -> ProbeFailure {
    match err {
        // ffprobe never reached the file, so counting it against the file would skip a whole
        // Library after one bad pass, such as during a package upgrade.
        ProbeError::Run(RunError::Run(..)) => ProbeFailure::StopsPass,
        // Nothing was read, so what's stored isn't known to be wrong, and a Server without
        // ffprobe keeps it, as it never probes in the background.
        ProbeError::Run(RunError::Unavailable(_)) => ProbeFailure::Retried,
        // ffprobe fails fast on a file it can't open, as on a share that dropped or an NFS handle
        // gone stale since the scan, which may read fine once it's back unchanged.
        ProbeError::Run(RunError::Failed(..)) if !readable(path).await => ProbeFailure::Retried,
        // ffprobe read the file and refused it, or said something it can't parse: it will again.
        // Only `Probing::keyframes` returns `UnlistedBefore` and `NotListedYet`, never ffprobe, so
        // no probe or listing gets here with them.
        ProbeError::Run(RunError::Failed(..))
        | ProbeError::UnlistedBefore
        | ProbeError::NotListedYet => ProbeFailure::Lasting(LastingReason::Failed),
        ProbeError::Output(_) => ProbeFailure::Lasting(LastingReason::UnreadableOutput),
        // A slow or unmounted share, or a busy disk, may not be slow next time.
        ProbeError::Run(RunError::TimedOut(_)) => ProbeFailure::Passing(PassingReason::TimedOut),
    }
}

/// Whether the file at `path` can be opened and read from now.
pub(super) async fn readable(path: &str) -> bool {
    use tokio::io::AsyncReadExt;

    match tokio::fs::File::open(path).await {
        Ok(mut file) => file.read(&mut [0; 1]).await.is_ok(),
        Err(_) => false,
    }
}

/// The right to probe one stored file, or list its keyframes, held by one task at a time and
/// given up when dropped, even by a probe cancelled midway.
#[derive(Debug)]
struct Claim<T> {
    claims: Claims<T>,
    file: Stored,
    /// Where what was found goes to whoever waits for it. Dropped with nothing sent, it tells
    /// them to do it themselves.
    found: watch::Sender<Option<T>>,
}

impl<T> Drop for Claim<T> {
    fn drop(&mut self) {
        lock(&self.claims).remove(&self.file.id);
    }
}

/// Claims the stored file `file` in `claims`, or if some task holds it, hands back where that will
/// send what it found.
fn try_claim<T>(claims: &Claims<T>, file: Stored) -> Result<Claim<T>, watch::Receiver<Option<T>>> {
    match lock(claims).entry(file.id) {
        Entry::Occupied(holder) => Err(holder.get().clone()),
        Entry::Vacant(unclaimed) => {
            let (found, waiting) = watch::channel(None);
            unclaimed.insert(waiting);
            Ok(Claim {
                claims: Arc::clone(claims),
                file,
                found,
            })
        }
    }
}

/// Deletes from `images` the folder under `dir` of every Version but `stored`'s, going on past one
/// it can't delete, which it logs.
///
/// # Errors
///
/// If the store can't be listed.
async fn keep_only(images: &ImageStore, dir: &str, stored: &[Stored]) -> io::Result<()> {
    let current: HashSet<String> = stored.iter().map(|file| file.id.to_string()).collect();
    let kept: HashSet<String> = (images.list(dir).await?.into_iter())
        .filter_map(|key| key.get(1).cloned())
        .collect();
    for gone in kept.difference(&current) {
        if let Err(err) = images.delete_dir_on_disk(&[dir, gone]).await {
            tracing::error!(
                version = gone,
                "could not delete a gone Version's {dir}: {err}"
            );
        }
    }
    Ok(())
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
#[path = "probing/probing_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "probing/retry_tests.rs"]
mod retry_tests;

#[cfg(test)]
#[path = "probing/keyframes_tests.rs"]
mod keyframes_tests;
