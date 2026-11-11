//! Watching Library folders, so a Library is scanned soon after its files change without anyone
//! asking.
//!
//! An OS event only says *when* to scan: a scan is incremental, so it finds what changed itself.
//! Events aren't reliable everywhere (network shares send none for changes made from other
//! machines, Docker bind mounts on macOS drop them, inotify has a per-user watch limit), so every
//! Library is also scanned now and then, and a folder that can't be watched is left to that.
//!
//! ponytail: macOS reports a file's writes only once it's closed, so a file written there for
//! longer than the quiet period is scanned when it's created and again once it's closed.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher as _};
use tokio::{sync::mpsc, time::Instant};

use super::{Scanner, lock};

/// The folders the [`Scanner`] watches: none until [`Scanner::watch`].
#[derive(Debug, Default)]
pub(super) struct Watches {
    /// `None` until watching starts, and where the OS can't watch. A tokio lock, held for a whole
    /// rewatch, so rewatches don't overlap.
    watcher: Arc<tokio::sync::Mutex<Option<RecommendedWatcher>>>,
    /// Each watched folder, with its Library.
    folders: Arc<Mutex<HashMap<PathBuf, i64>>>,
}

impl Scanner {
    /// Starts watching every Library's folders in the background: a Library is scanned once its
    /// folders have been quiet for `quiet` after a change, and every Library every `every`, until
    /// shutdown.
    pub(crate) fn watch(&self, quiet: Duration, every: Duration) {
        let scanner = self.clone();
        self.inner.tasks.spawn(async move {
            let changes = scanner.start_watcher(every).await;
            scanner.rewatch().await;
            scanner.scan_when_changed(changes, quiet, every).await;
        });
    }

    /// Starts the OS's watcher, and returns the Libraries it reports changes in.
    async fn start_watcher(&self, every: Duration) -> mpsc::UnboundedReceiver<i64> {
        // Unbounded, since the OS watcher's thread mustn't wait, and a change only moves a deadline.
        let (sender, changes) = mpsc::unbounded_channel();
        let folders = Arc::clone(&self.inner.watches.folders);
        let watcher = notify::recommended_watcher(move |event| {
            send_changed_libraries(&folders, &sender, event);
        });
        match watcher {
            Ok(watcher) => *self.inner.watches.watcher.lock().await = Some(watcher),
            Err(err) => tracing::warn!(
                "Library folders can't be watched, so changes in them are picked up only by the \
                 scan every {every:?}: {err}"
            ),
        }
        changes
    }

    /// Watches the folders of the Libraries there are now, and no others. Before
    /// [`Scanner::watch`] it does nothing.
    pub(super) async fn rewatch(&self) {
        let mut watcher = Arc::clone(&self.inner.watches.watcher).lock_owned().await;
        if watcher.is_none() {
            return;
        }
        let libraries = match self.inner.libraries.list().await {
            Ok(libraries) => libraries,
            Err(err) => {
                tracing::error!("could not list the Libraries to watch: {err}");
                return;
            }
        };
        let wanted = libraries
            .into_iter()
            .flat_map(|library| {
                let id = library.id;
                library
                    .folders
                    .into_iter()
                    .map(move |folder| (folder.into(), id))
            })
            .collect();
        let folders = Arc::clone(&self.inner.watches.folders);
        // Watching a folder walks all of it.
        let task = tokio::task::spawn_blocking(move || {
            let Some(watcher) = watcher.as_mut() else {
                return;
            };
            let before: Vec<PathBuf> = lock(&folders).keys().cloned().collect();
            // Not under `folders`' lock: the OS watcher's thread takes it to report an event, and
            // watching waits for that thread.
            let now_watched = watch_only(watcher, &before, wanted);
            *lock(&folders) = now_watched;
        });
        if let Err(err) = task.await {
            tracing::error!("watching the Library folders panicked: {err}");
        }
    }

    /// Scans each Library `quiet` after the last change `changes` reports in it, so a file being
    /// copied in is scanned once, and every Library every `every`, until shutdown.
    async fn scan_when_changed(
        self,
        mut changes: mpsc::UnboundedReceiver<i64>,
        quiet: Duration,
        every: Duration,
    ) {
        let mut due: HashMap<i64, Instant> = HashMap::new();
        let mut periodic = tokio::time::interval_at(Instant::now() + every, every);
        loop {
            let next_due = due.values().min().copied();
            tokio::select! {
                () = self.inner.cancel.cancelled() => return,
                Some(library) = changes.recv() => {
                    due.insert(library, Instant::now() + quiet);
                }
                () = sleep_until(next_due) => {
                    let now = Instant::now();
                    for (library, _) in due.extract_if(|_, at| *at <= now) {
                        self.queue(library);
                    }
                }
                _ = periodic.tick() => {
                    // A folder that couldn't be watched before may be back, such as one remounted.
                    self.rewatch().await;
                    self.start_all().await;
                }
            }
        }
    }
}

