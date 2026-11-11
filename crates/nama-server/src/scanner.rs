//! The scanner: walks each Library's folders and stores the video files in them, and what their
//! names say they hold (see [`names`]).
//!
//! A rescan is incremental. A file whose size and modified time are what was stored is skipped,
//! unless an older version of the name parser read it (ADR 0023); a new or changed file is parsed
//! and stored; a stored file that's gone is removed, except under a folder that couldn't be read,
//! so a drive that isn't mounted doesn't empty the Library. A renamed file is a file gone and a
//! file found. Changes are written in batches of [`BATCH_SIZE`] (see the crate's `AGENTS.md`).
//! Once the Server is serving, a Library is also scanned when its folders change (see [`watch`]).
//!
//! Files are only listed and their metadata read, never opened: sidecars such as `.nfo` files
//! are ignored, except that deleting media deletes them too (see [`deleting`]).
//!
//! A scan publishes its start, then after each batch is written its progress and the Items it
//! added and removed, then its finish, then once Matching has decided what it could
//! `MatchingFinished`, then once Probing has caught up `ProbingFinished` (see [`crate::events`]).
//!
//! After each scan the Library's items are Matched (see [`crate::matching`]), still under the
//! Library's scan lock, so Matching sees the items the scan stored. Then [`crate::probing`] is told
//! its files changed, and probes them in the background without holding up the next scan.
//!
//! Probing passes of one Library overlap, so `ProbingFinished` says the Library has caught up, not
//! that one pass is done (ADR 0030): a pass waits for the files it left to another prober, and
//! one whose Probing ends after a later scan of the Library started sends nothing.

pub(crate) mod deleting;
mod names;
mod watch;

pub(crate) use deleting::{Deleted, Doomed};

pub(crate) use names::{PARSER_VERSION, country_tag};

use std::{
    collections::{HashMap, HashSet},
    ffi::OsStr,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, PoisonError},
    time::{SystemTime, UNIX_EPOCH},
};

use nama_proto::nama::v1::{
    MatchingFinished, ProbingFinished, ScanFinished, ScanProgress, ScanStarted,
};
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use walkdir::{DirEntry, WalkDir};

use crate::{
    contents::{Change, Contents, Stored},
    db::{Database, detached},
    events::{EventBus, event},
    libraries::{self, Kind, Libraries, Library},
    matching::{self, Matcher, count},
    playback::running::RunningPlans,
    plugins::ExtensionPoints,
    probing::Probing,
};

/// Extensions, in lowercase, of the files the scanner treats as video.
const VIDEO_EXTENSIONS: &[&str] = &[
    "3gp", "avi", "divx", "flv", "m2ts", "m4v", "mkv", "mov", "mp4", "mpeg", "mpg", "mts", "ogm",
    "ogv", "ts", "webm", "wmv",
];

/// How many changed files one write stores or removes. Small enough that other writes don't
/// wait long behind a scan.
const BATCH_SIZE: usize = 200;

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

/// Scans Libraries, one scan per Library at a time, either now or in the background, and makes
/// every change to a Library along with what must follow it.
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
    /// Told when a scan finishes and asked to clean subtitles, unless nothing probes.
    probing: Option<Probing>,
    /// Asked what plays media about to be deleted, and to end it, unless nothing plays.
    running: Option<RunningPlans>,
    /// Background scans, so shutting down can wait for them.
    tasks: TaskTracker,
    /// Cancelled when the Server shuts down.
    cancel: CancellationToken,
    /// Libraries with a background scan waiting to start.
    queued: Mutex<HashSet<i64>>,
    /// How many scans of each Library have published `ScanStarted`, so Probing knows whether a
    /// later scan has. Held while publishing either, so none comes out of order.
    started: Mutex<HashMap<i64, u64>>,
    locks: ScanLocks,
    watches: watch::Watches,
}

/// Each Library's scan lock, shared by the [`Scanner`] and its [`Matcher`]. Only
/// [`Scanner::new`] makes one, so no Matcher fixes a Match under locks a scan doesn't take.
#[derive(Debug, Clone)]
pub(crate) struct ScanLocks(Arc<Mutex<HashMap<i64, Arc<tokio::sync::Mutex<()>>>>>);

impl ScanLocks {
    /// Waits for any scan of the Library `id` to finish, then takes its scan lock.
    pub(crate) async fn lock(&self, id: i64) -> ScanLock {
        let mutex = Arc::clone(lock(&self.0).entry(id).or_default());
        ScanLock {
            library: id,
            _guard: mutex.lock_owned().await,
        }
    }
}

