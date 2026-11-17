//! The scanner: walks each Library's folders and stores the video files in them, and what their
//! names say they hold (see [`names`]).
//!
//! A rescan is incremental. A file whose size and modified time are what was stored is skipped,
//! unless an older version of the name parser read it (ADR 0023), or it is unparsed and the
//! Library's enabled Recognisers changed since its last scan (ADR 0050); a new or changed file is
//! parsed and stored; a stored file that's gone is removed, except under a folder that couldn't be
//! read, so a drive that isn't mounted doesn't empty the Library. A renamed file is a file gone and
//! a file found. Changes are written in batches of [`BATCH_SIZE`] (see the crate's `AGENTS.md`).
//! Once the Server is serving, a Library is also scanned when its folders change (see [`watch`]).
//!
//! Files are only listed and their metadata read, never opened, except the images beside a Movie's
//! file. A scan also stores the sidecar subtitles beside each Version, such as `Movie.en.srt`,
//! replacing a Version's only when they changed, and apart from its Tracks, so a changed sidecar
//! never probes its video again. In a Movies Library it copies the images beside each Version,
//! such as `poster.jpg`, that stand in for its downloaded ones, again only when they changed (see
//! [`crate::library::LocalImages`]). Other sidecars, such as `.nfo` files, are ignored.
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
mod names;
mod recognised;
mod walking;
mod watch;

pub(crate) use deleting::{Deleted, Doomed, File as DoomedFile, read_only_folder};
use recognised::{Planned, read_again_if_changed, recognise};
use walking::{FoundLocalImages, FoundSidecars, after_stem, is_video, plan, walk};

use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::{Arc, Mutex, PoisonError},
};

use nama_plugin_api::{
    PluginId,
    fulfiller::{Fulfiller, Movies, Shows, StopMonitoring, stop_monitoring::Title},
};
use nama_proto::nama::v1::{
    MatchingFinished, ProbingFinished, ScanFinished, ScanProgress, ScanStarted,
};
use tokio_util::{sync::CancellationToken, task::TaskTracker};

