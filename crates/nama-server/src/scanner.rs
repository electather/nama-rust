//! The scanner: walks each Library's folders and stores the video files in them, and what their
//! names say they hold (see [`names`]).
//!
//! A rescan is incremental. A file whose size and modified time are what was stored is skipped,
//! unless an older version of the name parser read it (ADR 0023); a new or changed file is parsed
//! and stored; a stored file that's gone is removed, except under a folder that couldn't be read,
//! so a drive that isn't mounted doesn't empty the Library. A renamed file is a file gone and a
//! file found. Changes are written in batches of [`BATCH_SIZE`] (see the crate's `AGENTS.md`).
//!
//! Files are only listed and their metadata read, never opened: sidecars such as `.nfo` files
//! are ignored.
//!
//! A scan publishes its start, then after each batch is written its progress and the Items it
//! added and removed, then its finish (see [`crate::events`]).
//!
//! After each scan the Library's items are Matched (see [`crate::matching`]), still under the
//! Library's scan lock, so Matching sees the items the scan stored. Then its files without current
//! Tracks, never probed or changed or read by an older probe version since, are probed in the
//! background, a few at a time, and their Tracks stored (see [`crate::tracks`]), without holding
//! up the next scan. A Version can also be probed right away ([`Scanner::probe_now`]), which
//! waits for a background probe of it rather than probe it twice.

mod names;

pub(crate) use names::{PARSER_VERSION, country_tag};

use std::{
    collections::{HashMap, HashSet, hash_map::Entry},
    ffi::OsStr,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, PoisonError},
    time::{SystemTime, UNIX_EPOCH},
};

use futures_util::StreamExt;
use nama_proto::nama::v1::{ScanFinished, ScanProgress, ScanStarted};
use tokio::sync::watch;
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use walkdir::{DirEntry, WalkDir};

use crate::{
    contents::{Change, Contents, Stored},
    db::Database,
    events::{EventBus, event},
    ffmpeg::{Ffmpeg, ProbeError},
    libraries::{self, Kind, Libraries},
    matching::{self, ManualError, Matcher, count},
    plugins::Registry,
    tracks::{Probe, Tracks},
};

/// Extensions, in lowercase, of the files the scanner treats as video.
const VIDEO_EXTENSIONS: &[&str] = &[
    "3gp", "avi", "divx", "flv", "m2ts", "m4v", "mkv", "mov", "mp4", "mpeg", "mpg", "mts", "ogm",
    "ogv", "ts", "webm", "wmv",
];

/// How many changed files one write stores or removes. Small enough that other writes don't
/// wait long behind a scan.
const BATCH_SIZE: usize = 200;

/// How many files background passes probe at once, across all Libraries. Each probe reads only a
/// file's headers, but from a disk, or a network share, that playback is reading from too.
const PROBES_AT_ONCE: usize = 3;

/// What one scan changed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Summary {
    /// Files found that weren't stored before.
    pub(crate) added: u64,
    /// Stored files parsed again: those whose size or modified time changed, or that an older
    /// name parser read.
    pub(crate) updated: u64,
    /// Stored files that are gone.
    pub(crate) removed: u64,
    /// Stored files left as they were: those with the same size and modified time, and those
    /// under a folder that couldn't be read.
    pub(crate) unchanged: u64,
}

