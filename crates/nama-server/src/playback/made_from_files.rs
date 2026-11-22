//! What the things made from a Version's file share: Segments, Preview frames and text passes
//! follow Probing's passes ([`ProbePasses`]), what's made of a Version gone is deleted in one place
//! ([`Leftovers`]), and keyframe listings and text passes read whole files under one limit
//! ([`WholeFileReads`]). Each follower's loop keeps its own policy (ADR 0075, ADR 0076).

use std::{
    collections::HashSet,
    io,
    sync::{Arc, Mutex, PoisonError},
};

use tokio::sync::{Notify, Semaphore};
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use tracing::Instrument as _;

use super::subtitles;
use crate::{
    db::Database,
    images::{DOWNLOADED_SUBTITLES_DIR, ImageStore, PREVIEW_FRAMES_DIR},
    library::{Contents, Stored},
};

/// How many whole files are read at once, to list a Version's keyframes or read its text
/// subtitles out, from a disk that playback is reading from too.
pub(super) const WHOLE_FILE_READS_AT_ONCE: usize = 2;

/// Who follows Probing's background passes, each woken once a pass has probed what it found stale.
#[derive(Debug, Clone, Default)]
pub(crate) struct ProbePasses {
    followers: Arc<Mutex<Vec<Arc<Notify>>>>,
}

impl ProbePasses {
    /// A new follower, notified after each pass from now on.
    pub(crate) fn follow(&self) -> Arc<Notify> {
        let follower = Arc::new(Notify::new());
        (self
            .followers
            .lock()
            .unwrap_or_else(PoisonError::into_inner))
        .push(Arc::clone(&follower));
        follower
    }

    /// Wakes each follower. Only Probing calls it.
    pub(super) fn finished(&self) {
        let followers = self
            .followers
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        for follower in followers.iter() {
            // Not `notify_waiters`, which a follower still busy with the last pass would miss.
            follower.notify_one();
        }
    }
}

/// Deletes what was made of the files of Versions gone: their cached and downloaded subtitles and
/// their Preview frames.
#[derive(Debug, Clone)]
pub(crate) struct Leftovers {
    contents: Contents,
    subtitles: subtitles::Cache,
    images: ImageStore,
}

impl Leftovers {
    /// Keeps, of what `subtitles` caches and `images` holds, only what's of the files `db` stores.
    pub(crate) fn new(db: Arc<Database>, subtitles: subtitles::Cache, images: ImageStore) -> Self {
        Self {
            contents: Contents::new(db),
            subtitles,
            images,
        }
    }

    /// Throws away the subtitles cached for files gone, such as a deleted Library's, or changed
    /// since they were cached, and deletes the subtitles downloaded for Versions gone and their
    /// Preview frames, or logs why it couldn't.
    pub(crate) async fn clean(&self) {
        // ponytail: reads every stored file, and lists every downloaded subtitle, after each scan;
        // clean only the Library scanned once Libraries get big.
        let stored = match self.contents.every_stored().await {
            Ok(stored) => stored,
            Err(err) => {
                tracing::error!("could not read which files to keep subtitles for: {err}");
                return;
            }
        };
        if let Err(err) = self.subtitles.keep_only(&stored).await {
            tracing::error!("could not throw away subtitles of files gone or changed: {err}");
        }
        for dir in [DOWNLOADED_SUBTITLES_DIR, PREVIEW_FRAMES_DIR] {
            if let Err(err) = keep_only(&self.images, dir, &stored).await {
                tracing::error!("could not list {dir} to delete those of Versions gone: {err}");
            }
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

/// Runs reads of whole files in the background, [`WHOLE_FILE_READS_AT_ONCE`] at a time, shared by
/// Probing's keyframe listings and text passes.
#[derive(Debug, Clone)]
pub(crate) struct WholeFileReads {
    inner: Arc<Reads>,
}

#[derive(Debug)]
struct Reads {
    /// A permit per read running.
    running: Semaphore,
    /// The reads, so shutting down can wait for them.
    tasks: TaskTracker,
    /// Cancelled when the Server shuts down.
    cancel: CancellationToken,
}

impl Default for WholeFileReads {
    fn default() -> Self {
        Self {
            inner: Arc::new(Reads {
                running: Semaphore::new(WHOLE_FILE_READS_AT_ONCE),
                tasks: TaskTracker::new(),
                cancel: CancellationToken::new(),
            }),
        }
    }
}

impl WholeFileReads {
    /// Runs `read`, which reads a whole file, in `span` as a background task, so a request given
    /// up on doesn't stop it (#866), once fewer than [`WHOLE_FILE_READS_AT_ONCE`] such reads run.
    /// Shutting down cancels it. Returns `false`, dropping `read`, once the Server is shutting
    /// down.
    pub(super) fn spawn(
        &self,
        span: tracing::Span,
        read: impl Future<Output = ()> + Send + 'static,
    ) -> bool {
        // A read spawned after shutdown would be cancelled at once, and whoever waited for it
        // would spawn it again, round and round.
        if self.inner.cancel.is_cancelled() {
            return false;
        }
        let reads = Arc::clone(&self.inner);
        let read = async move {
            // The semaphore is never closed.
            if let Ok(_reading) = reads.running.acquire().await {
                read.await;
            }
        };
        let cancel = self.inner.cancel.clone();
        (self.inner.tasks)
            .spawn(async move { cancel.run_until_cancelled(read).await }.instrument(span));
        true
    }

    /// Cancels the reads running, waits for them to stop, and starts no more.
    pub(crate) async fn shutdown(&self) {
        self.inner.cancel.cancel();
        self.inner.tasks.close();
        self.inner.tasks.wait().await;
    }

    /// Waits until no read is running.
    #[cfg(test)]
    pub(super) async fn idle(&self) {
        self.inner.tasks.close();
        self.inner.tasks.wait().await;
        self.inner.tasks.reopen();
    }
}

#[cfg(test)]
#[path = "made_from_files_tests.rs"]
mod tests;
