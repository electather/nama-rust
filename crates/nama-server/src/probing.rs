//! Probing: runs ffprobe on stored files and stores what each holds as its Tracks (see
//! [`crate::tracks`]), and decides what a failure to probe one means for trying it again. It also
//! lists a Version's keyframes when first asked for them, and keeps them for its file as it is
//! ([`Probing::keyframes`]).
//!
//! After a Library's files change, those without current Tracks, never probed or changed or read
//! by an older probe version since, are probed in the background, a few at a time
//! ([`Probing::files_changed`]). A file ffprobe couldn't read is skipped the same way until it
//! changes, as probing it again would fail again. One that failed for a passing reason, such as a
//! timeout, is tried again on the next few passes first. A Version can also be probed right away
//! ([`Probing::current`]), which waits for a background probe of it rather than probe it twice.

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

use crate::{
    contents::{Contents, Stored},
    db::Database,
    ffmpeg::{Ffmpeg, ProbeError},
    playback::subtitles,
    tracks::{LastingReason, PassingReason, Probe, ProbeFailure, Tracks},
};

/// How many files background passes probe at once, across all Libraries. Each probe reads only a
/// file's headers, but from a disk, or a network share, that playback is reading from too.
const PROBES_AT_ONCE: usize = 3;

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
    pub(crate) async fn keyframes(&self, version: i64) -> Result<Vec<i64>, Error> {
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
        self.inner.tasks.spawn(async move {
            let pass = probing.probe_stale(library);
            match probing.inner.cancel.run_until_cancelled(pass).await {
                Some(Ok(probed)) => finished(probed),
                Some(Err(err)) => {
                    tracing::error!(library, "probing a Library's files failed: {err}");
                }
                None => {}
            }
        });
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
    async fn probe_and_store(&self, path: &str, file: Stored) -> Result<Probe, Unprobed> {
        if self.inner.tracks.current(&[file]).await?.contains(&file.id)
            && let Some(probe) = self.inner.tracks.of(&[file]).await?.remove(&file.id)
        {
            return Ok(probe);
        }
        match self.inner.ffmpeg.probe(path).await {
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
    /// listed, sent to whoever waited for them, and kept.
    async fn list_claimed(&self, path: &str, claim: Claim<'_, Listed>) -> Result<Vec<i64>, Error> {
        if let Some(kept) = self.inner.tracks.keyframes(claim.file).await? {
            return Ok(kept);
        }
        let listed = self.inner.ffmpeg.keyframes(path).await.map_err(Arc::new);
        claim.found.send_replace(Some(listed.clone()));
        let keyframes = listed.map_err(Error::Probe)?;
        (self.inner.tracks)
            .store_keyframes(claim.file, &keyframes)
            .await?;
        Ok(keyframes)
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
        // ffprobe read the file and refused it, or said something it can't parse: it will again.
        ProbeError::Failed(_) => ProbeFailure::Lasting(LastingReason::Failed),
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
mod tests {
    use std::{fs, path::PathBuf, sync::Arc, time::Duration};

    use nama_plugin_api::{PluginId, metadata::ExternalIds};
    use nama_proto::nama::v1::ProbingFinished;
    use tempfile::TempDir;

    use super::{Error, PROBES_AT_ONCE, Probing};
    use crate::{
        config::Config,
        contents::{Change, Contents, Decided, Item, Metadata, Stored},
        db::Database,
        ffmpeg::{self, Ffmpeg, ProbeError},
        libraries::{Kind, Libraries},
        playback::subtitles,
        tracks::{
            BaseLayer, Hdr, PROBE_ATTEMPTS, PROBE_VERSION, PassingReason, Probe, ProbeFailure,
            Track, TrackKind, Tracks,
        },
    };

    /// A database, a Movies Library backed by one scratch folder, and Probing with `ffmpeg`.
    /// Files are stored as a scan would, without scanning.
    struct Fixture {
        probing: Probing,
        contents: Contents,
        tracks: Tracks,
        library: i64,
        root: PathBuf,
        _media: TempDir,
        _data: TempDir,
    }

    impl Fixture {
        /// With no ffprobe, so nothing is probed.
        async fn new() -> Self {
            Self::with_ffmpeg(Ffmpeg::default()).await
        }

        async fn with_ffmpeg(ffmpeg: Ffmpeg) -> Self {
            let data = tempfile::tempdir().unwrap();
            let media = tempfile::tempdir().unwrap();
            let root = media.path().canonicalize().unwrap().join("library");
            fs::create_dir(&root).unwrap();
            let db = Arc::new(Database::open(data.path()).await.unwrap());
            let library = Libraries::new(Arc::clone(&db))
                .create(
                    "Media",
                    Kind::Movies,
                    vec![root.to_str().unwrap().to_owned()],
                )
                .await
                .unwrap();
            Self {
                probing: Probing::new(
                    Arc::clone(&db),
                    Arc::new(ffmpeg),
                    subtitles::Cache::new(data.path()),
                ),
                tracks: Tracks::new(Arc::clone(&db)),
                contents: Contents::new(db),
                library: library.id,
                root,
                _media: media,
                _data: data,
            }
        }

        fn path(&self, relative: &str) -> PathBuf {
            self.root.join(relative)
        }

        /// Writes `size` bytes to the file at `relative` and stores it as a scan would.
        async fn write(&self, relative: &str, size: usize) {
            let path = self.path(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, vec![0_u8; size]).unwrap();
            self.store(relative).await;
        }

        /// Stores the file at `relative` as holding Alien, at its size and modified time now, as a
        /// scan would.
        async fn store(&self, relative: &str) {
            let path = self.path(relative);
            let metadata = fs::metadata(&path).unwrap();
            let modified = metadata.modified().unwrap();
            let file = Change::Store {
                path: path.to_str().unwrap().to_owned(),
                size: i64::try_from(metadata.len()).unwrap(),
                modified_ns: i64::try_from(
                    modified
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_nanos(),
                )
                .unwrap(),
                items: vec![Item::Movie {
                    title: "Alien".to_owned(),
                    year: Some(1979),
                    hint: None,
                }],
            };
            self.contents.apply(self.library, vec![file]).await.unwrap();
        }

        /// Tells Probing the Library's files changed, then waits for the pass it starts.
        async fn probe_changed(&self) {
            self.probing.files_changed(self.library, |_| {});
            self.probing.idle().await;
        }

        /// The channels of the one audio Track stored for the file at `relative`.
        #[cfg(unix)]
        async fn channels(&self, relative: &str) -> Option<i64> {
            let probe = self.probe(relative).await?;
            match probe.tracks.as_slice() {
                [
                    Track {
                        kind: TrackKind::Audio { channels },
                        ..
                    },
                ] => *channels,
                tracks => panic!("expected one audio Track: {tracks:?}"),
            }
        }

        /// The file stored at `relative`.
        async fn stored(&self, relative: &str) -> Stored {
            let path = self.path(relative);
            self.contents.stored(self.library).await.unwrap()[path.to_str().unwrap()]
        }

        /// The id of the file stored at `relative`.
        async fn file_id(&self, relative: &str) -> i64 {
            self.stored(relative).await.id
        }

        /// What probing found in the file at `relative`, if it read the file as it is stored now.
        async fn probe(&self, relative: &str) -> Option<Probe> {
            let file = self.stored(relative).await;
            self.tracks.of(&[file]).await.unwrap().remove(&file.id)
        }

        /// Matches every undecided Item to Alien, which makes its file a Version.
        async fn match_to_alien(&self) {
            let alien = Metadata {
                plugin: PluginId::from_static("tmdb"),
                ids: ExternalIds {
                    tmdb: Some(348),
                    ..ExternalIds::default()
                },
                show_ids: ExternalIds::default(),
                title: "Alien".to_owned(),
                original_title: None,
                show_title: None,
                overview: None,
                release_date: None,
                runtime_minutes: None,
                poster: None,
                backdrop: None,
            };
            let undecided = self.contents.undecided(self.library).await.unwrap();
            let decided = undecided
                .into_iter()
                .map(|(item, _, _)| (item, Decided::Matched(Box::new(alien.clone()))))
                .collect::<Vec<_>>();
            self.contents.record(decided).await.unwrap();
        }

        /// Probes the Version stored at `relative` right away.
        async fn current(&self, relative: &str) -> Result<Probe, Error> {
            self.probing.current(self.file_id(relative).await).await
        }
    }

    /// The ffmpeg and ffprobe on `PATH`, which tests that probe need.
    fn system_ffmpeg() -> Ffmpeg {
        let path = std::env::var_os("PATH").unwrap_or_default();
        let ffmpeg = Ffmpeg::find(&Config::default(), &path);
        assert!(
            ffmpeg.direct_play_only_because().is_none(),
            "this test needs ffmpeg and ffprobe on PATH: {ffmpeg:?}"
        );
        ffmpeg
    }

    /// Writes a fifth of a second of 4K video with `transfer` and 5.1 audio at `relative`, and
    /// stores it as a Version.
    async fn write_4k_with_5_1(media: &Fixture, relative: &str, transfer: &str) {
        let path = media.path(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        ffmpeg::write_4k_with_5_1(&path, transfer).unwrap();
        media.store(relative).await;
        media.match_to_alien().await;
    }

    #[tokio::test]
    async fn a_4k_hdr10_file_with_5_1_audio_is_probed_and_its_tracks_stored() {
        let media = Fixture::with_ffmpeg(system_ffmpeg()).await;
        let alien = "Alien (1979)/Alien (1979).mkv";
        write_4k_with_5_1(&media, alien, "smpte2084").await;

        let probe = media.current(alien).await.unwrap();

        assert_eq!(probe.container, "matroska,webm");
        // A fifth of a second, give or take a frame and the audio's padding.
        assert!(
            probe.duration_ms.is_some_and(|ms| (150..300).contains(&ms)),
            "{probe:?}"
        );
        assert!(probe.bitrate.is_some_and(|bps| bps > 0), "{probe:?}");
        assert_eq!(
            probe.tracks,
            [
                Track {
                    codec: "ffv1".to_owned(),
                    language: None,
                    title: None,
                    default: false,
                    forced: false,
                    sdh: false,
                    kind: TrackKind::Video {
                        width: Some(3840),
                        height: Some(2160),
                        frame_rate: Some(24.0),
                        bit_depth: Some(10),
                        hdr: Some(Hdr::Hdr10),
                    },
                },
                Track {
                    codec: "ac3".to_owned(),
                    language: Some("eng".to_owned()),
                    title: None,
                    default: false,
                    forced: false,
                    sdh: false,
                    kind: TrackKind::Audio { channels: Some(6) },
                },
            ]
        );
        assert_eq!(media.probe(alien).await, Some(probe));
        assert_eq!(
            media
                .tracks
                .probe_version_of(media.file_id(alien).await)
                .await,
            Some(PROBE_VERSION)
        );
    }

    #[tokio::test]
    async fn an_hlg_file_is_recorded_as_hlg_and_an_sdr_file_as_no_hdr() {
        let media = Fixture::with_ffmpeg(system_ffmpeg()).await;
        let hlg = "Alien (1979)/Alien (1979).mkv";
        let sdr = "Heat (1995)/Heat (1995).mkv";
        write_4k_with_5_1(&media, hlg, "arib-std-b67").await;
        write_4k_with_5_1(&media, sdr, "bt709").await;

        let hdr_of = async |file| {
            let tracks = media.current(file).await.unwrap().tracks;
            match tracks.first().map(|track| &track.kind) {
                Some(TrackKind::Video { hdr, .. }) => *hdr,
                _ => panic!("no video Track first: {tracks:?}"),
            }
        };
        assert_eq!(hdr_of(hlg).await, Some(Hdr::Hlg));
        assert_eq!(hdr_of(sdr).await, None);
    }

    #[tokio::test]
    async fn without_ffprobe_a_pass_stores_no_tracks() {
        let media = Fixture::new().await;
        let alien = "Alien (1979)/Alien (1979).mkv";
        media.write(alien, 10).await;

        media.probe_changed().await;

        let version = media.file_id(alien).await;
        assert_eq!(media.tracks.probe_version_of(version).await, None);
    }

    #[tokio::test]
    async fn without_ffprobe_probing_a_version_right_away_reports_that_probing_is_unavailable() {
        let media = Fixture::new().await;
        let alien = "Alien (1979)/Alien (1979).mkv";
        media.write(alien, 10).await;
        media.match_to_alien().await;

        let probed = media.current(alien).await;

        assert!(
            matches!(&probed, Err(Error::Probe(err)) if matches!(**err, ProbeError::Unavailable)),
            "{probed:?}"
        );
    }

    #[tokio::test]
    async fn probing_a_file_that_isnt_a_version_reports_no_such_version() {
        let media = Fixture::new().await;
        let alien = "Alien (1979)/Alien (1979).mkv";
        media.write(alien, 10).await;

        let probed = media.current(alien).await;

        assert!(matches!(probed, Err(Error::NoSuchVersion(_))), "{probed:?}");
    }

    /// An ffprobe in `bin` that runs `script`, a shell script.
    #[cfg(unix)]
    fn fake_ffprobe(bin: &std::path::Path, script: &str) -> Ffmpeg {
        Ffmpeg {
            ffprobe: Ok(ffmpeg::fake::script(
                bin,
                "ffprobe",
                &format!("#!/bin/sh\n{script}\n"),
            )),
            ..Ffmpeg::default()
        }
    }

    // ffmpeg can't write Dolby Vision, so a fake ffprobe prints what a real one does for it, cut
    // down to what probing reads. The fake is a shell script.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_dolby_vision_files_profile_and_whether_its_base_layer_is_hdr10_are_stored() {
        let bin = tempfile::tempdir().unwrap();
        let ffmpeg = fake_ffprobe(
            bin.path(),
            r#"cat <<'EOF'
{"streams": [{"codec_name": "hevc", "codec_type": "video", "pix_fmt": "yuv420p10le",
  "color_transfer": "smpte2084",
  "side_data_list": [{"side_data_type": "DOVI configuration record", "dv_profile": 8,
                      "dv_bl_signal_compatibility_id": 1}]}],
 "format": {"format_name": "matroska,webm", "duration": "1.000000"}}
EOF"#,
        );
        let media = Fixture::with_ffmpeg(ffmpeg).await;
        let alien = "Alien (1979)/Alien (1979).mkv";
        media.write(alien, 10).await;
        media.match_to_alien().await;

        let probe = media.current(alien).await.unwrap();

        let kind = probe.tracks.first().map(|track| track.kind.clone());
        assert_eq!(
            kind,
            Some(TrackKind::Video {
                width: None,
                height: None,
                frame_rate: None,
                bit_depth: Some(10),
                hdr: Some(Hdr::DolbyVision {
                    profile: 8,
                    base_layer: Some(BaseLayer::Hdr10),
                }),
            })
        );
        assert_eq!(media.probe(alien).await, Some(probe));
    }

    // The fake ffprobe is a shell script.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_file_whose_packets_cant_be_read_keeps_its_tracks_without_a_length() {
        let bin = tempfile::tempdir().unwrap();
        let ffmpeg = fake_ffprobe(
            bin.path(),
            r#"case "$*" in *packet=*) echo 'Invalid data found when processing input' >&2; exit 1 ;; esac
echo '{"format": {"format_name": "matroska,webm"}, "streams": [{"codec_type": "audio", "codec_name": "ac3", "channels": 2}]}'"#,
        );
        let media = Fixture::with_ffmpeg(ffmpeg).await;
        let alien = "Alien (1979)/Alien (1979).mkv";
        media.write(alien, 10).await;
        media.match_to_alien().await;

        let probe = media.current(alien).await.unwrap();

        assert_eq!((probe.tracks.len(), probe.duration_ms), (1, None));
        assert_eq!(media.probe(alien).await, Some(probe));
    }

    // The fake ffprobe is a shell script.
    #[cfg(unix)]
    #[tokio::test]
    async fn shutting_down_stops_the_probes() {
        let bin = tempfile::tempdir().unwrap();
        let media = Fixture::with_ffmpeg(fake_ffprobe(bin.path(), "exec sleep 60")).await;
        media.write("Alien (1979)/Alien (1979).mkv", 10).await;
        media.probing.files_changed(media.library, |_| {});

        let stopped = tokio::time::timeout(Duration::from_secs(5), media.probing.shutdown()).await;

        assert!(stopped.is_ok(), "shutting down waited for ffprobe");
    }

    /// An ffprobe in `bin` that adds a line to `runs` each time it runs, then takes a second to
    /// find a second-long Matroska file with no Tracks.
    #[cfg(unix)]
    fn slow_ffprobe(bin: &std::path::Path, runs: &std::path::Path) -> Ffmpeg {
        fake_ffprobe(
            bin,
            &format!(
                "echo run >> '{}'\nsleep 1\necho '{{\"format\": {{\"format_name\": \"matroska,webm\", \"duration\": \"1.000000\"}}}}'",
                runs.display()
            ),
        )
    }

    /// Waits until a [`slow_ffprobe`] writing `runs` has started.
    #[cfg(unix)]
    async fn wait_for_probe(runs: &std::path::Path) {
        let started = tokio::time::Instant::now();
        while !runs.exists() {
            assert!(
                started.elapsed() < Duration::from_secs(5),
                "the background probe never started"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    // The fake ffprobe is a shell script.
    #[cfg(unix)]
    #[tokio::test]
    async fn files_changed_during_a_pass_leaves_the_files_being_probed_to_it() {
        let bin = tempfile::tempdir().unwrap();
        let runs = bin.path().join("runs");
        let media = Fixture::with_ffmpeg(slow_ffprobe(bin.path(), &runs)).await;
        let alien = "Alien (1979)/Alien (1979).mkv";
        media.write(alien, 10).await;

        media.probing.files_changed(media.library, |_| {});
        wait_for_probe(&runs).await;
        media.probe_changed().await;

        assert_eq!(fs::read_to_string(&runs).unwrap(), "run\n");
        assert!(media.probe(alien).await.is_some());
    }

    /// Tells Probing the Library's files changed, and returns what its pass reports once done.
    fn files_changed_reporting(
        media: &Fixture,
    ) -> tokio::sync::mpsc::UnboundedReceiver<ProbingFinished> {
        let (told, reported) = tokio::sync::mpsc::unbounded_channel();
        media.probing.files_changed(media.library, move |probed| {
            drop(told.send(probed));
        });
        reported
    }

    // The fake ffprobe is a shell script.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_pass_waits_for_the_file_another_prober_holds_and_counts_it_as_probed() {
        let bin = tempfile::tempdir().unwrap();
        let runs = bin.path().join("runs");
        let media = Fixture::with_ffmpeg(slow_ffprobe(bin.path(), &runs)).await;
        let alien = "Alien (1979)/Alien (1979).mkv";
        media.write(alien, 10).await;
        let mut first = files_changed_reporting(&media);
        wait_for_probe(&runs).await;

        let mut second = files_changed_reporting(&media);

        let caught_up = ProbingFinished {
            library_id: media.library,
            probed: 1,
            ..Default::default()
        };
        assert_eq!(first.recv().await, Some(caught_up.clone()));
        assert_eq!(second.recv().await, Some(caught_up));
        assert_eq!(fs::read_to_string(&runs).unwrap(), "run\n");
    }

    // The fake ffprobe is a shell script.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_pass_probes_a_file_itself_once_the_prober_it_waited_for_gives_up() {
        let bin = tempfile::tempdir().unwrap();
        let runs = bin.path().join("runs");
        let media = Fixture::with_ffmpeg(slow_ffprobe(bin.path(), &runs)).await;
        let alien = "Alien (1979)/Alien (1979).mkv";
        media.write(alien, 10).await;
        media.match_to_alien().await;
        // Holds the background pass back until `current` has claimed the file.
        let busy = media
            .probing
            .inner
            .running
            .acquire_many(u32::try_from(PROBES_AT_ONCE).unwrap())
            .await
            .unwrap();
        let mut reported = files_changed_reporting(&media);
        let probing = media.probing.clone();
        let version = media.file_id(alien).await;
        let gives_up = tokio::spawn(async move { probing.current(version).await });
        wait_for_probe(&runs).await;
        drop(busy);
        // ponytail: no hook says when the pass is waiting; this gives it time to find the claim.
        tokio::time::sleep(Duration::from_millis(100)).await;

        gives_up.abort();

        let caught_up = ProbingFinished {
            library_id: media.library,
            probed: 1,
            ..Default::default()
        };
        assert_eq!(reported.recv().await, Some(caught_up));
        assert_eq!(fs::read_to_string(&runs).unwrap(), "run\nrun\n");
    }

    // The fake ffprobe is a shell script.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_pass_counts_the_files_probed_and_those_ffprobe_couldnt_read() {
        let bin = tempfile::tempdir().unwrap();
        // Reads a file of up to 10 bytes, failing on a bigger one.
        let ffmpeg = fake_ffprobe(
            bin.path(),
            r#"for file; do :; done
[ "$(wc -c < "$file")" -le 10 ] || exit 1
echo '{"format": {"format_name": "matroska,webm", "duration": "1.000000"}}'"#,
        );
        let media = Fixture::with_ffmpeg(ffmpeg).await;
        media.write("Alien (1979)/Alien (1979).mkv", 10).await;
        media.write("Heat (1995)/Heat (1995).mkv", 20).await;

        let mut first = files_changed_reporting(&media);

        let counted = ProbingFinished {
            library_id: media.library,
            probed: 1,
            failed: 1,
            ..Default::default()
        };
        assert_eq!(first.recv().await, Some(counted));

        // Nothing is left to probe: Alien's Tracks are current, and Heat failed for good.
        let mut second = files_changed_reporting(&media);

        let nothing = ProbingFinished {
            library_id: media.library,
            ..Default::default()
        };
        assert_eq!(second.recv().await, Some(nothing));
    }

    // The fake ffprobe is a shell script.
    #[cfg(unix)]
    #[tokio::test]
    async fn probing_a_version_right_away_while_the_background_probes_it_runs_ffprobe_once() {
        let bin = tempfile::tempdir().unwrap();
        let runs = bin.path().join("runs");
        let media = Fixture::with_ffmpeg(slow_ffprobe(bin.path(), &runs)).await;
        let alien = "Alien (1979)/Alien (1979).mkv";
        media.write(alien, 10).await;
        media.match_to_alien().await;
        media.probing.files_changed(media.library, |_| {});
        wait_for_probe(&runs).await;

        let probe = media.current(alien).await.unwrap();

        assert_eq!(probe.container, "matroska,webm");
        media.probing.idle().await;
        assert_eq!(fs::read_to_string(&runs).unwrap(), "run\n");
    }

    // The fake ffprobe is a shell script.
    #[cfg(unix)]
    #[tokio::test]
    async fn probing_a_version_right_away_while_the_background_probe_fails_returns_that_failure() {
        let bin = tempfile::tempdir().unwrap();
        let runs = bin.path().join("runs");
        let ffmpeg = fake_ffprobe(
            bin.path(),
            &format!("echo run >> '{}'\nsleep 1\nexit 1", runs.display()),
        );
        let media = Fixture::with_ffmpeg(ffmpeg).await;
        let alien = "Alien (1979)/Alien (1979).mkv";
        media.write(alien, 10).await;
        media.match_to_alien().await;
        media.probing.files_changed(media.library, |_| {});
        wait_for_probe(&runs).await;

        let probed = media.current(alien).await;

        assert!(
            matches!(&probed, Err(Error::Probe(err)) if matches!(**err, ProbeError::Failed(_))),
            "{probed:?}"
        );
        media.probing.idle().await;
        assert_eq!(fs::read_to_string(&runs).unwrap(), "run\n");
    }

    // The fake ffprobe is a shell script.
    #[cfg(unix)]
    #[tokio::test]
    async fn probing_a_version_right_away_doesnt_wait_for_the_background_pass_to_reach_it() {
        let bin = tempfile::tempdir().unwrap();
        let runs = bin.path().join("runs");
        let media = Fixture::with_ffmpeg(slow_ffprobe(bin.path(), &runs)).await;
        let alien = "Alien (1979)/Alien (1979).mkv";
        media.write(alien, 10).await;
        media.match_to_alien().await;
        // The background pass is busy with other files.
        let busy = (media.probing.inner.running)
            .acquire_many(u32::try_from(PROBES_AT_ONCE).unwrap())
            .await
            .unwrap();
        media.probing.files_changed(media.library, |_| {});

        let probed = tokio::time::timeout(Duration::from_secs(5), media.current(alien)).await;

        assert!(matches!(probed, Ok(Ok(_))), "{probed:?}");
        drop(busy);
        media.probing.idle().await;
        assert_eq!(fs::read_to_string(&runs).unwrap(), "run\n");
    }

    /// A fake ffprobe in `bin` that logs each run to `bin/runs` and reads every file as one audio
    /// Track with as many channels as the file has bytes, so a changed file reads differently.
    #[cfg(unix)]
    fn counting_ffprobe(bin: &std::path::Path) -> Ffmpeg {
        let runs = bin.join("runs");
        fake_ffprobe(
            bin,
            &format!(
                r#"echo run >> '{}'
for file; do :; done
echo "{{\"format\": {{\"format_name\": \"matroska,webm\", \"duration\": \"1.000000\"}}, \"streams\": [{{\"codec_type\": \"audio\", \"codec_name\": \"ac3\", \"channels\": $(wc -c < "$file")}}]}}""#,
                runs.display()
            ),
        )
    }

    /// How many times [`counting_ffprobe`] in `bin` has run.
    #[cfg(unix)]
    fn probe_runs(bin: &std::path::Path) -> usize {
        fs::read_to_string(bin.join("runs")).map_or(0, |runs| runs.lines().count())
    }

    // The fake ffprobe is a shell script.
    #[cfg(unix)]
    #[tokio::test]
    async fn no_more_files_than_the_limit_are_probed_at_once() {
        let bin = tempfile::tempdir().unwrap();
        let running = bin.path().join("running");
        let most = bin.path().join("most");
        fs::create_dir(&running).unwrap();
        // Counts the ffprobes running alongside it in `running`, keeping the most in `most`.
        let ffmpeg = fake_ffprobe(
            bin.path(),
            &format!(
                r#"me='{running}'/$$
touch "$me"
now=$(ls '{running}' | wc -l)
[ "$now" -gt "$(cat '{most}' 2>/dev/null || echo 0)" ] && echo "$now" > '{most}'
sleep 0.3
rm "$me"
echo '{{"format": {{"format_name": "matroska,webm", "duration": "1.000000"}}}}'"#,
                running = running.display(),
                most = most.display()
            ),
        );
        let media = Fixture::with_ffmpeg(ffmpeg).await;
        for n in 0..PROBES_AT_ONCE * 2 {
            media
                .write(&format!("Movie {n} (2000)/Movie {n} (2000).mkv"), 10)
                .await;
        }

        media.probe_changed().await;

        let most: usize = fs::read_to_string(&most).unwrap().trim().parse().unwrap();
        assert!((1..=PROBES_AT_ONCE).contains(&most), "{most} ran at once");
    }

    // The fake ffprobe is a shell script.
    #[cfg(unix)]
    #[tokio::test]
    async fn an_unchanged_file_is_probed_again_only_once_the_probe_version_is_bumped() {
        let bin = tempfile::tempdir().unwrap();
        let media = Fixture::with_ffmpeg(counting_ffprobe(bin.path())).await;
        let alien = "Alien (1979)/Alien (1979).mkv";
        media.write(alien, 10).await;
        media.probe_changed().await;
        media.probe_changed().await;
        assert_eq!(
            probe_runs(bin.path()),
            1,
            "an unchanged file was probed again"
        );

        let version = media.file_id(alien).await;
        media.tracks.read_by_older_probe(version).await;
        media.probe_changed().await;

        assert_eq!(probe_runs(bin.path()), 2);
        assert_eq!(
            media.tracks.probe_version_of(version).await,
            Some(PROBE_VERSION)
        );
        media.probe_changed().await;
        assert_eq!(
            probe_runs(bin.path()),
            2,
            "probed again at the current version"
        );
    }

    // The fake ffprobe is a shell script.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_file_replaced_by_another_is_probed_again_and_its_tracks_replaced() {
        let bin = tempfile::tempdir().unwrap();
        let media = Fixture::with_ffmpeg(counting_ffprobe(bin.path())).await;
        let alien = "Alien (1979)/Alien (1979).mkv";
        media.write(alien, 10).await;
        media.probe_changed().await;
        assert_eq!(media.channels(alien).await, Some(10));

        media.write(alien, 20).await;
        media.probe_changed().await;

        assert_eq!(probe_runs(bin.path()), 2);
        assert_eq!(media.channels(alien).await, Some(20));
    }

    // The fake ffprobe is a shell script.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_file_touched_without_changing_size_is_probed_again() {
        let bin = tempfile::tempdir().unwrap();
        let media = Fixture::with_ffmpeg(counting_ffprobe(bin.path())).await;
        let alien = "Alien (1979)/Alien (1979).mkv";
        media.write(alien, 10).await;
        media.probe_changed().await;

        let touched = std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        fs::File::options()
            .write(true)
            .open(media.path(alien))
            .unwrap()
            .set_modified(touched)
            .unwrap();
        media.store(alien).await;
        media.probe_changed().await;

        assert_eq!(probe_runs(bin.path()), 2);
        assert_eq!(media.channels(alien).await, Some(10));
    }

    // The fake ffprobe is a shell script.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_file_replaced_by_one_that_cant_be_probed_loses_its_old_tracks() {
        let bin = tempfile::tempdir().unwrap();
        // Reads a file of up to 10 bytes as one audio Track, and fails on a bigger one.
        let ffmpeg = fake_ffprobe(
            bin.path(),
            r#"for file; do :; done
[ "$(wc -c < "$file")" -le 10 ] || exit 1
echo '{"format": {"format_name": "matroska,webm", "duration": "1.000000"}, "streams": [{"codec_type": "audio", "codec_name": "ac3", "channels": 2}]}'"#,
        );
        let media = Fixture::with_ffmpeg(ffmpeg).await;
        let alien = "Alien (1979)/Alien (1979).mkv";
        media.write(alien, 10).await;
        media.probe_changed().await;
        assert_eq!(media.channels(alien).await, Some(2));

        media.write(alien, 20).await;
        media.probe_changed().await;

        let version = media.file_id(alien).await;
        assert_eq!(media.tracks.probe_version_of(version).await, None);
    }

    // The fake ffprobe is a shell script.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_file_that_cant_be_probed_is_probed_again_only_once_it_changes() {
        let bin = tempfile::tempdir().unwrap();
        let runs = bin.path().join("runs");
        // Logs each run, and reads a file of up to 10 bytes as one audio Track, failing on a
        // bigger one.
        let ffmpeg = fake_ffprobe(
            bin.path(),
            &format!(
                r#"echo run >> '{}'
for file; do :; done
[ "$(wc -c < "$file")" -le 10 ] || exit 1
echo '{{"format": {{"format_name": "matroska,webm", "duration": "1.000000"}}, "streams": [{{"codec_type": "audio", "codec_name": "ac3", "channels": 2}}]}}'"#,
                runs.display()
            ),
        );
        let media = Fixture::with_ffmpeg(ffmpeg).await;
        let alien = "Alien (1979)/Alien (1979).mkv";
        media.write(alien, 20).await;
        media.probe_changed().await;
        media.probe_changed().await;
        assert_eq!(
            probe_runs(bin.path()),
            1,
            "an unreadable file was probed again"
        );

        media.write(alien, 10).await;
        media.probe_changed().await;

        assert_eq!(probe_runs(bin.path()), 2);
        assert_eq!(media.channels(alien).await, Some(2));
    }

    // The fake ffprobe is a shell script.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_file_that_couldnt_be_probed_is_probed_again_once_the_probe_version_is_bumped() {
        let bin = tempfile::tempdir().unwrap();
        let runs = bin.path().join("runs");
        let ffmpeg = fake_ffprobe(
            bin.path(),
            &format!("echo run >> '{}'\nexit 1", runs.display()),
        );
        let media = Fixture::with_ffmpeg(ffmpeg).await;
        let alien = "Alien (1979)/Alien (1979).mkv";
        media.write(alien, 10).await;
        media.probe_changed().await;

        media
            .tracks
            .failed_by_older_probe(media.file_id(alien).await)
            .await;
        media.probe_changed().await;

        assert_eq!(probe_runs(bin.path()), 2);
    }

    // The fake ffprobe is a shell script.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_file_that_couldnt_be_probed_is_probed_again_once_the_failure_version_is_bumped() {
        let bin = tempfile::tempdir().unwrap();
        let runs = bin.path().join("runs");
        let ffmpeg = fake_ffprobe(
            bin.path(),
            &format!("echo run >> '{}'\nexit 1", runs.display()),
        );
        let media = Fixture::with_ffmpeg(ffmpeg).await;
        let alien = "Alien (1979)/Alien (1979).mkv";
        media.write(alien, 10).await;
        media.probe_changed().await;
        let version = media.file_id(alien).await;
        assert_eq!(
            media.tracks.failure_reason_of(version).await.as_deref(),
            Some("failed")
        );

        media.tracks.failed_by_older_failure_version(version).await;
        media.probe_changed().await;

        assert_eq!(probe_runs(bin.path()), 2);
    }

    // The fake ffprobe is a shell script.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_file_ffprobe_couldnt_be_run_on_is_probed_again_next_pass() {
        let bin = tempfile::tempdir().unwrap();
        let media = Fixture::with_ffmpeg(counting_ffprobe(bin.path())).await;
        let alien = "Alien (1979)/Alien (1979).mkv";
        media.write(alien, 10).await;
        fs::remove_file(bin.path().join("ffprobe")).unwrap();
        media.probe_changed().await;
        let version = media.file_id(alien).await;
        assert_eq!(media.tracks.failure_reason_of(version).await, None);

        counting_ffprobe(bin.path());
        media.probe_changed().await;

        assert_eq!(media.channels(alien).await, Some(10));
    }

    // The fake ffprobe is a shell script.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_file_is_probed_once_ffprobe_runs_again_however_many_passes_it_couldnt() {
        let bin = tempfile::tempdir().unwrap();
        let media = Fixture::with_ffmpeg(counting_ffprobe(bin.path())).await;
        let alien = "Alien (1979)/Alien (1979).mkv";
        media.write(alien, 10).await;
        fs::remove_file(bin.path().join("ffprobe")).unwrap();
        for _ in 0..=PROBE_ATTEMPTS {
            media.probe_changed().await;
        }

        counting_ffprobe(bin.path());
        media.probe_changed().await;

        assert_eq!(media.channels(alien).await, Some(10));
    }

    const TIMED_OUT: ProbeFailure = ProbeFailure::Passing(PassingReason::TimedOut);

    // The fake ffprobe is a shell script.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_file_that_timed_out_is_probed_again_next_pass() {
        let bin = tempfile::tempdir().unwrap();
        let media = Fixture::with_ffmpeg(counting_ffprobe(bin.path())).await;
        let alien = "Alien (1979)/Alien (1979).mkv";
        media.write(alien, 10).await;
        let file = media.stored(alien).await;
        for _ in 1..PROBE_ATTEMPTS {
            media.tracks.store_failure(file, TIMED_OUT).await.unwrap();
        }

        media.probe_changed().await;

        assert_eq!(media.channels(alien).await, Some(10));
    }

    // The fake ffprobe is a shell script.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_file_that_keeps_timing_out_is_given_up_on() {
        let bin = tempfile::tempdir().unwrap();
        let media = Fixture::with_ffmpeg(counting_ffprobe(bin.path())).await;
        let alien = "Alien (1979)/Alien (1979).mkv";
        media.write(alien, 10).await;
        let file = media.stored(alien).await;
        for _ in 0..PROBE_ATTEMPTS {
            media.tracks.store_failure(file, TIMED_OUT).await.unwrap();
        }

        media.probe_changed().await;
        assert_eq!(probe_runs(bin.path()), 0, "probed after too many attempts");

        media.tracks.failed_by_older_failure_version(file.id).await;
        media.probe_changed().await;
        assert_eq!(media.channels(alien).await, Some(10));
    }

    // The fake ffprobe is a shell script.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_file_gone_by_the_time_its_probed_is_probed_again_once_back() {
        let bin = tempfile::tempdir().unwrap();
        let gone = bin.path().join("gone");
        // The first time, moves the file away as a dropped share would, and fails on it.
        let ffmpeg = fake_ffprobe(
            bin.path(),
            &format!(
                r#"for file; do :; done
if [ ! -e '{gone}' ]; then touch '{gone}'; mv "$file" "$file.away"; exit 1; fi
echo "{{\"format\": {{\"format_name\": \"matroska,webm\", \"duration\": \"1.000000\"}}, \"streams\": [{{\"codec_type\": \"audio\", \"codec_name\": \"ac3\", \"channels\": $(wc -c < "$file")}}]}}""#,
                gone = gone.display()
            ),
        );
        let media = Fixture::with_ffmpeg(ffmpeg).await;
        let alien = "Alien (1979)/Alien (1979).mkv";
        media.write(alien, 10).await;
        media.probe_changed().await;
        let version = media.file_id(alien).await;
        assert_eq!(media.tracks.failure_reason_of(version).await, None);

        // A rename keeps its size and modified time, as a share coming back would.
        let path = media.path(alien);
        fs::rename(path.with_extension("mkv.away"), &path).unwrap();
        media.probe_changed().await;

        assert_eq!(media.channels(alien).await, Some(10));
    }

    /// A fake ffprobe in `bin` that reads every file as a second-long Matroska file with no
    /// Tracks, and lists keyframes at 0 and at as many seconds as the file has bytes, so a changed
    /// file lists differently. Each listing logs a line to `bin/runs` first, then takes a second.
    #[cfg(unix)]
    fn listing_ffprobe(bin: &std::path::Path) -> Ffmpeg {
        let runs = bin.join("runs");
        fake_ffprobe(
            bin,
            &format!(
                r#"for file; do :; done
case "$*" in *packet=*)
    echo list >> '{}'
    sleep 1
    printf '0.000000,K__\n%s.000000,K__\n' "$(wc -c < "$file")"
    exit ;;
esac
echo '{{"format": {{"format_name": "matroska,webm", "duration": "1.000000"}}}}'"#,
                runs.display()
            ),
        )
    }

    impl Fixture {
        /// Writes `size` bytes to the file at `relative`, stores it as a Version, and probes it.
        async fn write_probed(&self, relative: &str, size: usize) {
            self.write(relative, size).await;
            self.match_to_alien().await;
            self.current(relative).await.unwrap();
        }

        /// The keyframes of the Version stored at `relative`.
        async fn keyframes(&self, relative: &str) -> Vec<i64> {
            let version = self.file_id(relative).await;
            self.probing.keyframes(version).await.unwrap()
        }
    }

    // The fake ffprobe is a shell script.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_versions_keyframes_are_listed_once_and_kept() {
        let bin = tempfile::tempdir().unwrap();
        let media = Fixture::with_ffmpeg(listing_ffprobe(bin.path())).await;
        let alien = "Alien (1979)/Alien (1979).mkv";
        media.write_probed(alien, 10).await;

        assert_eq!(media.keyframes(alien).await, [0, 10_000]);
        assert_eq!(media.keyframes(alien).await, [0, 10_000]);

        assert_eq!(probe_runs(bin.path()), 1);
    }

    // The fake ffprobe is a shell script.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_versions_keyframes_asked_for_at_once_are_listed_once() {
        let bin = tempfile::tempdir().unwrap();
        let media = Fixture::with_ffmpeg(listing_ffprobe(bin.path())).await;
        let alien = "Alien (1979)/Alien (1979).mkv";
        media.write_probed(alien, 10).await;
        let version = media.file_id(alien).await;
        let probing = media.probing.clone();
        let first = tokio::spawn(async move { probing.keyframes(version).await });
        wait_for_probe(&bin.path().join("runs")).await;

        let second = media.probing.keyframes(version).await.unwrap();

        assert_eq!(first.await.unwrap().unwrap(), second);
        assert_eq!(probe_runs(bin.path()), 1);
    }

    // The fake ffprobe is a shell script.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_versions_keyframes_are_listed_again_once_its_files_size_changes() {
        let bin = tempfile::tempdir().unwrap();
        let media = Fixture::with_ffmpeg(listing_ffprobe(bin.path())).await;
        let alien = "Alien (1979)/Alien (1979).mkv";
        media.write_probed(alien, 10).await;
        media.keyframes(alien).await;

        // Stored as a scan would, but not probed again, so its Tracks were read from the old file.
        media.write(alien, 20).await;
        media.match_to_alien().await;

        assert_eq!(media.keyframes(alien).await, [0, 20_000]);
        assert_eq!(probe_runs(bin.path()), 2);
    }

    // The fake ffprobe is a shell script.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_versions_keyframes_are_listed_again_once_its_file_is_touched() {
        let bin = tempfile::tempdir().unwrap();
        let media = Fixture::with_ffmpeg(listing_ffprobe(bin.path())).await;
        let alien = "Alien (1979)/Alien (1979).mkv";
        media.write_probed(alien, 10).await;
        media.keyframes(alien).await;

        let touched = std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        fs::File::options()
            .write(true)
            .open(media.path(alien))
            .unwrap()
            .set_modified(touched)
            .unwrap();
        media.store(alien).await;
        media.match_to_alien().await;
        media.keyframes(alien).await;

        assert_eq!(probe_runs(bin.path()), 2);
    }
}
