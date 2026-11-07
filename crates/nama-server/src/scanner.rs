//! The scanner: walks each Library's folders and stores the video files in them, and what their
//! names say they hold (see [`names`]).
//!
//! A rescan is incremental. A file whose size and modified time are what was stored is skipped;
//! a new or changed file is parsed and stored; a stored file that's gone is removed, except
//! under a folder that couldn't be read, so a drive that isn't mounted doesn't empty the
//! Library. A renamed file is a file gone and a file found. Changes are written in batches of
//! [`BATCH_SIZE`] (see the crate's `AGENTS.md`).
//!
//! Files are only listed and their metadata read, never opened: sidecars such as `.nfo` files
//! are ignored.
//!
//! A scan publishes its start, then after each batch is written its progress and the Items it
//! added and removed, then its finish (see [`crate::events`]).
//!
//! After each scan the Library's items are Matched (see [`crate::matching`]), still under the
//! Library's scan lock, so Matching sees the items the scan stored.

mod names;

use std::{
    collections::{HashMap, HashSet},
    ffi::OsStr,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, PoisonError},
    time::{SystemTime, UNIX_EPOCH},
};

use nama_proto::nama::v1::{ScanFinished, ScanProgress, ScanStarted};
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use walkdir::{DirEntry, WalkDir};

use crate::{
    contents::{Change, Contents, Stored},
    db::Database,
    events::{EventBus, event},
    libraries::{self, Kind, Libraries},
    matching::{self, ManualError, Matcher, count},
    plugins::Registry,
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
    /// Stored files whose size or modified time changed, which were parsed again.
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
    /// Background scans, so shutting down can wait for them.
    tasks: TaskTracker,
    /// Cancelled when the Server shuts down.
    cancel: CancellationToken,
    /// Libraries with a background scan waiting to start.
    queued: Mutex<HashSet<i64>>,
    /// Each Library's scan lock, handed out as a [`ScanLock`].
    running: Mutex<HashMap<i64, Arc<tokio::sync::Mutex<()>>>>,
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
    pub(crate) fn new(db: Arc<Database>, plugins: Registry, events: EventBus) -> Self {
        Self {
            inner: Arc::new(Inner {
                libraries: Libraries::new(Arc::clone(&db)),
                matcher: Matcher::new(Arc::clone(&db), plugins, events.clone()),
                events,
                contents: Contents::new(db),
                tasks: TaskTracker::new(),
                cancel: CancellationToken::new(),
                queued: Mutex::default(),
                running: Mutex::default(),
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
        Ok(summary)
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
    for file in found.files {
        if !seen.insert(file.path.clone()) {
            continue;
        }
        match stored.remove(&file.path) {
            Some(old) if old.size == file.size && old.modified_ns == file.modified_ns => {
                summary.unchanged += 1;
                continue;
            }
            Some(_) => summary.updated += 1,
            None => summary.added += 1,
        }
        changes.push(Change::Store {
            items: names::parse(kind, &file.relative).unwrap_or_default(),
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
    use std::{fs, path::PathBuf, sync::Arc, time::Duration};

    use futures_util::StreamExt;
    use nama_proto::nama::v1::{Lagged, subscribe_response};
    use tempfile::TempDir;

    use super::{Scanner, Summary};
    use crate::{
        contents::{Contents, File, Item},
        db::Database,
        events::{EventBus, Filter},
        libraries::{self, Kind, Libraries},
        plugins::Registry,
    };

    /// How many events a [`Fixture`]'s subscribers may fall behind.
    const BUFFERED: usize = 8;

    /// A database, a Library of `kind` backed by one scratch folder, and a scanner publishing on
    /// a bus whose subscribers lag once [`BUFFERED`] events behind.
    struct Fixture {
        scanner: Scanner,
        contents: Contents,
        events: EventBus,
        library: i64,
        root: PathBuf,
        _media: TempDir,
        _data: TempDir,
    }

    impl Fixture {
        async fn new(kind: Kind) -> Self {
            let data = tempfile::tempdir().unwrap();
            let media = tempfile::tempdir().unwrap();
            let root = media.path().canonicalize().unwrap().join("library");
            fs::create_dir(&root).unwrap();
            let db = Arc::new(Database::open(data.path()).await.unwrap());
            let library = Libraries::new(Arc::clone(&db))
                .create("Media", kind, vec![root.to_str().unwrap().to_owned()])
                .await
                .unwrap();
            let events = EventBus::with_capacity(Registry::new(), BUFFERED);
            Self {
                scanner: Scanner::new(Arc::clone(&db), Registry::new(), events.clone()),
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

    fn movie(title: &str, year: i64, tmdb_id: Option<i64>) -> Vec<Item> {
        vec![Item::Movie {
            title: title.to_owned(),
            year: Some(year),
            tmdb_id,
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
            tmdb_id: None,
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
                    movie("Blade Runner", 1982, Some(78))
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
}