/// Why a scan failed.
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    /// The Library couldn't be read, or there is no such Library.
    #[error(transparent)]
    Library(#[from] libraries::Error),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    /// Walking the Library's folders panicked.
    #[error("walking the folders failed: {0}")]
    Walk(#[from] tokio::task::JoinError),
    /// The Server is shutting down.
    #[error("the scan was cancelled")]
    Cancelled,
}

impl From<matching::Error> for Error {
    fn from(err: matching::Error) -> Self {
        match err {
            matching::Error::Database(err) => Self::Database(err),
            matching::Error::Cancelled => Self::Cancelled,
        }
    }
}

/// Why probing a Version right away failed.
#[derive(Debug, thiserror::Error)]
pub(crate) enum ProbeNowError {
    /// ffprobe couldn't read it, or the Server has no ffprobe ([`ProbeError::Unavailable`]).
    /// Shared, as whoever waited for the same probe gets it too.
    #[error(transparent)]
    Probe(Arc<ProbeError>),
    #[error("there is no Version {0}")]
    NoSuchVersion(i64),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl From<ProbeError> for ProbeNowError {
    fn from(err: ProbeError) -> Self {
        Self::Probe(Arc::new(err))
    }
}

/// What probing a file found, or why it couldn't, as handed to whoever waited for it.
type Probed = Result<Probe, Arc<ProbeError>>;

/// Scans Libraries, one scan per Library at a time, either now or in the background.
#[derive(Debug, Clone)]
pub(crate) struct Scanner {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    contents: Contents,
    libraries: Libraries,
    matcher: Matcher,
    events: EventBus,
    tracks: Tracks,
    ffmpeg: Arc<Ffmpeg>,
    /// Background scans and probes, so shutting down can wait for them.
    tasks: TaskTracker,
    /// Cancelled when the Server shuts down.
    cancel: CancellationToken,
    /// Libraries with a background scan waiting to start.
    queued: Mutex<HashSet<i64>>,
    /// Each Library's scan lock, handed out as a [`ScanLock`].
    running: Mutex<HashMap<i64, Arc<tokio::sync::Mutex<()>>>>,
    /// A permit per probe running, [`PROBES_AT_ONCE`] in all.
    probing: tokio::sync::Semaphore,
    /// The Versions being probed, each by one [`Claim`] holder, and where it sends what it found,
    /// so a pass after another scan of their Library leaves them to it and
    /// [`Scanner::probe_now`] waits for its result, rather than probe them twice.
    claimed: Mutex<HashMap<i64, watch::Receiver<Option<Probed>>>>,
}

/// A Library's scan lock: while it's held, no scan of the Library runs or starts, so the
/// Library's items don't change. Only the [`Scanner`] makes one.
#[derive(Debug)]
pub(crate) struct ScanLock {
    library: i64,
    _guard: tokio::sync::OwnedMutexGuard<()>,
}

impl ScanLock {
    /// The Library this lock covers.
    pub(crate) const fn library(&self) -> i64 {
        self.library
    }
}

impl Scanner {
    /// Probes files with `ffmpeg`'s ffprobe, or not at all without one.
    pub(crate) fn new(
        db: Arc<Database>,
        plugins: Registry,
        events: EventBus,
        ffmpeg: Arc<Ffmpeg>,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                libraries: Libraries::new(Arc::clone(&db)),
                matcher: Matcher::new(Arc::clone(&db), plugins, events.clone()),
                events,
                tracks: Tracks::new(Arc::clone(&db)),
                ffmpeg,
                contents: Contents::new(db),
                tasks: TaskTracker::new(),
                cancel: CancellationToken::new(),
                queued: Mutex::default(),
                running: Mutex::default(),
                probing: tokio::sync::Semaphore::new(PROBES_AT_ONCE),
                claimed: Mutex::default(),
            }),
        }
    }

    /// Starts scanning the Library `id` in the background. If a scan of it is running, one more
    /// follows, so what changed since the running scan began is picked up; if one is already
    /// waiting to follow, that one does.
    pub(crate) async fn start(&self, id: i64) -> Result<(), libraries::Error> {
        self.inner.libraries.get(id).await?;
        self.queue(id);
        Ok(())
    }

    /// Starts scanning every Library in the background.
    pub(crate) async fn start_all(&self) {
        match self.inner.libraries.list().await {
            Ok(libraries) => libraries.iter().for_each(|library| self.queue(library.id)),
            Err(err) => tracing::error!("could not list the Libraries to scan: {err}"),
        }
    }

    /// Cancels the scans running or waiting, waits for them to stop, and starts no more.
    pub(crate) async fn shutdown(&self) {
        self.inner.cancel.cancel();
        self.inner.tasks.close();
        self.inner.tasks.wait().await;
    }

    /// Queues a background scan of the Library `id`, unless one is already waiting.
    fn queue(&self, id: i64) {
        if self.inner.cancel.is_cancelled() || !lock(&self.inner.queued).insert(id) {
            return;
        }
        let scanner = self.clone();
        self.inner.tasks.spawn(async move {
            let scan = scanner.lock(id).await;
            lock(&scanner.inner.queued).remove(&id);
            match scanner.scan_locked(&scan).await {
                // The Library was deleted after the scan was asked for.
                Ok(_) | Err(Error::Cancelled | Error::Library(libraries::Error::NotFound(_))) => {}
                Err(err) => tracing::error!(library = id, "scanning a Library failed: {err}"),
            }
        });
    }

    /// Waits for any scan of the Library `id` to finish, then takes its scan lock.
    pub(crate) async fn lock(&self, id: i64) -> ScanLock {
        let mutex = Arc::clone(lock(&self.inner.running).entry(id).or_default());
        ScanLock {
            library: id,
            _guard: mutex.lock_owned().await,
        }
    }

    /// Takes the scan lock of the Library the item `item` is in, refusing an unknown item as
    /// [`ManualError::NoSuchItem`].
    pub(crate) async fn lock_library_of(&self, item: i64) -> Result<ScanLock, ManualError> {
        let library = self
            .inner
            .contents
            .library_of(item)
            .await?
            .ok_or(ManualError::NoSuchItem(item))?;
        Ok(self.lock(library).await)
    }

    async fn scan_locked(&self, scan: &ScanLock) -> Result<Summary, Error> {
        let id = scan.library();
        let cancel = self.inner.cancel.clone();
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let library = self.inner.libraries.get(id).await?;
        self.inner.events.publish(vec![event(ScanStarted {
            library_id: id,
            ..Default::default()
        })]);
        let stored = self.inner.contents.stored(id).await?;
        let walking = cancel.clone();
        let found = tokio::task::spawn_blocking(move || walk(&library.folders, &walking)).await??;
        let (changes, summary) = plan(library.kind, stored, found);
        let files_total = count(changes.len());
        let mut files_done = 0;
        let mut changes = changes.into_iter();
        loop {
            let batch: Vec<Change> = changes.by_ref().take(BATCH_SIZE).collect();
            if batch.is_empty() {
                break;
            }
            if cancel.is_cancelled() {
                return Err(Error::Cancelled);
            }
            files_done += count(batch.len());
            let progress = event(ScanProgress {
                library_id: id,
                files_done,
                files_total,
                ..Default::default()
            });
            let items = self.inner.contents.apply(id, batch).await?;
            self.inner
                .events
                .publish(std::iter::once(progress).chain(items).collect());
        }
        tracing::info!(library = id, ?summary, "scanned a Library");
        self.inner.events.publish(vec![event(ScanFinished {
            library_id: id,
            added: summary.added,
            updated: summary.updated,
            removed: summary.removed,
            unchanged: summary.unchanged,
            ..Default::default()
        })]);
        self.inner.matcher.match_library(scan, &cancel).await?;
        self.probe_in_background(id);
        Ok(summary)
    }

    /// Probes the Library `library`'s files without current Tracks in the background, unless the
    /// Server has no ffprobe.
    fn probe_in_background(&self, library: i64) {
        if !self.inner.ffmpeg.can_probe() {
            return;
        }
        let scanner = self.clone();
        self.inner.tasks.spawn(async move {
            let probing = scanner.probe_stale(library);
            if let Some(Err(err)) = scanner.inner.cancel.run_until_cancelled(probing).await {
                tracing::error!(library, "probing a Library's files failed: {err}");
            }
        });
    }

    /// Probes the Library `library`'s files without current Tracks and stores what each holds.
    /// A file that can't be probed is logged and skipped.
    async fn probe_stale(&self, library: i64) -> Result<(), sqlx::Error> {
        let stored = self.inner.contents.stored(library).await?;
        let files: Vec<Stored> = stored.values().copied().collect();
        let current = self.inner.tracks.current(&files).await?;
        let stale = stored
            .into_iter()
            .filter(|(_, file)| !current.contains(&file.id));
        futures_util::stream::iter(stale)
            .for_each_concurrent(PROBES_AT_ONCE, |(path, file)| self.probe(path, file))
            .await;
        Ok(())
    }

    /// Probes the Version `version` now, without waiting for a background pass to reach it, and
    /// stores and returns what it holds. If something is probing it already, waits for that and
    /// returns what it found instead, or probes it itself if that gave up without finding
    /// anything; if its Tracks are current, returns them.
    pub(crate) async fn probe_now(&self, version: i64) -> Result<Probe, ProbeNowError> {
        let (path, file) = (self.inner.contents.version_file(version).await?)
            .ok_or(ProbeNowError::NoSuchVersion(version))?;
        // ponytail: not held to PROBES_AT_ONCE, so a play press never queues; limit it apart if
        // many at once swamp the disk.
        loop {
            let mut prober = match self.try_claim(file) {
                Ok(claim) => return self.probe_claimed(&path, claim).await,
                Err(prober) => prober,
            };
            // An error means the prober gave up without a result, as when it was cancelled
            // midway, so it's probed here instead.
            if let Ok(found) = prober.wait_for(Option::is_some).await
                && let Some(found) = Option::clone(&found)
            {
                return found.map_err(ProbeNowError::Probe);
            }
        }
    }

    /// Claims the stored file `file`, or if something is probing it, hands back where that will
    /// send what it found.
    fn try_claim(&self, file: Stored) -> Result<Claim<'_>, watch::Receiver<Option<Probed>>> {
        match lock(&self.inner.claimed).entry(file.id) {
            Entry::Occupied(prober) => Err(prober.get().clone()),
            Entry::Vacant(unclaimed) => {
                let (found, waiting) = watch::channel(None);
                unclaimed.insert(waiting);
                Ok(Claim {
                    scanner: self,
                    file,
                    found,
                })
            }
        }
    }

    /// Probes the file at `path`, stored as `file`, and stores what it holds, or logs why not and
    /// forgets what it held before. Leaves it to whoever else is probing it.
    async fn probe(&self, path: String, file: Stored) {
        // The semaphore is never closed.
        let Ok(_running) = self.inner.probing.acquire().await else {
            return;
        };
        // Claimed only now, so `probe_now` doesn't wait for a file still queued here.
        let Ok(claim) = self.try_claim(file) else {
            return;
        };
        match self.probe_claimed(&path, claim).await {
            Ok(_) => {}
            Err(ProbeNowError::Database(err)) => {
                tracing::error!("could not read or store what {path} holds: {err}");
            }
            Err(err) => tracing::warn!("could not probe {path}: {err}"),
        }
    }

    /// Probes the file at `path`, stored as the file `claim` is for, like
    /// [`Self::probe_and_store`], and sends what it found to whoever waited for it.
    async fn probe_claimed(&self, path: &str, claim: Claim<'_>) -> Result<Probe, ProbeNowError> {
        let probed = self.probe_and_store(path, claim.file).await;
        let found = match &probed {
            Ok(probe) => Ok(probe.clone()),
            Err(ProbeNowError::Probe(err)) => Err(Arc::clone(err)),
            // Nothing was found, so a waiter probes it itself.
            Err(ProbeNowError::NoSuchVersion(_) | ProbeNowError::Database(_)) => return probed,
        };
        claim.found.send_replace(Some(found));
        probed
    }

    /// Probes the file at `path`, stored as `file`, and stores and returns what it holds, unless
    /// its Tracks became current since whoever claimed it found them stale. Forgets what it held
    /// before if ffprobe can't read it.
    async fn probe_and_store(&self, path: &str, file: Stored) -> Result<Probe, ProbeNowError> {
        if self.inner.tracks.current(&[file]).await?.contains(&file.id)
            && let Some(probe) = self.inner.tracks.of(&[file.id]).await?.remove(&file.id)
        {
            return Ok(probe);
        }
        let probe = match self.inner.ffmpeg.probe(path).await {
            Ok(probe) => probe,
            // Nothing was read, so what's stored isn't known to be wrong, and a Server without
            // ffprobe keeps it, as it never probes in the background.
            Err(ProbeError::Unavailable) => return Err(ProbeError::Unavailable.into()),
            Err(err) => {
                self.inner.tracks.forget(file.id).await?;
                return Err(err.into());
            }
        };
        self.inner.tracks.store(file, probe.clone()).await?;
        Ok(probe)
    }
}