/// Has `watcher` watch the folders in `wanted`, given it watches those in `before`. Returns
/// `wanted` less those it couldn't watch.
///
/// Only the outermost folders get OS watches: a watch of a folder covers what's inside it, and
/// notify keys its inotify watches by path, so watching a folder inside another shares the outer
/// watch's, and unwatching either ends both.
// ponytail: a watch the OS ended, as it does for a folder deleted or unmounted, goes unnoticed, so
// the folder is left to the periodic scan even once it's back; forget the folder on its own
// remove event if that matters.
fn watch_only(
    watcher: &mut RecommendedWatcher,
    before: &[PathBuf],
    mut wanted: HashMap<PathBuf, i64>,
) -> HashMap<PathBuf, i64> {
    let outer_before = outermost(&before.iter().collect::<Vec<_>>());
    let outer_wanted = outermost(&wanted.keys().collect::<Vec<_>>());
    // Unwatching first, since a folder that was inside one unwatched may now need its own watch.
    for folder in outer_before
        .iter()
        .filter(|folder| !outer_wanted.contains(folder))
    {
        // It fails only for a folder whose watch the OS already ended.
        let _: notify::Result<()> = watcher.unwatch(folder);
    }
    let failed: Vec<PathBuf> = outer_wanted
        .into_iter()
        .filter(|folder| !outer_before.contains(folder) && !watch_folder(watcher, folder))
        .cloned()
        .collect();
    wanted.retain(|folder, _| !failed.iter().any(|outer| folder.starts_with(outer)));
    wanted
}

/// The `folders` not inside another of them.
fn outermost<'a>(folders: &[&'a PathBuf]) -> Vec<&'a PathBuf> {
    folders
        .iter()
        .filter(|folder| {
            !folders
                .iter()
                .any(|outer| outer != *folder && folder.starts_with(outer))
        })
        .copied()
        .collect()
}

/// Whether `watcher` could watch `folder` and all that's in it.
fn watch_folder(watcher: &mut RecommendedWatcher, folder: &Path) -> bool {
    let Err(err) = watcher.watch(folder, RecursiveMode::Recursive) else {
        return true;
    };
    tracing::warn!(
        "{} isn't watched, so changes in it wait for the next scan of every Library: {err}",
        folder.display()
    );
    // It may have failed partway, such as at inotify's limit, and what it watched counts towards it.
    let _: notify::Result<()> = watcher.unwatch(folder);
    false
}

/// Sends `changes` the Library of each watched folder in `folders` that `event` happened in.
fn send_changed_libraries(
    folders: &Mutex<HashMap<PathBuf, i64>>,
    changes: &mpsc::UnboundedSender<i64>,
    event: notify::Result<Event>,
) {
    let event = match event {
        // Probing opens files, which changes nothing.
        Ok(event) if event.kind.is_access() => return,
        Ok(event) => event,
        Err(err) => {
            tracing::warn!("watching the Library folders failed: {err}");
            return;
        }
    };
    for (folder, &library) in lock(folders).iter() {
        // The OS dropped events it had no room for, so any folder may have changed.
        if event.need_rescan() || event.paths.iter().any(|path| path.starts_with(folder)) {
            // It fails only once scanning when changed has stopped, at shutdown.
            let _: Result<(), _> = changes.send(library);
        }
    }
}

/// Sleeps until `deadline`, or forever without one.
async fn sleep_until(deadline: Option<Instant>) {
    match deadline {
        Some(deadline) => tokio::time::sleep_until(deadline).await,
        None => std::future::pending().await,
    }
}

#[cfg(test)]
impl Scanner {
    /// The watched folders, each with its Library, in the order of their Libraries.
    pub(super) fn watched(&self) -> Vec<(PathBuf, i64)> {
        let mut watched: Vec<_> = lock(&self.inner.watches.folders)
            .iter()
            .map(|(folder, &library)| (folder.clone(), library))
            .collect();
        watched.sort_by_key(|&(_, library)| library);
        watched
    }
}