use crate::{
    db::{Database, detached},
    events::{EventBus, event},
    library::{
        Artwork, Change, Contents, CustomImages, Libraries, Library, LibraryError, LibraryKind,
        LocalImages, Order, Role, ScanLock, ScanLocks, check_title_sources,
    },
    matching::{self, Matcher, count},
    playback::{Probing, RunningPlans, Sidecars},
    plugins::{ExtensionPoints, Failure},
    requests::{self, RequestTitle, Requests},
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
    /// Where a scan starts by checking its Library's Recognisers (ADR 0050).
    db: Arc<Database>,
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
    /// Asks Recognisers what a file the name parser can't read is, and Fulfillers to stop
    /// monitoring what a delete takes.
    plugins: ExtensionPoints,
    events: EventBus,
    /// Told when a scan finishes and asked to clean subtitles, unless nothing probes.
    probing: Option<Probing>,
    /// Asked what plays media about to be deleted, and to end it, unless nothing plays.
    running: Option<RunningPlans>,
    /// Asked which Requests a delete cancels, and to cancel them, unless nothing has Requests.
    requests: Option<Requests>,
    /// Background scans and media deletes, so shutting down can wait for them.
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
        plugins: ExtensionPoints,
        artwork: Artwork,
        custom_images: CustomImages,
        local_images: LocalImages,
        events: EventBus,
        probing: Option<Probing>,
        running: Option<RunningPlans>,
        requests: Option<Requests>,
    ) -> Self {
        let locks = ScanLocks::new();
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
                plugins,
                events,
                probing,
                running,
                requests,
                contents: Contents::new(Arc::clone(&db)),
                sidecars: Sidecars::new(Arc::clone(&db)),
                db,
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
    pub(crate) async fn start(&self, id: i64) -> Result<(), LibraryError> {
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

    /// Creates a Library asking its Plugins in `order`, and watches its folders. The watch follows
    /// even if the caller stops waiting (ADR 0029); scanning the new Library is left to the
    /// caller.
    pub(crate) async fn create_library(
        &self,
        name: String,
        kind: LibraryKind,
        folders: Vec<String>,
        order: Order,
    ) -> Result<Library, LibraryError> {
        let scanner = self.clone();
        detached(async move {
            let libraries = &scanner.inner.libraries;
            if !order.title_sources.is_empty() {
                check_title_sources(&order.title_sources)?;
            }
            let library = (libraries.create_in_order(&name, kind, folders, order)).await?;
            scanner.rewatch().await;
            Ok(library)
        })
        .await
    }

    /// Renames the Library `id`, replaces its folders and whichever of its orders `order` gives,
    /// then watches its new folders and not its old ones, even if the caller stops waiting (ADR
    /// 0029). A new order doesn't Match its titles again.
    pub(crate) async fn update_library(
        &self,
        id: i64,
        name: String,
        folders: Vec<String>,
        order: Order,
    ) -> Result<Library, LibraryError> {
        let scanner = self.clone();
        detached(async move {
            let libraries = &scanner.inner.libraries;
            if !order.title_sources.is_empty() {
                check_title_sources(&order.title_sources)?;
            }
            let library = libraries.update(id, &name, folders, order).await?;
            scanner.rewatch().await;
            Ok(library)
        })
        .await
    }

    /// Deletes the Library `id`, then throws away its Versions' cached subtitles where something
    /// probes, the images only its Matches stored, its Custom Image and its copies of the images
    /// beside its media, and stops watching its folders, even if the caller stops waiting
    /// (ADR 0029).
    pub(crate) async fn delete_library(&self, id: i64) -> Result<(), LibraryError> {
        let scanner = self.clone();
        detached(async move {
            scanner.inner.libraries.delete(id).await?;
            scanner
                .inner
                .artwork
                .drop_unused(&scanner.inner.contents)
                .await;
            scanner.inner.custom_images.delete_library(id).await;
            scanner.inner.local_images.delete_library(id).await;
            if let Some(probing) = &scanner.inner.probing {
                probing.clean_subtitles().await;
            }
            scanner.rewatch().await;
            Ok(())
        })
        .await
    }

    /// What [`Self::delete_media`] would delete, stop playing and cancel, deleting nothing.
    pub(crate) async fn preview_delete(&self, doomed: Doomed) -> Result<Deleted, Error> {
        let versions = doomed.version_ids();
        let playing = (self.inner.running.as_ref()).map_or(0, |running| running.playing(&versions));
        let requests = match &self.inner.requests {
            Some(requests) => (requests.waiting_on(&doomed.request_titles(&versions))).await?,
            None => Vec::new(),
        };
        let planned = tokio::task::spawn_blocking(move || deleting::preview(&doomed)).await?;
        Ok(Deleted {
            playing,
            requests,
            ..planned
        })
    }

    /// Asks every Fulfiller of its kind to stop monitoring `doomed`, then ends the plans
    /// playing it, deletes it from disk for good, removes the Versions whose files went as a
    /// scan removes them, with the images only their Matches stored, and logs that `by` deleted
    /// them, and cancels the Requests still waiting on what they were of. It refuses, before
    /// any of that, media in a folder the Library's last scan found read-only
    /// ([`Error::ReadOnly`]). Once the files are deleted it returns what went: a later step's
    /// failure is only logged. The one way to delete media (see AGENTS.md, "One path for
    /// deleting media"). It holds the Library's scan lock, so a scan racing it sees either
    /// everything or nothing gone, and finishes even if the caller stops waiting (ADR 0029).
    /// Shutting down waits for a delete already started.
    pub(crate) async fn delete_media(&self, doomed: Doomed, by: String) -> Result<Deleted, Error> {
        refuse_read_only(&doomed)?;
        let deleting = self.inner.tasks.spawn(self.clone().deleting(doomed, by));
        // The task is never aborted, so it only fails if it panicked.
        (deleting.await).unwrap_or_else(|_| Err(sqlx::Error::WorkerCrashed.into()))
    }

    /// Starts deleting `doomed` for the Plugin `plugin`, as [`Self::delete_media`] does, without
    /// waiting, and logs a failure naming `plugin`. Refuses media in a read-only folder before
    /// starting. Started on the tracker before it returns, so shutting down right after still
    /// waits for the delete.
    pub(crate) fn start_delete(
        &self,
        doomed: Doomed,
        by: String,
        plugin: PluginId,
    ) -> Result<(), Error> {
        refuse_read_only(&doomed)?;
        let deleting = self.clone().deleting(doomed, by);
        self.inner.tasks.spawn(async move {
            if let Err(err) = deleting.await {
                tracing::error!(%plugin, "the Plugin's delete failed: {err}");
            }
        });
        Ok(())
    }

    /// The delete [`Self::delete_media`] describes, from stopping the monitoring on. Run it on
    /// the tracker, so shutting down waits for it; it never reads the tracker's cancel token, so
    /// shutting down doesn't stop a delete halfway.
    async fn deleting(self, doomed: Doomed, by: String) -> Result<Deleted, Error> {
        // First, so the service can't see the file missing and download it again (ADR 0038).
        let not_told = self.stop_monitoring(&doomed).await;
        let library = doomed.library.id;
        let _scan = self.inner.locks.lock(library).await;
        // Held until the Versions are gone, so none is served again before.
        let _ending = match &self.inner.running {
            Some(running) => Some(running.end_for_deleting(&doomed.version_ids()).await),
            None => None,
        };
        let folders: Vec<String> = (self.inner.libraries.list().await?.into_iter())
            .flat_map(|other| other.folders)
            .collect();
        let (doomed, mut deleted) = tokio::task::spawn_blocking(move || {
            let kept: Vec<&Path> = folders.iter().map(Path::new).collect();
            let deleted = deleting::delete(&doomed, &kept);
            (doomed, deleted)
        })
        .await?;
        deleted.fulfillers_not_told.extend(not_told);
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
        let events = match self.inner.contents.apply(library, removed.collect()).await {
            Ok(events) => events,
            Err(error) => {
                // The next scan removes them, as it does any file gone from disk.
                tracing::error!(
                    %error,
                    title = %doomed.title,
                    library,
                    "couldn't remove the deleted Versions; they stay listed until the next scan"
                );
                Vec::new()
            }
        };
        if let Some(probing) = &self.inner.probing {
            probing.clean_subtitles().await;
        }
        self.inner.events.publish(events);
        self.inner.artwork.drop_unused(&self.inner.contents).await;
        (self.cancel_requests(doomed.request_titles(&deleted.versions), by)).await;
        Ok(deleted)
    }

    /// Cancels the Requests still waiting on `titles`, whose media `by` deleted. A failure is
    /// logged, not returned: the media is gone by then, and an error would hide what went.
    async fn cancel_requests(&self, titles: Vec<RequestTitle>, by: String) {
        let Some(requests) = &self.inner.requests else {
            return;
        };
        if let Err(error) = requests.cancel_deleted(titles.clone(), by).await {
            tracing::error!(
                %error,
                ?titles,
                "couldn't cancel the Requests waiting on deleted media; they stay open"
            );
        }
    }

    /// Asks each enabled Fulfiller of `doomed`'s Library's kind to stop monitoring what it takes.
    /// Returns the name of each that couldn't be told, failing or panicking, which isn't asked
    /// again. One that says to try again is asked again later, in its scope (ADR 0047).
    async fn stop_monitoring(&self, doomed: &Doomed) -> Vec<String> {
        let Some(title) = doomed.to_unmonitor.clone() else {
            return Vec::new();
        };
        let title = StopMonitoring {
            title: Some(title),
            ..Default::default()
        };
        match doomed.library.kind {
            LibraryKind::Movies => self.ask_to_stop::<Movies>(&title, &doomed.title).await,
            LibraryKind::Shows => self.ask_to_stop::<Shows>(&title, &doomed.title).await,
        }
    }

    /// Asks each enabled Fulfiller of the kind `K` to stop monitoring `title`, which the log calls
    /// `name`, but one that needs an id the title lacks. Returns the name of each not told.
    async fn ask_to_stop<K: 'static>(&self, title: &StopMonitoring, name: &str) -> Vec<String> {
        let (kind, ids) = match &title.title {
            Some(Title::Movie(movie)) => ("Movie", &movie.ids[..]),
            Some(Title::Show(show)) => ("Show", &show.show_ids[..]),
            None => ("title", &[][..]),
        };
        let stop = |fulfiller: Arc<dyn Fulfiller<K>>| async move {
            let namespaces = ids.iter().map(|id| id.namespace.as_str());
            if let Some(needs) = requests::unmet(&*fulfiller, namespaces) {
                return Ok(Err(needs));
            }
            let stopped = fulfiller.stop_monitoring(title).await;
            stopped.map(Ok).map_err(deleting::Retryable)
        };
        let plugins = &self.inner.plugins;
        let mut not_told = Vec::new();
        for plugin in plugins.providers::<dyn Fulfiller<K>>() {
            let answer = plugins.ask(&plugin, &stop).await;
            // `None`: it panicked, which disabled it, or was disabled meanwhile.
            let reason = match answer.map(|answer| answer.result) {
                Some(Ok(Ok(()))) => continue,
                Some(Err(Failure {
                    kind: Some(wait), ..
                })) => {
                    let (title, name) = (title.clone(), name.to_owned());
                    let again = deleting::StopAgain {
                        plugin,
                        title,
                        name,
                    };
                    again.after::<K>(plugins, wait, 1);
                    continue;
                }
                Some(Ok(Err(needs))) => requests::missing(&plugin, kind, &needs),
                Some(Err(failure)) => failure.message,
                None => "it panicked or was disabled".to_owned(),
            };
            tracing::warn!(
                plugin = %plugin,
                title = name,
                reason,
                "a Fulfiller didn't stop monitoring a deleted title, so it may download it again"
            );
            not_told.push(plugins.name(&plugin));
        }
        not_told
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
        read_again_if_changed(&self.inner.db, &self.inner.plugins, &library).await?;
        let stored = self.inner.contents.stored(id).await?;
        let walking = cancel.clone();
        let mut found =
            tokio::task::spawn_blocking(move || walk(&library.folders, &walking)).await??;
        let sidecars = found.take_sidecars();
        let images = found.take_images();
        if found.read_only != library.read_only {
            let read_only = found.read_only.clone();
            self.inner.libraries.set_read_only(id, read_only).await?;
        }
        let (planned, mut summary) = plan(library.kind, stored, found);
        let order = &library.recognisers;
        let (changes, dropped) =
            recognise(&self.inner.plugins, library.kind, order, planned, &cancel).await?;
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
        if library.kind == LibraryKind::Movies {
            self.store_local_images(id, images, &cancel).await?;
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

    /// Copies the images beside each of the Library `id`'s Versions whose found ones differ from
    /// those stored, in batches, then deletes the copies none has any more. After the scan's files
    /// are stored, so new Versions have ids.
    async fn store_local_images(
        &self,
        id: i64,
        found: FoundLocalImages,
        cancel: &CancellationToken,
    ) -> Result<(), Error> {
        let versions = self.inner.contents.stored(id).await?;
        let (read, found) = found.beside(&versions);
        let stored = self.inner.local_images.stored(id).await?;
        let differs = |version: &i64| {
            [Role::Poster, Role::Backdrop]
                .iter()
                .any(|&role| found.get(&(*version, role)) != stored.get(&(*version, role)))
        };
        let changed: Vec<i64> = read.into_iter().filter(differs).collect();
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

/// Refuses `doomed` if it is in a folder the Library's last scan found read-only.
fn refuse_read_only(doomed: &Doomed) -> Result<(), Error> {
    match doomed.read_only_folder() {
        Some(folder) => Err(Error::ReadOnly(folder.to_owned())),
        None => Ok(()),
    }
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