/// The right to probe one stored file, held by one task at a time and given up when dropped,
/// even by a probe cancelled midway.
#[derive(Debug)]
struct Claim<'a> {
    scanner: &'a Scanner,
    file: Stored,
    /// Where what probing found goes to whoever waits for it. Dropped with nothing sent, it
    /// tells them to probe the file themselves.
    found: watch::Sender<Option<Probed>>,
}

impl Drop for Claim<'_> {
    fn drop(&mut self) {
        lock(&self.scanner.inner.claimed).remove(&self.file.id);
    }
}

/// Locks `mutex`, which no code panics while holding.
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[derive(Debug)]
struct Found {
    path: String,
    /// The path relative to the Library folder it was found in, which is what's parsed.
    relative: PathBuf,
    size: i64,
    modified_ns: i64,
}

#[derive(Debug, Default)]
struct Walked {
    files: Vec<Found>,
    /// Folders and files that couldn't be read. What's stored under them is kept.
    unreadable: Vec<PathBuf>,
}

/// Blocks, so call it from `spawn_blocking`.
fn walk(folders: &[String], cancel: &CancellationToken) -> Result<Walked, Error> {
    let mut walked = Walked::default();
    for folder in folders {
        let root = Path::new(folder);
        let entries = WalkDir::new(root)
            .follow_links(true)
            .into_iter()
            .filter_entry(|entry| entry.depth() == 0 || !is_hidden(entry.file_name()));
        for entry in entries {
            if cancel.is_cancelled() {
                return Err(Error::Cancelled);
            }
            match entry.and_then(|entry| video_file(root, &entry)) {
                Ok(Some(file)) => walked.files.push(file),
                Ok(None) => {}
                Err(err) => {
                    let path = err.path().unwrap_or(root).to_owned();
                    tracing::warn!("could not read {}: {err}", path.display());
                    walked.unreadable.push(path);
                }
            }
        }
    }
    Ok(walked)
}

/// Whether a file or folder is hidden: dot files, and the thumbnails Synology NASes leave.
fn is_hidden(name: &OsStr) -> bool {
    name.as_encoded_bytes().starts_with(b".") || name == "@eaDir"
}

