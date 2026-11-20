//! The scanner: walks each Library's folders and stores the video files in them, and what their
//! names say they hold (see [`names`]).
//!
//! A rescan is incremental. A file whose size and modified time are what was stored is skipped,
//! unless an older version of the name parser read it (ADR 0023), or it is unparsed and the
//! Library's enabled Recognisers changed since its last scan (ADR 0050); a new or changed file is
//! parsed and stored; a stored file that's gone is removed, except under a folder that couldn't be
//! read, or one of the Library's folders that held files and now holds none, as an unmounted
//! share's mountpoint reads, so a drive that isn't mounted doesn't empty the Library. Such a folder
//! is recorded as looking empty until a scan finds files there, or an Admin has one remove what's
//! missing ([`Scanner::remove_missing`]). A renamed file is a file gone and a file found. Changes
//! are written in batches of [`BATCH_SIZE`] (see the crate's `AGENTS.md`).
//! Once the Server is serving, a Library is also scanned when its folders change (see [`watch`]).
//!
//! Files are only listed and their metadata read, never opened, except the images that stand in
//! for downloaded ones. A scan also stores the sidecar subtitles beside each Version, such as
//! `Movie.en.srt`, replacing a Version's only when they changed, and apart from its Tracks, so a
//! changed sidecar never probes its video again. It copies the images beside each Version, such as
//! `poster.jpg`, or for an Episode in its Show's or Season's folder, again only when they changed
//! (see [`crate::library::LocalImages`]). Other sidecars, such as `.nfo` files, are ignored.
//! Deleting media deletes every sidecar too (see [`deleting`]).
//! A scan also records which of the Library's folders the Server can't write to, so a delete from
//! one is refused before anyone tries (ADR 0038).
//!
//! A scan publishes its start, then after each batch is written its progress and the Items it
//! added and removed, then its finish, then once Matching has decided what it could
//! `MatchingFinished`, then once Probing has caught up `ProbingFinished` (see [`crate::events`]).
//!
//! After each scan the Library's items are Matched (see [`crate::matching`]), still under the
//! Library's scan lock, so Matching sees the items the scan stored. Then
//! [`crate::playback::Probing`] is told its files changed, and probes them in the background
//! without holding up the next scan. Last, the images no Match stores any more are deleted, and
//! those the Library's Matches store and the store lacks are downloaded in the background (see
//! [`crate::library::Artwork`]).
//!
//! Probing passes of one Library overlap, so `ProbingFinished` says the Library has caught up, not
//! that one pass is done (ADR 0030): a pass waits for the files it left to another prober, and
//! one whose Probing ends after a later scan of the Library started sends nothing.

mod deleting;
mod library_changes;
mod names;
mod recognised;
mod walking;
mod watch;

pub(crate) use deleting::{Deleted, File as DoomedFile, TakenAlong, ToDelete, read_only_folder};
use recognised::{Planned, recognise};
use walking::{FoundLocalImages, FoundSidecars, after_stem, is_video, plan, walk};

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex, PoisonError},
};

use nama_plugin_api::recogniser::RecogniserMethod;
use nama_proto::nama::v1::{
    MatchingFinished, ProbingFinished, ScanFinished, ScanProgress, ScanStarted,
};
use tokio_util::{sync::CancellationToken, task::TaskTracker};

use crate::{
    db::{self, Database},
    events::{EventBus, event},
    library::{
        Artwork, Change, Contents, CustomImages, LastScan, Libraries, LibraryError, LibraryKind,
        LocalImages, NotFound, Recognisers, ScanLock, ScanLocks,
    },
    matching::{self, Matcher, count},
    playback::{Probing, RunningPlans, Sidecars},
    plugin_settings::PluginSettings,
    plugins::ExtensionPoints,
    requests::{self, Requests},
};

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