/// A Library's scan lock: while it's held, no scan of the Library runs or starts, so the
/// Library's items don't change. Only [`ScanLocks`] makes one.
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
    /// Tells `probing` each Library whose scan finished and has it clean subtitles, or does
    /// neither where nothing probes, and has `running` end the plans playing media it deletes.
    pub(crate) fn new(
        db: Arc<Database>,
        plugins: ExtensionPoints,
        events: EventBus,
        probing: Option<Probing>,
        running: Option<RunningPlans>,
    ) -> Self {
        let locks = ScanLocks(Arc::default());
        Self {
            inner: Arc::new(Inner {
                libraries: Libraries::new(Arc::clone(&db)),
                matcher: Matcher::new(Arc::clone(&db), plugins, events.clone(), locks.clone()),
                events,
                probing,
                running,
                contents: Contents::new(db),
                tasks: TaskTracker::new(),
                cancel: CancellationToken::new(),
                queued: Mutex::default(),
                started: Mutex::default(),
                locks,
                watches: watch::Watches::default(),
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

    /// Creates a Library and watches its folders. The watch follows even if the caller stops
    /// waiting (ADR 0029); scanning the new Library is left to the caller.
    pub(crate) async fn create_library(
        &self,
        name: String,
        kind: Kind,
        folders: Vec<String>,
    ) -> Result<Library, libraries::Error> {
        let scanner = self.clone();
        detached(async move {
            let library = scanner.inner.libraries.create(&name, kind, folders).await?;
            scanner.rewatch().await;
            Ok(library)
        })
        .await
    }

    /// Renames the Library `id` and replaces its folders, then watches its new folders and not its
    /// old ones, even if the caller stops waiting (ADR 0029).
    pub(crate) async fn update_library(
        &self,
        id: i64,
        name: String,
        folders: Vec<String>,
    ) -> Result<Library, libraries::Error> {
        let scanner = self.clone();
        detached(async move {
            let library = scanner.inner.libraries.update(id, &name, folders).await?;
            scanner.rewatch().await;
            Ok(library)
        })
        .await
    }

    /// Deletes the Library `id`, then throws away its Versions' cached subtitles where something
    /// probes, and stops watching its folders, even if the caller stops waiting (ADR 0029).
    pub(crate) async fn delete_library(&self, id: i64) -> Result<(), libraries::Error> {
        let scanner = self.clone();
        detached(async move {
            scanner.inner.libraries.delete(id).await?;
            if let Some(probing) = &scanner.inner.probing {
                probing.clean_subtitles().await;
            }
            scanner.rewatch().await;
            Ok(())
        })
        .await
    }

    /// What [`Self::delete_media`] would delete and stop playing, deleting nothing.
    pub(crate) async fn preview_delete(&self, doomed: Doomed) -> Result<Deleted, Error> {
        let playing = (self.inner.running.as_ref())
            .map_or(0, |running| running.playing(&doomed.version_ids()));
        let planned = tokio::task::spawn_blocking(move || deleting::preview(&doomed)).await?;
        Ok(Deleted { playing, ..planned })
    }

    /// Ends the plans playing `doomed`, then deletes it from disk for good, then removes the
    /// Versions whose files went as a scan removes them, and logs that `by` deleted them. The one
    /// way to delete media (see AGENTS.md, "One path for deleting media"). It holds the Library's
    /// scan lock, so a scan racing it sees either everything or nothing gone, and finishes even if
    /// the caller stops waiting (ADR 0029).
    pub(crate) async fn delete_media(&self, doomed: Doomed, by: String) -> Result<Deleted, Error> {
        let scanner = self.clone();
        detached(async move {
            let library = doomed.library.id;
            let _scan = scanner.inner.locks.lock(library).await;
            // Held until the Versions are gone, so none is served again before.
            let _ending = match &scanner.inner.running {
                Some(running) => Some(running.end_for_deleting(&doomed.version_ids()).await),
                None => None,
            };
            let folders: Vec<String> = (scanner.inner.libraries.list().await?.into_iter())
                .flat_map(|other| other.folders)
                .collect();
            let (doomed, deleted) = tokio::task::spawn_blocking(move || {
                let kept: Vec<&Path> = folders.iter().map(Path::new).collect();
                let deleted = deleting::delete(&doomed, &kept);
                (doomed, deleted)
            })
            .await?;
            // Before anything else can fail: the files are gone for good.
            let files: Vec<_> = deleted.files.iter().map(|file| &file.path).collect();
            tracing::info!(
                by = %by,
                title = %doomed.title,
                library,
                ?files,
                bytes_freed = deleted.bytes_freed(),
                not_deleted = ?deleted.failed,
                "deleted media"
            );
            let removed = deleted.versions.iter().copied().map(Change::Remove);
            let events = scanner
                .inner
                .contents
                .apply(library, removed.collect())
                .await?;
            if let Some(probing) = &scanner.inner.probing {
                probing.clean_subtitles().await;
            }
            scanner.inner.events.publish(events);
            Ok(deleted)
        })
        .await
    }

    /// The Matcher that fixes a Match by hand, waiting for a scan of the item's Library.
    pub(crate) fn matcher(&self) -> &Matcher {
        &self.inner.matcher
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
            let scan = scanner.inner.locks.lock(id).await;
            lock(&scanner.inner.queued).remove(&id);
            match scanner.scan_locked(&scan).await {
                // The Library was deleted after the scan was asked for.
                Ok(_) | Err(Error::Cancelled | Error::Library(libraries::Error::NotFound(_))) => {}
                Err(err) => tracing::error!(library = id, "scanning a Library failed: {err}"),
            }
        });
    }

    async fn scan_locked(&self, scan: &ScanLock) -> Result<Summary, Error> {
        let id = scan.library();
        let cancel = self.inner.cancel.clone();
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let library = self.inner.libraries.get(id).await?;
        let scan_number = self.publish_started(id);
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
        if let Some(probing) = &self.inner.probing {
            // Before `ScanFinished`, so a client that waited for the scan isn't served a gone or
            // changed file's subtitles.
            probing.clean_subtitles().await;
        }
        self.inner.events.publish(vec![event(ScanFinished {
            library_id: id,
            added: summary.added,
            updated: summary.updated,
            removed: summary.removed,
            unchanged: summary.unchanged,
            ..Default::default()
        })]);
        let matched = self.inner.matcher.match_library(scan, &cancel).await?;
        self.inner.events.publish(vec![event(MatchingFinished {
            library_id: id,
            matched: matched.matched,
            unmatched: matched.unmatched,
            undecided: matched.undecided,
            ..Default::default()
        })]);
        match &self.inner.probing {
            Some(probing) => {
                let scanner = self.clone();
                probing.files_changed(id, move |probed| {
                    scanner.publish_probed(scan_number, probed);
                });
            }
            // Nothing probes, so Probing has caught up already.
            None => self.publish_probed(
                scan_number,
                ProbingFinished {
                    library_id: id,
                    ..Default::default()
                },
            ),
        }
        Ok(summary)
    }

    /// Publishes that a scan of the Library `library` started, and returns which scan of it this
    /// is.
    fn publish_started(&self, library: i64) -> u64 {
        let mut started = lock(&self.inner.started);
        let scan_number = started.entry(library).or_default();
        *scan_number += 1;
        self.inner.events.publish(vec![event(ScanStarted {
            library_id: library,
            ..Default::default()
        })]);
        *scan_number
    }

    /// Publishes `probed`, which the scan `scan_number` of its Library found, unless a later scan
    /// of it has started, which reports instead.
    fn publish_probed(&self, scan_number: u64, probed: ProbingFinished) {
        let started = lock(&self.inner.started);
        if started.get(&probed.library_id) == Some(&scan_number) {
            self.inner.events.publish(vec![event(probed)]);
        }
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

/// Whether `path`'s extension is a video's.
fn is_video(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .is_some_and(|ext| VIDEO_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
}

fn video_file(root: &Path, entry: &DirEntry) -> Result<Option<Found>, walkdir::Error> {
    let path = entry.path();
    if !entry.file_type().is_file() || !is_video(path) {
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

    /// Takes the scan lock a scan of the Library `id` takes, to hold it as a scan would.
    pub(crate) async fn lock(&self, id: i64) -> ScanLock {
        self.inner.locks.lock(id).await
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
    use std::{fs, io::Write as _, path::PathBuf, sync::Arc, time::Duration};

    use futures_util::StreamExt;
    use nama_plugin_api::metadata::ExternalId;
    use nama_proto::nama::v1::{Lagged, subscribe_response};
    use tempfile::TempDir;

    use super::{Scanner, Summary};
    use crate::{
        contents::{Contents, File, Item},
        db::Database,
        events::{EventBus, Filter},
        ffmpeg::Ffmpeg,
        libraries::{self, Kind, Libraries},
        playback::subtitles,
        plugins::ExtensionPoints,
        probing::Probing,
        server_settings::ServerUrl,
    };

    /// How many events a [`Fixture`]'s subscribers may fall behind.
    const BUFFERED: usize = 8;

    /// A database, a Library of `kind` backed by one scratch folder, and a scanner publishing on
    /// a bus whose subscribers lag once [`BUFFERED`] events behind.
    struct Fixture {
        scanner: Scanner,
        /// What the scanner tells a scan finished, if anything probes.
        probing: Option<Probing>,
        contents: Contents,
        events: EventBus,
        library: i64,
        root: PathBuf,
        _media: TempDir,
        _data: TempDir,
    }

    impl Fixture {
        /// With nothing probing.
        async fn new(kind: Kind) -> Self {
            Self::with_probing(kind, None).await
        }

        /// Probing with `ffmpeg`.
        #[cfg(unix)]
        async fn with_ffmpeg(kind: Kind, ffmpeg: Ffmpeg) -> Self {
            Self::with_probing(kind, Some(ffmpeg)).await
        }

        async fn with_probing(kind: Kind, ffmpeg: Option<Ffmpeg>) -> Self {
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
                ExtensionPoints::default(),
                ServerUrl::default(),
                BUFFERED,
            );
            let probing = ffmpeg.map(|ffmpeg| {
                let subtitles = subtitles::Cache::new(data.path());
                Probing::new(Arc::clone(&db), Arc::new(ffmpeg), subtitles)
            });
            Self {
                scanner: Scanner::new(
                    Arc::clone(&db),
                    ExtensionPoints::default(),
                    events.clone(),
                    probing.clone(),
                    None,
                ),
                probing,
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
        let mut slow = Box::pin(media.events.subscribe_as_admin(Filter::default()));

        let scanned =
            tokio::time::timeout(Duration::from_secs(5), media.scanner.scan(media.library)).await;
        assert!(matches!(scanned, Ok(Ok(_))), "{scanned:?}");

        // The scan published 25 events: its start, one batch's progress, 20 Items, its finish, and
        // Matching's and Probing's finish. Only the last 8 were kept.
        let lagged = Lagged {
            dropped: 17,
            ..Default::default()
        };
        assert_eq!(
            slow.next().await.and_then(|response| response.message),
            Some(subscribe_response::Message::from(lagged))
        );
    }

    /// An ffprobe in `bin` that adds a line to `bin/runs` each time it runs, then runs `then`, a
    /// shell script.
    #[cfg(unix)]
    fn fake_ffprobe(bin: &std::path::Path, then: &str) -> Ffmpeg {
        let runs = bin.join("runs");
        let script = format!("#!/bin/sh\necho run >> '{}'\n{then}\n", runs.display());
        Ffmpeg {
            ffprobe: Ok(crate::ffmpeg::fake::script(bin, "ffprobe", &script)),
            ..Ffmpeg::default()
        }
    }

    // The fake ffprobe is a shell script.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_scan_probes_its_new_files_in_the_background() {
        let bin = tempfile::tempdir().unwrap();
        let ffmpeg = fake_ffprobe(bin.path(), "exit 1");
        let media = Fixture::with_ffmpeg(Kind::Movies, ffmpeg).await;
        media.write("Alien (1979)/Alien (1979).mkv", 10);

        media.scan().await;
        media.probing.as_ref().unwrap().idle().await;

        assert!(bin.path().join("runs").exists(), "nothing was probed");
    }

    // The fake ffprobe is a shell script.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_scan_finishes_without_waiting_for_probing() {
        let bin = tempfile::tempdir().unwrap();
        let ffmpeg = fake_ffprobe(bin.path(), "exec sleep 60");
        let media = Fixture::with_ffmpeg(Kind::Movies, ffmpeg).await;
        media.write("Alien (1979)/Alien (1979).mkv", 10);

        let scanned = tokio::time::timeout(Duration::from_secs(5), media.scan()).await;

        assert_eq!(scanned, Ok(summary(1, 0, 0, 0)));
        media.probing.as_ref().unwrap().shutdown().await;
    }

    /// How long a watched Library's folders stay quiet in these tests before it's scanned.
    const QUIET: Duration = Duration::from_secs(1);

    /// The periodic scan's interval, so it never comes within a test.
    const LONGER_THAN_A_TEST: Duration = Duration::from_secs(3600);

    /// Starts watching, and waits up to 10 s for the Library's folder to be watched.
    async fn watch(media: &Fixture) {
        media.scanner.watch(QUIET, LONGER_THAN_A_TEST);
        for _ in 0..200 {
            if !media.scanner.watched().is_empty() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        panic!("the Library's folder was never watched");
    }

    /// Waits up to 10 s for the stored files to be `expected`.
    async fn wait_for_files(media: &Fixture, expected: &[&str]) {
        let names = async || -> Vec<String> {
            media
                .files()
                .await
                .into_iter()
                .map(|(path, _)| path)
                .collect()
        };
        for _ in 0..200 {
            if names().await == expected {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert_eq!(names().await, expected);
    }

    #[tokio::test]
    async fn a_watched_library_follows_files_added_renamed_and_deleted_without_being_asked() {
        let media = Fixture::new(Kind::Movies).await;
        watch(&media).await;

        media.write("Alien (1979).mkv", 10);
        wait_for_files(&media, &["Alien (1979).mkv"]).await;

        fs::rename(
            media.path("Alien (1979).mkv"),
            media.path("Aliens (1986).mkv"),
        )
        .unwrap();
        wait_for_files(&media, &["Aliens (1986).mkv"]).await;

        fs::remove_file(media.path("Aliens (1986).mkv")).unwrap();
        wait_for_files(&media, &[]).await;
        media.scanner.shutdown().await;
    }

    #[tokio::test]
    async fn a_file_written_in_chunks_is_scanned_once() {
        let media = Fixture::new(Kind::Movies).await;
        watch(&media).await;

        let mut file = fs::File::create(media.path("Alien (1979).mkv")).unwrap();
        for _ in 0..5 {
            file.write_all(&[0; 1024]).unwrap();
            file.sync_all().unwrap();
            tokio::time::sleep(QUIET / 5).await;
        }
        drop(file);
        wait_for_files(&media, &["Alien (1979).mkv"]).await;
        tokio::time::sleep(QUIET * 2).await;

        let scans = super::lock(&media.scanner.inner.started)[&media.library];
        assert_eq!(scans, 1);
        media.scanner.shutdown().await;
    }

    #[tokio::test]
    async fn watches_follow_libraries_created_updated_and_deleted() {
        let media = Fixture::new(Kind::Movies).await;
        let watched = || media.scanner.watched();
        let more = tempfile::tempdir().unwrap();
        let (first, second) = (more.path().join("first"), more.path().join("second"));
        fs::create_dir(&first).unwrap();
        fs::create_dir(&second).unwrap();
        let first = first.canonicalize().unwrap();
        let second = second.canonicalize().unwrap();
        watch(&media).await;

        let created = media
            .scanner
            .create_library(
                "More".to_owned(),
                Kind::Shows,
                vec![first.to_str().unwrap().to_owned()],
            )
            .await
            .unwrap();
        assert_eq!(
            watched(),
            [(media.root.clone(), media.library), (first, created.id)]
        );

        media
            .scanner
            .update_library(
                created.id,
                "More".to_owned(),
                vec![second.to_str().unwrap().to_owned()],
            )
            .await
            .unwrap();
        assert_eq!(
            watched(),
            [(media.root.clone(), media.library), (second, created.id)]
        );

        media.scanner.delete_library(created.id).await.unwrap();
        assert_eq!(watched(), [(media.root.clone(), media.library)]);
        media.scanner.shutdown().await;
    }

    #[tokio::test]
    async fn deleting_a_library_inside_another_keeps_the_outer_one_watched() {
        let media = Fixture::new(Kind::Movies).await;
        fs::create_dir(media.path("inner")).unwrap();
        let inner = media.path("inner").to_str().unwrap().to_owned();
        watch(&media).await;
        let created = media
            .scanner
            .create_library("Inner".to_owned(), Kind::Shows, vec![inner])
            .await
            .unwrap();

        media.scanner.delete_library(created.id).await.unwrap();
        media.write("inner/Alien (1979).mkv", 10);

        wait_for_files(&media, &["inner/Alien (1979).mkv"]).await;
        media.scanner.shutdown().await;
    }
}