fn video_file(root: &Path, entry: &DirEntry) -> Result<Option<Found>, walkdir::Error> {
    let path = entry.path();
    let is_video = path
        .extension()
        .and_then(OsStr::to_str)
        .is_some_and(|ext| VIDEO_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()));
    if !entry.file_type().is_file() || !is_video {
        return Ok(None);
    }
    let Ok(relative) = path.strip_prefix(root) else {
        return Ok(None);
    };
    if names::is_extra(relative) {
        return Ok(None);
    }
    let Some(utf8) = path.to_str() else {
        tracing::warn!("skipped {}: its path isn't valid UTF-8", path.display());
        return Ok(None);
    };
    let metadata = entry.metadata()?;
    Ok(Some(Found {
        path: utf8.to_owned(),
        relative: relative.to_owned(),
        size: i64::try_from(metadata.len()).unwrap_or(i64::MAX),
        modified_ns: metadata.modified().map_or(0, nanos_since_epoch),
    }))
}

/// Nanoseconds since the Unix epoch, saturating.
fn nanos_since_epoch(time: SystemTime) -> i64 {
    match time.duration_since(UNIX_EPOCH) {
        Ok(after) => i64::try_from(after.as_nanos()).unwrap_or(i64::MAX),
        Err(before) => i64::try_from(before.duration().as_nanos()).map_or(i64::MIN, |n| -n),
    }
}

fn plan(kind: Kind, mut stored: HashMap<String, Stored>, found: Walked) -> (Vec<Change>, Summary) {
    let mut changes = Vec::new();
    let mut summary = Summary::default();
    // A file under two of the Library's folders, one inside the other, is found twice.
    let mut seen = HashSet::new();
    let parse = |relative: &Path| names::parse(kind, relative).unwrap_or_default();
    for file in found.files {
        if !seen.insert(file.path.clone()) {
            continue;
        }
        match stored.remove(&file.path) {
            Some(old) if old.size == file.size && old.modified_ns == file.modified_ns => {
                if old.parser_version < PARSER_VERSION {
                    summary.updated += 1;
                    changes.push(Change::Reparse {
                        file: old.id,
                        items: parse(&file.relative),
                        path: file.path,
                    });
                } else {
                    summary.unchanged += 1;
                }
                continue;
            }
            Some(_) => summary.updated += 1,
            None => summary.added += 1,
        }
        changes.push(Change::Store {
            items: parse(&file.relative),
            path: file.path,
            size: file.size,
            modified_ns: file.modified_ns,
        });
    }
    for (path, old) in stored {
        let path = Path::new(&path);
        if found.unreadable.iter().any(|dir| path.starts_with(dir)) {
            summary.unchanged += 1;
        } else {
            summary.removed += 1;
            changes.push(Change::Remove(old.id));
        }
    }
    (changes, summary)
}

#[cfg(test)]
impl Scanner {
    /// Scans the Library `id` now, after any scan of it that's running, and returns what
    /// changed.
    pub(crate) async fn scan(&self, id: i64) -> Result<Summary, Error> {
        let scan = self.lock(id).await;
        self.scan_locked(&scan).await
    }

    /// Waits until no background scan is running or waiting.
    async fn idle(&self) {
        self.inner.tasks.close();
        self.inner.tasks.wait().await;
        self.inner.tasks.reopen();
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, fs, path::PathBuf, sync::Arc, time::Duration};

    use futures_util::StreamExt;
    use nama_plugin_api::{
        PluginId,
        metadata::{ExternalId, ExternalIds},
    };
    use nama_proto::nama::v1::{Lagged, subscribe_response};
    use tempfile::TempDir;

    use super::{PROBES_AT_ONCE, ProbeNowError, Scanner, Summary};
    use crate::{
        config::Config,
        contents::{Change, Contents, Decided, File, Item, Metadata},
        db::Database,
        events::{EventBus, Filter},
        ffmpeg::{self, Ffmpeg, FoundProgram, ProbeError},
        libraries::{self, Kind, Libraries},
        playback,
        plugins::Registry,
        server_settings::ServerUrl,
        tracks::{Hdr, PROBE_VERSION, Track, TrackKind, Tracks},
    };

    /// How many events a [`Fixture`]'s subscribers may fall behind.
    const BUFFERED: usize = 8;

    /// A database, a Library of `kind` backed by one scratch folder, and a scanner publishing on
    /// a bus whose subscribers lag once [`BUFFERED`] events behind.
    struct Fixture {
        scanner: Scanner,
        contents: Contents,
        tracks: Tracks,
        events: EventBus,
        library: i64,
        root: PathBuf,
        _media: TempDir,
        _data: TempDir,
    }

    impl Fixture {
        /// With no ffprobe, so nothing is probed.
        async fn new(kind: Kind) -> Self {
            Self::with_ffmpeg(kind, Ffmpeg::default()).await
        }

        async fn with_ffmpeg(kind: Kind, ffmpeg: Ffmpeg) -> Self {
            let data = tempfile::tempdir().unwrap();
            let media = tempfile::tempdir().unwrap();
            let root = media.path().canonicalize().unwrap().join("library");
            fs::create_dir(&root).unwrap();
            let db = Arc::new(Database::open(data.path()).await.unwrap());
            let library = Libraries::new(Arc::clone(&db))
                .create("Media", kind, vec![root.to_str().unwrap().to_owned()])
                .await
                .unwrap();
            let events = EventBus::with_capacity(
                Arc::clone(&db),
                Registry::new(),
                ServerUrl::default(),
                BUFFERED,
            );
            Self {
                scanner: Scanner::new(
                    Arc::clone(&db),
                    Registry::new(),
                    events.clone(),
                    Arc::new(ffmpeg),
                ),
                tracks: Tracks::new(Arc::clone(&db)),
                contents: Contents::new(db),
                events,
                library: library.id,
                root,
                _media: media,
                _data: data,
            }
        }

        fn write(&self, relative: &str, size: usize) {
            let path = self.root.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, vec![0_u8; size]).unwrap();
        }

        fn path(&self, relative: &str) -> PathBuf {
            self.root.join(relative)
        }

        async fn scan(&self) -> Summary {
            self.scanner.scan(self.library).await.unwrap()
        }

        /// Scans, then waits for the probing it starts.
        async fn scan_and_probe(&self) {
            self.scan().await;
            self.scanner.idle().await;
        }