impl Summary {
    /// Takes the `dropped` changes, which aren't stored, off what the scan added and updated.
    fn leave_out(&mut self, dropped: &[Planned]) {
        for planned in dropped {
            if planned.added {
                self.added -= 1;
            } else {
                // Can't happen today: only a planned Store or Reparse with an unread file is
                // dropped, and `plan` counted each of those as added or updated.
                debug_assert!(
                    self.updated > 0,
                    "left out a change the scan never counted as updated"
                );
                self.updated = self.updated.saturating_sub(1);
            }
        }
    }
}

/// Why a scan failed.
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    /// The Library couldn't be read, or there is no such Library.
    #[error(transparent)]
    Library(#[from] LibraryError),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    /// Walking the Library's folders panicked.
    #[error("walking the folders failed: {0}")]
    Walk(#[from] tokio::task::JoinError),
    /// The Server is shutting down.
    #[error("the scan was cancelled")]
    Cancelled,
    /// A delete names media in this folder of its Library, which the Library's last scan found the
    /// Server can't write to.
    #[error("media in {0} can't be deleted: the Server can't write to it")]
    ReadOnly(String),
    /// The Requests a delete cancels couldn't be read or cancelled.
    #[error(transparent)]
    Requests(#[from] requests::Error),
    /// A delete names no title, Library or Version there is.
    #[error(transparent)]
    NotFound(#[from] NotFound),
    /// A delete's files hold these Episodes too, which it doesn't name.
    #[error("the title's files also hold {}; name each to delete it too", TakenAlong::names(.0))]
    TakesAlongUnnamed(Vec<TakenAlong>),
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
    /// Asked what a file the name parser can't read is (ADR 0050).
    recognisers: Recognisers,
    contents: Contents,
    sidecars: Sidecars,
    /// Downloads the images Matches store, and deletes those none does.
    artwork: Artwork,
    /// Deletes a deleted Library's Custom Image.
    custom_images: CustomImages,
    /// Copies the images beside Movies' files, and deletes a deleted Library's copies.
    local_images: LocalImages,
    libraries: Libraries,
    matcher: Matcher,
    /// Asks Fulfillers to stop monitoring what a delete takes.
    plugins: ExtensionPoints,
    /// Says which title sources and Recognisers a Library's orders may name.
    plugin_settings: PluginSettings,
    events: EventBus,
    /// Told when a scan finishes and asked to clean subtitles, unless nothing probes.
    probing: Option<Probing>,
    /// Asked what plays media about to be deleted, and to end it.
    running: RunningPlans,
    /// Asked which Requests a delete cancels, and to cancel them.
    requests: Requests,
    /// Background scans and media deletes, so shutting down can wait for them.
    tasks: TaskTracker,
    /// Cancelled when the Server shuts down.
    cancel: CancellationToken,
    /// Libraries with a background scan waiting to start.
    queued: Mutex<HashSet<i64>>,
    /// Folders, by Library id, whose next scan removes what's stored under them though they read
    /// empty.
    // ponytail: kept in memory, so a restart or a failed scan before the removal leaves the folder
    // held, and the Admin presses again; store it if that turns out to bother anyone.
    released: Mutex<HashSet<(i64, String)>>,
    /// How many scans of each Library have published `ScanStarted`, so Probing knows whether a
    /// later scan has. Held while publishing either, so none comes out of order.
    started: Mutex<HashMap<i64, u64>>,
    locks: ScanLocks,
    watches: watch::Watches,
}

impl Scanner {
    /// Tells `probing` each Library whose scan finished and has it clean subtitles, or does
    /// neither where nothing probes, has `running` end the plans playing media it deletes, and has
    /// `requests` cancel the Requests still waiting on it.
    #[expect(
        clippy::too_many_arguments,
        reason = "each is a separate handle the Scanner is built from; a struct bundling them \
                  would exist only for this call"
    )]
    pub(crate) fn new(
        db: Arc<Database>,
        plugin_settings: PluginSettings,
        artwork: Artwork,
        custom_images: CustomImages,
        local_images: LocalImages,
        events: EventBus,
        probing: Option<Probing>,
        running: RunningPlans,
        requests: Requests,
    ) -> Self {
        let locks = ScanLocks::new();
        let plugins = plugin_settings.extension_points();
        Self {
            inner: Arc::new(Inner {
                libraries: Libraries::new(Arc::clone(&db)),
                matcher: Matcher::new(
                    Arc::clone(&db),
                    plugins.clone(),
                    artwork.clone(),
                    events.clone(),
                    locks.clone(),
                ),
                artwork,
                custom_images,
                local_images,
                recognisers: Recognisers::new(plugins.clone(), RecogniserMethod::File),
                plugins,
                plugin_settings,
                events,
                probing,
                running,
                requests,
                contents: Contents::new(Arc::clone(&db)),
                sidecars: Sidecars::new(db),
                tasks: TaskTracker::new(),
                cancel: CancellationToken::new(),
                queued: Mutex::default(),
                released: Mutex::default(),
                started: Mutex::default(),
                locks,
                watches: watch::Watches::default(),
            }),
        }
    }

    /// Starts scanning the Library `id` in the background. If a scan of it is running, one more
    /// follows, so what changed since the running scan began is picked up; if one is already
    /// waiting to follow, that one does.
    pub(crate) async fn start(&self, id: i64) -> Result<(), LibraryError> {
        self.inner.libraries.get(id).await?;
        self.queue(id);
        Ok(())
    }

    /// Starts scanning the Library `id` in the background, as [`Self::start`] does, removing what's
    /// stored under its `folder` even if the folder reads empty, that once. A `folder` its last
    /// scan didn't find empty is scanned as [`Self::start`] would.
    pub(crate) async fn remove_missing(&self, id: i64, folder: String) -> Result<(), LibraryError> {
        let library = self.inner.libraries.get(id).await?;
        if library.empty_folders.contains(&folder) {
            lock(&self.inner.released).insert((id, folder));
        }
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

    /// The Matcher that fixes a Match by hand, waiting for a scan of the item's Library.
    pub(crate) fn matcher(&self) -> &Matcher {
        &self.inner.matcher
    }

    /// Cancels the scans running or waiting, waits for them and for media deletes already
    /// started to stop, and starts no more scans.
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
                Ok(_) | Err(Error::Cancelled | Error::Library(LibraryError::NotFound(_))) => {}
                Err(err) => tracing::error!(library = id, "scanning a Library failed: {err}"),
            }
        });
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one scan's steps in the order they run; split, it would be read in pieces"
    )]
    #[tracing::instrument(name = "scan", skip_all, fields(library.id = scan.library()))]
    async fn scan_locked(&self, scan: &ScanLock) -> Result<Summary, Error> {
        let id = scan.library();
        let cancel = self.inner.cancel.clone();
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let library = self.inner.libraries.get(id).await?;
        let scan_number = self.publish_started(id);
        (self.inner.recognisers)
            .read_again_if_changed(&self.inner.libraries, &library)
            .await?;
        let stored = self.inner.contents.stored(id).await?;
        let walking = cancel.clone();
        let mut found =
            tokio::task::spawn_blocking(move || walk(&library.folders, &walking)).await??;
        let released = (lock(&self.inner.released).extract_if(|(library_id, _)| *library_id == id))
            .map(|(_, folder)| folder)
            .collect();
        let held = found.hold_empty(&stored, &released);
        for folder in &held {
            tracing::warn!(
                library = id,
                "{folder} looks empty. Is the drive connected? Nothing was removed."
            );
        }
        if held != library.empty_folders {
            self.inner.libraries.set_empty_folders(id, held).await?;
        }
        let sidecars = found.take_sidecars();
        let images = found.take_images();
        if found.read_only != library.read_only {
            let read_only = found.read_only.clone();
            self.inner.libraries.set_read_only(id, read_only).await?;
        }
        let (planned, mut summary) = plan(library.kind, stored, found);
        // A temporary, so its failures are logged as soon as this statement ends.
        let (changes, dropped) = recognise(
            &self.inner.recognisers.in_order(&library.recognisers),
            library.kind,
            planned,
            &cancel,
        )
        .await?;
        summary.leave_out(&dropped);
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
        self.store_sidecars(id, sidecars, &cancel).await?;
        self.store_local_images(library.kind, id, images, &cancel)
            .await?;
        tracing::info!(library = id, ?summary, "scanned a Library");
        if let Some(probing) = &self.inner.probing {
            // Before `ScanFinished`, so a client that waited for the scan isn't served a gone or
            // changed file's subtitles.
            probing.clean_leftovers().await;
        }
        let finished_at = db::now();
        self.inner.events.publish(vec![event(ScanFinished {
            library_id: id,
            added: summary.added,
            updated: summary.updated,
            removed: summary.removed,
            unchanged: summary.unchanged,
            ..Default::default()
        })]);
        // After `ScanFinished`, which mustn't wait behind other writes when nothing changed
        // (#1130), and before Matching, so `MatchingFinished` finds it recorded.
        let last = LastScan {
            finished_at,
            added: summary.added,
            updated: summary.updated,
            removed: summary.removed,
            unchanged: summary.unchanged,
        };
        // Only bookkeeping, so Matching still follows the `ScanFinished` already sent.
        if let Err(err) = self.inner.libraries.record_last_scan(id, last).await {
            tracing::error!(
                library = id,
                "recording a Library's last scan failed: {err}"
            );
        }
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
        // Last, so no event of the scan waits on it.
        let contents = &self.inner.contents;
        self.inner.artwork.drop_unused(contents).await;
        self.inner.artwork.download_missing(contents.clone(), id);
        Ok(summary)
    }

    /// Replaces the stored sidecars of each of the Library `id`'s Versions whose found ones differ,
    /// in batches. After the scan's files are stored, so new Versions have ids.
    async fn store_sidecars(
        &self,
        id: i64,
        found: FoundSidecars,
        cancel: &CancellationToken,
    ) -> Result<(), Error> {
        let versions = self.inner.contents.stored(id).await?;
        let ids: Vec<i64> = versions.values().map(|version| version.id).collect();
        let stored = self.inner.sidecars.stored(&ids).await?;
        let mut changed = found.changed(&versions, stored).into_iter();
        loop {
            let batch: Vec<_> = changed.by_ref().take(BATCH_SIZE).collect();
            if batch.is_empty() {
                return Ok(());
            }
            if cancel.is_cancelled() {
                return Err(Error::Cancelled);
            }
            self.inner.sidecars.replace(batch).await?;
        }
    }

    /// Copies the images a Library of `kind` ties to the Library `id`'s Versions that differ from
    /// those stored, or whose copies are gone, in batches, then deletes those none has any more.
    /// After the scan's files and Items are stored, so new Versions have ids and Seasons.
    async fn store_local_images(
        &self,
        kind: LibraryKind,
        id: i64,
        found: FoundLocalImages,
        cancel: &CancellationToken,
    ) -> Result<(), Error> {
        let versions = self.inner.contents.stored(id).await?;
        let seasons = self.inner.contents.episode_seasons(id).await?;
        let (read, found) = found.beside(kind, &versions, &seasons);
        let changed = self.inner.local_images.changed(id, read, &found).await?;
        for batch in changed.chunks(BATCH_SIZE) {
            if cancel.is_cancelled() {
                return Err(Error::Cancelled);
            }
            self.inner.local_images.keep(id, batch, &found).await?;
        }
        self.inner.local_images.drop_unused(id).await;
        Ok(())
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
#[path = "scanner/shutdown_tests.rs"]
mod shutdown_tests;

#[cfg(test)]
#[path = "scanner/sidecars_tests.rs"]
mod sidecars_tests;

#[cfg(test)]
#[path = "scanner/scanner_tests.rs"]
mod tests;