        /// The channels of the one audio Track stored for the file at `relative`.
        #[cfg(unix)]
        async fn channels(&self, relative: &str) -> Option<i64> {
            let version = self.file_id(relative).await;
            let probe = self.tracks.of(&[version]).await.unwrap().remove(&version)?;
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

        /// The id of the file stored at `relative`.
        async fn file_id(&self, relative: &str) -> i64 {
            let path = self.path(relative);
            self.contents.stored(self.library).await.unwrap()[path.to_str().unwrap()].id
        }

        /// Stores the file at `relative` as holding Alien, as a scan would but without probing it.
        async fn store_alien(&self, relative: &str) {
            let file = Change::Store {
                path: self.path(relative).to_str().unwrap().to_owned(),
                size: 10,
                modified_ns: 0,
                items: movie("Alien", 1979, None),
            };
            self.contents.apply(self.library, vec![file]).await.unwrap();
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

        /// The stored files, as `/`-separated paths relative to the Library folder, whatever
        /// separator the platform uses.
        async fn files(&self) -> Vec<(String, Vec<Item>)> {
            self.contents
                .files(self.library)
                .await
                .unwrap()
                .into_iter()
                .map(|File { path, items, .. }| {
                    let path = PathBuf::from(path);
                    let relative = path.strip_prefix(&self.root).unwrap();
                    let parts: Vec<&str> = relative
                        .components()
                        .map(|part| part.as_os_str().to_str().unwrap())
                        .collect();
                    let items = items.into_iter().map(|stored| stored.item).collect();
                    (parts.join("/"), items)
                })
                .collect()
        }
    }

    fn movie(title: &str, year: i64, hint: Option<ExternalId>) -> Vec<Item> {
        vec![Item::Movie {
            title: title.to_owned(),
            year: Some(year),
            hint,
        }]
    }

    fn summary(added: u64, updated: u64, removed: u64, unchanged: u64) -> Summary {
        Summary {
            added,
            updated,
            removed,
            unchanged,
        }
    }

    #[tokio::test]
    async fn scanning_a_movies_library_stores_its_video_files_and_what_they_hold() {
        let media = Fixture::new(Kind::Movies).await;
        media.write("Blade Runner (1982)/Blade Runner (1982).mkv", 10);
        media.write("Alien (1979)/Alien (1979).MP4", 10);
        // Not video, an extra, hidden, and a name that doesn't parse.
        media.write("Alien (1979)/Alien (1979).srt", 10);
        media.write("Alien (1979)/Alien (1979).nfo", 10);
        media.write("Alien (1979)/Featurettes/Making Alien.mkv", 10);
        media.write("Alien (1979)/Alien (1979)-trailer.mkv", 10);
        media.write(".trash/Heat (1995).mkv", 10);
        media.write("Alien (1979)/._Alien (1979).mkv", 10);
        media.write("home video.mkv", 10);

        assert_eq!(media.scan().await, summary(3, 0, 0, 0));
        assert_eq!(
            media.files().await,
            [
                (
                    "Alien (1979)/Alien (1979).MP4".to_owned(),
                    movie("Alien", 1979, None)
                ),
                (
                    "Blade Runner (1982)/Blade Runner (1982).mkv".to_owned(),
                    movie("Blade Runner", 1982, None)
                ),
                // Stored, but Unmatched: there's nothing to Match it by.
                ("home video.mkv".to_owned(), vec![]),
            ]
        );
    }

    #[tokio::test]
    async fn scanning_a_shows_library_stores_an_item_per_episode() {
        let media = Fixture::new(Kind::Shows).await;
        media.write("Dark (2017)/Season 01/Dark (2017) - S01E01-E02.mkv", 10);
        media.write("Dark (2017)/Season 00/Dark (2017) - S00E01.mkv", 10);

        assert_eq!(media.scan().await, summary(2, 0, 0, 0));
        let episode = |season, episode| Item::Episode {
            show: "Dark".to_owned(),
            year: Some(2017),
            hint: None,
            season,
            episode,
        };
        assert_eq!(
            media.files().await,
            [
                (
                    "Dark (2017)/Season 00/Dark (2017) - S00E01.mkv".to_owned(),
                    vec![episode(0, 1)]
                ),
                (
                    "Dark (2017)/Season 01/Dark (2017) - S01E01-E02.mkv".to_owned(),
                    vec![episode(1, 1), episode(1, 2)]
                ),
            ]
        );
    }

    #[tokio::test]
    async fn a_rescan_skips_unchanged_files_and_picks_up_added_changed_renamed_and_deleted_ones() {
        let media = Fixture::new(Kind::Movies).await;
        media.write("Blade Runner (1982)/Blade Runner (1982).mkv", 10);
        media.write("Alien (1979)/Alien (1979).mkv", 10);
        media.write("Heat (1995)/Heat (1995).mkv", 10);
        media.write("home video.mkv", 10);
        assert_eq!(media.scan().await, summary(4, 0, 0, 0));
        assert_eq!(media.scan().await, summary(0, 0, 0, 4), "nothing changed");

        media.write("Up (2009)/Up (2009).mkv", 10);
        media.write("Alien (1979)/Alien (1979).mkv", 20);
        // Renamed, with an ID hint.
        fs::rename(
            media.path("Blade Runner (1982)"),
            media.path("Blade Runner (1982) {tmdb-78}"),
        )
        .unwrap();
        fs::remove_file(media.path("home video.mkv")).unwrap();

        assert_eq!(media.scan().await, summary(2, 1, 2, 1));
        assert_eq!(
            media.files().await,
            [
                (
                    "Alien (1979)/Alien (1979).mkv".to_owned(),
                    movie("Alien", 1979, None)
                ),
                (
                    "Blade Runner (1982) {tmdb-78}/Blade Runner (1982).mkv".to_owned(),
                    movie("Blade Runner", 1982, Some(ExternalId::Tmdb(78)))
                ),
                (
                    "Heat (1995)/Heat (1995).mkv".to_owned(),
                    movie("Heat", 1995, None)
                ),
                (
                    "Up (2009)/Up (2009).mkv".to_owned(),
                    movie("Up", 2009, None)
                ),
            ]
        );
    }

    #[tokio::test]
    async fn a_newer_name_parser_parses_unchanged_files_again_once() {
        let media = Fixture::new(Kind::Movies).await;
        media.write("Alien (1979)/Alien (1979).mkv", 10);
        media.write("home video.mkv", 10);
        media.scan().await;

        media.contents.read_by_older_parser(media.library).await;

        assert_eq!(media.scan().await, summary(0, 2, 0, 0));
        assert_eq!(media.scan().await, summary(0, 0, 0, 2));
        assert_eq!(media.files().await[0].1, movie("Alien", 1979, None));
    }

    #[tokio::test]
    async fn files_in_a_folder_that_cant_be_read_are_kept() {
        let media = Fixture::new(Kind::Movies).await;
        media.write("Alien (1979)/Alien (1979).mkv", 10);
        assert_eq!(media.scan().await, summary(1, 0, 0, 0));

        // The Library folder is gone, as when a drive isn't mounted.
        let parked = media.root.with_file_name("parked");
        fs::rename(&media.root, &parked).unwrap();
        assert_eq!(media.scan().await, summary(0, 0, 0, 1));
        assert_eq!(
            media.files().await.len(),
            1,
            "an unreadable folder emptied the Library"
        );
        fs::rename(&parked, &media.root).unwrap();

        assert_eq!(media.scan().await, summary(0, 0, 0, 1));
    }

    #[tokio::test]
    async fn a_scan_started_in_the_background_stores_the_files() {
        let media = Fixture::new(Kind::Movies).await;
        media.write("Alien (1979)/Alien (1979).mkv", 10);

        media.scanner.start(media.library).await.unwrap();
        // Asking again while the first is queued or running changes nothing it finds.
        media.scanner.start(media.library).await.unwrap();
        media.scanner.idle().await;

        assert_eq!(
            media.files().await,
            [(
                "Alien (1979)/Alien (1979).mkv".to_owned(),
                movie("Alien", 1979, None)
            )]
        );
    }

    #[tokio::test]
    async fn every_library_is_scanned_when_asked_to_scan_all() {
        let media = Fixture::new(Kind::Movies).await;
        media.write("Alien (1979)/Alien (1979).mkv", 10);

        media.scanner.start_all().await;
        media.scanner.idle().await;

        assert_eq!(media.files().await.len(), 1);
    }

    #[tokio::test]
    async fn starting_a_scan_of_a_library_that_doesnt_exist_fails() {
        let media = Fixture::new(Kind::Movies).await;
        let result = media.scanner.start(media.library + 1).await;
        assert!(
            matches!(result, Err(libraries::Error::NotFound(_))),
            "{result:?}"
        );
    }

    #[tokio::test]
    async fn shutting_down_stops_scans_and_starts_no_more() {
        let media = Fixture::new(Kind::Movies).await;
        media.write("Alien (1979)/Alien (1979).mkv", 10);
        media.scanner.start(media.library).await.unwrap();
        media.scanner.shutdown().await;

        let before = media.files().await.len();
        media.scanner.start(media.library).await.unwrap();
        media.scanner.idle().await;
        assert_eq!(
            media.files().await.len(),
            before,
            "a scan ran after shutdown"
        );
    }

    #[tokio::test]
    async fn a_subscriber_that_falls_behind_lags_while_the_scan_finishes_on_time() {
        let media = Fixture::new(Kind::Movies).await;
        for n in 0..20 {
            media.write(&format!("Movie {n} (2000)/Movie {n} (2000).mkv"), 10);
        }
        // Never read while the scan runs.
        let mut slow = Box::pin(media.events.subscribe(Filter::default()));

        let scanned =
            tokio::time::timeout(Duration::from_secs(5), media.scanner.scan(media.library)).await;
        assert!(matches!(scanned, Ok(Ok(_))), "{scanned:?}");

        // The scan published 23 events: its start, one batch's progress, 20 Items and its finish.
        // Only the last 8 were kept.
        let lagged = Lagged {
            dropped: 15,
            ..Default::default()
        };
        assert_eq!(
            slow.next().await.and_then(|response| response.message),
            Some(subscribe_response::Message::from(lagged))
        );
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

    #[tokio::test]
    async fn a_scan_probes_a_4k_hdr10_file_with_5_1_audio_and_stores_its_tracks() {
        let media = Fixture::with_ffmpeg(Kind::Movies, system_ffmpeg()).await;
        let alien = "Alien (1979)/Alien (1979).mkv";
        fs::create_dir_all(media.path("Alien (1979)")).unwrap();
        ffmpeg::write_4k_with_5_1(&media.path(alien), "smpte2084").unwrap();

        media.scan_and_probe().await;

        let version = media.file_id(alien).await;
        let mut probes = media.tracks.of(&[version]).await.unwrap();
        let probe = probes.remove(&version).expect("the file wasn't probed");
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
        assert_eq!(
            media.tracks.probe_version_of(version).await,
            Some(PROBE_VERSION)
        );
    }

    #[tokio::test]
    async fn a_scan_records_an_hlg_file_as_hlg_and_an_sdr_file_as_no_hdr() {
        let media = Fixture::with_ffmpeg(Kind::Movies, system_ffmpeg()).await;
        let hlg = "Alien (1979)/Alien (1979).mkv";
        let sdr = "Heat (1995)/Heat (1995).mkv";
        for (file, transfer) in [(hlg, "arib-std-b67"), (sdr, "bt709")] {
            fs::create_dir_all(media.path(file).parent().unwrap()).unwrap();
            ffmpeg::write_4k_with_5_1(&media.path(file), transfer).unwrap();
        }

        media.scan().await;
        media.scanner.idle().await;

        let versions = [media.file_id(hlg).await, media.file_id(sdr).await];
        let mut probes = media.tracks.of(&versions).await.unwrap();
        let mut hdr_of = |version| match probes.remove(&version).map(|probe| probe.tracks) {
            Some(tracks) => match tracks.first().map(|track| &track.kind) {
                Some(TrackKind::Video { hdr, .. }) => *hdr,
                _ => panic!("no video Track first: {tracks:?}"),
            },
            None => panic!("the file wasn't probed"),
        };
        assert_eq!(hdr_of(versions[0]), Some(Hdr::Hlg));
        assert_eq!(hdr_of(versions[1]), None);
    }

    #[tokio::test]
    async fn without_ffprobe_a_scan_stores_no_tracks() {
        let media = Fixture::new(Kind::Movies).await;
        let alien = "Alien (1979)/Alien (1979).mkv";
        media.write(alien, 10);

        assert_eq!(media.scan().await, summary(1, 0, 0, 0));
        media.scanner.idle().await;

        let version = media.file_id(alien).await;
        assert!(media.tracks.of(&[version]).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn probing_an_unprobed_version_right_away_returns_and_stores_its_tracks() {
        let media = Fixture::with_ffmpeg(Kind::Movies, system_ffmpeg()).await;
        let alien = "Alien (1979)/Alien (1979).mkv";
        fs::create_dir_all(media.path("Alien (1979)")).unwrap();
        ffmpeg::write_4k_with_5_1(&media.path(alien), "smpte2084").unwrap();
        media.store_alien(alien).await;
        media.match_to_alien().await;
        let version = media.file_id(alien).await;

        let probe = media.scanner.probe_now(version).await.unwrap();

        let kinds: Vec<_> = probe.tracks.iter().map(|track| &track.kind).collect();
        assert_eq!(
            kinds,
            [
                &TrackKind::Video {
                    width: Some(3840),
                    height: Some(2160),
                    frame_rate: Some(24.0),
                    bit_depth: Some(10),
                    hdr: Some(Hdr::Hdr10),
                },
                &TrackKind::Audio { channels: Some(6) },
            ]
        );
        assert_eq!(
            media.tracks.of(&[version]).await.unwrap().remove(&version),
            Some(probe)
        );
    }

    #[tokio::test]
    async fn without_ffprobe_probing_a_version_right_away_reports_that_probing_is_unavailable() {
        let media = Fixture::new(Kind::Movies).await;
        let alien = "Alien (1979)/Alien (1979).mkv";
        media.store_alien(alien).await;
        media.match_to_alien().await;

        let probed = media.scanner.probe_now(media.file_id(alien).await).await;

        assert!(
            matches!(&probed, Err(ProbeNowError::Probe(err)) if matches!(**err, ProbeError::Unavailable)),
            "{probed:?}"
        );
    }

    /// An ffprobe in `bin` that runs `script`, a shell script.
    #[cfg(unix)]
    fn fake_ffprobe(bin: &std::path::Path, script: &str) -> Ffmpeg {
        use std::os::unix::fs::PermissionsExt;

        let path = bin.join("ffprobe");
        fs::write(&path, format!("#!/bin/sh\n{script}\n")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        Ffmpeg {
            ffprobe: Ok(FoundProgram {
                path,
                version: "7.1.1".to_owned(),
            }),
            ..Ffmpeg::default()
        }
    }

    // The fake ffprobe is a shell script. The Version is stored without a scan, so no background
    // pass probes it.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_version_without_tracks_is_probed_before_its_playback_plan_is_built() {
        let bin = tempfile::tempdir().unwrap();
        let ffmpeg = fake_ffprobe(
            bin.path(),
            r#"cat <<'EOF'
{"streams": [{"codec_name": "h264", "codec_type": "video"},
             {"codec_name": "aac", "codec_type": "audio"}],
 "format": {"format_name": "mov,mp4,m4a,3gp,3g2,mj2"}}
EOF"#,
        );
        let media = Fixture::with_ffmpeg(Kind::Movies, ffmpeg).await;
        let alien = "Alien (1979)/Alien (1979).mp4";
        media.store_alien(alien).await;
        media.match_to_alien().await;
        let version = media.file_id(alien).await;

        let device = playback::DeviceDecoding::default();
        let plan = playback::plan(&media.scanner, version, device, BTreeSet::new())
            .await
            .unwrap();

        let tracks = plan.map(|plan| plan.tracks).unwrap_or_default();
        let codecs: Vec<&str> = tracks.iter().map(|track| track.codec.as_str()).collect();
        assert_eq!(codecs, ["h264", "aac"]);
    }

    // ffmpeg can't write Dolby Vision, so a fake ffprobe prints what a real one does for it, cut
    // down to what probing reads. The fake is a shell script.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_scan_stores_a_dolby_vision_files_profile_and_whether_its_base_layer_is_hdr10() {
        let bin = tempfile::tempdir().unwrap();
        let ffmpeg = fake_ffprobe(
            bin.path(),
            r#"cat <<'EOF'
{"streams": [{"codec_name": "hevc", "codec_type": "video", "pix_fmt": "yuv420p10le",
  "color_transfer": "smpte2084",
  "side_data_list": [{"side_data_type": "DOVI configuration record", "dv_profile": 8,
                      "dv_bl_signal_compatibility_id": 1}]}],
 "format": {"format_name": "matroska,webm"}}
EOF"#,
        );
        let media = Fixture::with_ffmpeg(Kind::Movies, ffmpeg).await;
        let alien = "Alien (1979)/Alien (1979).mkv";
        media.write(alien, 10);

        media.scan().await;
        media.scanner.idle().await;

        let version = media.file_id(alien).await;
        let probe = media.tracks.of(&[version]).await.unwrap().remove(&version);
        let kind = probe
            .and_then(|probe| probe.tracks.into_iter().next())
            .map(|t| t.kind);
        assert_eq!(
            kind,
            Some(TrackKind::Video {
                width: None,
                height: None,
                frame_rate: None,
                bit_depth: Some(10),
                hdr: Some(Hdr::DolbyVision {
                    profile: 8,
                    hdr10_base_layer: true,
                }),
            })
        );
    }

    // The fake ffprobe is a shell script.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_scan_finishes_without_waiting_for_probing_and_shutdown_stops_the_probes() {
        let bin = tempfile::tempdir().unwrap();
        let ffmpeg = fake_ffprobe(bin.path(), "exec sleep 60");
        let media = Fixture::with_ffmpeg(Kind::Movies, ffmpeg).await;
        media.write("Alien (1979)/Alien (1979).mkv", 10);

        let scanned = tokio::time::timeout(Duration::from_secs(5), media.scan()).await;
        assert_eq!(scanned, Ok(summary(1, 0, 0, 0)));

        let stopped = tokio::time::timeout(Duration::from_secs(5), media.scanner.shutdown()).await;
        assert!(stopped.is_ok(), "shutting down waited for ffprobe");
    }

    /// An ffprobe in `bin` that adds a line to `runs` each time it runs, then takes a second to
    /// find a Matroska file with no Tracks.
    #[cfg(unix)]
    fn slow_ffprobe(bin: &std::path::Path, runs: &std::path::Path) -> Ffmpeg {
        fake_ffprobe(
            bin,
            &format!(
                "echo run >> '{}'\nsleep 1\necho '{{\"format\": {{\"format_name\": \"matroska,webm\"}}}}'",
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
    async fn a_scan_during_probing_leaves_the_files_being_probed_to_it() {
        let bin = tempfile::tempdir().unwrap();
        let runs = bin.path().join("runs");
        let media = Fixture::with_ffmpeg(Kind::Movies, slow_ffprobe(bin.path(), &runs)).await;
        let alien = "Alien (1979)/Alien (1979).mkv";
        media.write(alien, 10);

        media.scan().await;
        wait_for_probe(&runs).await;
        media.scan_and_probe().await;

        assert_eq!(fs::read_to_string(&runs).unwrap(), "run\n");
        let version = media.file_id(alien).await;
        assert!(
            media
                .tracks
                .of(&[version])
                .await
                .unwrap()
                .contains_key(&version)
        );
    }

    // The fake ffprobe is a shell script.
    #[cfg(unix)]
    #[tokio::test]
    async fn probing_a_version_right_away_while_the_background_probes_it_runs_ffprobe_once() {
        let bin = tempfile::tempdir().unwrap();
        let runs = bin.path().join("runs");
        let media = Fixture::with_ffmpeg(Kind::Movies, slow_ffprobe(bin.path(), &runs)).await;
        let alien = "Alien (1979)/Alien (1979).mkv";
        media.write(alien, 10);
        media.scan().await;
        media.match_to_alien().await;
        wait_for_probe(&runs).await;

        let probe = media
            .scanner
            .probe_now(media.file_id(alien).await)
            .await
            .unwrap();

        assert_eq!(probe.container, "matroska,webm");
        media.scanner.idle().await;
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
        let media = Fixture::with_ffmpeg(Kind::Movies, ffmpeg).await;
        let alien = "Alien (1979)/Alien (1979).mkv";
        media.write(alien, 10);
        media.scan().await;
        media.match_to_alien().await;
        wait_for_probe(&runs).await;

        let probed = media.scanner.probe_now(media.file_id(alien).await).await;

        assert!(
            matches!(&probed, Err(ProbeNowError::Probe(err)) if matches!(**err, ProbeError::Failed(_))),
            "{probed:?}"
        );
        media.scanner.idle().await;
        assert_eq!(fs::read_to_string(&runs).unwrap(), "run\n");
    }

    // The fake ffprobe is a shell script.
    #[cfg(unix)]
    #[tokio::test]
    async fn probing_a_version_right_away_doesnt_wait_for_the_background_pass_to_reach_it() {
        let bin = tempfile::tempdir().unwrap();
        let runs = bin.path().join("runs");
        let media = Fixture::with_ffmpeg(Kind::Movies, slow_ffprobe(bin.path(), &runs)).await;
        let alien = "Alien (1979)/Alien (1979).mkv";
        media.write(alien, 10);
        // The background pass is busy with other files.
        let busy = (media.scanner.inner.probing)
            .acquire_many(u32::try_from(PROBES_AT_ONCE).unwrap())
            .await
            .unwrap();
        media.scan().await;
        media.match_to_alien().await;

        let probed = tokio::time::timeout(
            Duration::from_secs(5),
            media.scanner.probe_now(media.file_id(alien).await),
        )
        .await;

        assert!(matches!(probed, Ok(Ok(_))), "{probed:?}");
        drop(busy);
        media.scanner.idle().await;
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
echo "{{\"format\": {{\"format_name\": \"matroska,webm\"}}, \"streams\": [{{\"codec_type\": \"audio\", \"codec_name\": \"ac3\", \"channels\": $(wc -c < "$file")}}]}}""#,
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
    async fn an_unchanged_file_is_probed_again_only_once_the_probe_version_is_bumped() {
        let bin = tempfile::tempdir().unwrap();
        let media = Fixture::with_ffmpeg(Kind::Movies, counting_ffprobe(bin.path())).await;
        let alien = "Alien (1979)/Alien (1979).mkv";
        media.write(alien, 10);
        media.scan_and_probe().await;
        media.scan_and_probe().await;
        assert_eq!(
            probe_runs(bin.path()),
            1,
            "an unchanged file was probed again"
        );

        let version = media.file_id(alien).await;
        media.tracks.read_by_older_probe(version).await;
        media.scan_and_probe().await;

        assert_eq!(probe_runs(bin.path()), 2);
        assert_eq!(
            media.tracks.probe_version_of(version).await,
            Some(PROBE_VERSION)
        );
        media.scan_and_probe().await;
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
        let media = Fixture::with_ffmpeg(Kind::Movies, counting_ffprobe(bin.path())).await;
        let alien = "Alien (1979)/Alien (1979).mkv";
        media.write(alien, 10);
        media.scan_and_probe().await;
        assert_eq!(media.channels(alien).await, Some(10));

        media.write(alien, 20);
        media.scan_and_probe().await;

        assert_eq!(probe_runs(bin.path()), 2);
        assert_eq!(media.channels(alien).await, Some(20));
    }

    // The fake ffprobe is a shell script.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_file_touched_without_changing_size_is_probed_again() {
        let bin = tempfile::tempdir().unwrap();
        let media = Fixture::with_ffmpeg(Kind::Movies, counting_ffprobe(bin.path())).await;
        let alien = "Alien (1979)/Alien (1979).mkv";
        media.write(alien, 10);
        media.scan_and_probe().await;

        let touched = std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        fs::File::options()
            .write(true)
            .open(media.path(alien))
            .unwrap()
            .set_modified(touched)
            .unwrap();
        media.scan_and_probe().await;

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
echo '{"format": {"format_name": "matroska,webm"}, "streams": [{"codec_type": "audio", "codec_name": "ac3", "channels": 2}]}'"#,
        );
        let media = Fixture::with_ffmpeg(Kind::Movies, ffmpeg).await;
        let alien = "Alien (1979)/Alien (1979).mkv";
        media.write(alien, 10);
        media.scan_and_probe().await;
        assert_eq!(media.channels(alien).await, Some(2));

        media.write(alien, 20);
        media.scan_and_probe().await;

        let version = media.file_id(alien).await;
        assert!(media.tracks.of(&[version]).await.unwrap().is_empty());
    }
}
