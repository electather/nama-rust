//! Moving the originals to another store while the Server keeps serving them: the new store is
//! probed, every original copied across one at a time while writes go to both stores, and only
//! then, once the caller has saved the choice, made the store in use.

use std::{
    io,
    path::PathBuf,
    sync::{Arc, MutexGuard, PoisonError},
};

use tempfile::NamedTempFile;

use super::{Blobs, ImageStore, MAX_BYTES, ORIGINAL_DIRS};
use crate::db::BoxFuture;

/// The key a probe writes and deletes: outside every folder of originals, so no copy or listing
/// ever takes it for an image.
const PROBE_KEY: &str = "store-probe/probe";

const PROBE_BYTES: &[u8] = b"Nama checks it can write, read and delete here";

/// Saves the new store as the Server's setting, called once every original is copied.
pub(crate) type Persist = Box<dyn FnOnce() -> BoxFuture<'static, io::Result<()>> + Send>;

/// How far a switch to another store has got.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Switching {
    /// The new store, as the caller named it.
    pub(crate) to: String,
    /// The originals handled so far, those the new store already held included.
    pub(crate) copied: usize,
    /// The originals to handle, once listed.
    pub(crate) total: usize,
    /// Why the switch stopped, the old store still in use; `None` while it runs.
    pub(crate) failed: Option<String>,
}

/// Why a switch to another store didn't start.
#[derive(Debug, thiserror::Error)]
pub(crate) enum SwitchRefused {
    /// Another switch hasn't finished.
    #[error("a switch to another image store is already running")]
    Running,
    /// The new store failed a write, read-back, delete or check that the object is gone.
    #[error("the new image store failed its check: {0}")]
    FailedProbe(String),
}

impl ImageStore {
    /// Starts moving the originals to `target`, which [`Switching`] names `to`, refusing if a
    /// switch is running or `target` fails a probe. The copy runs in the background, watched
    /// through [`Self::switching`]; `persist` is called once it is done, and `target` is made the
    /// store in use only if that succeeds. A failure keeps the old store in use.
    ///
    /// # Errors
    ///
    /// [`SwitchRefused`] says why the switch didn't start.
    pub(crate) async fn switch_to(
        &self,
        target: Arc<dyn Blobs>,
        to: &str,
        persist: Persist,
    ) -> Result<(), SwitchRefused> {
        {
            let mut switching = self.progress();
            if switching
                .as_ref()
                .is_some_and(|running| running.failed.is_none())
            {
                return Err(SwitchRefused::Running);
            }
            *switching = Some(Switching {
                to: to.to_owned(),
                ..Switching::default()
            });
        }
        let mut started = Unmark(Some(self));
        probe(target.as_ref())
            .await
            .map_err(SwitchRefused::FailedProbe)?;
        let old = self.blobs();
        {
            // A write that took the old store before the mirror is in place finishes first, so
            // the copy's listing sees it.
            let mut writes = Vec::with_capacity(self.writing.len());
            for lock in self.writing.iter() {
                writes.push(lock.lock().await);
            }
            let mirror = Mirror {
                old: Arc::clone(&old),
                new: Arc::clone(&target),
            };
            self.make_current(Arc::new(mirror));
            drop(writes);
        }
        started.0 = None;
        let store = self.clone();
        tokio::spawn(async move { store.finish_switch(old, target, persist).await });
        Ok(())
    }

    /// The running switch, or the last one if it failed; `None` once one succeeds.
    pub(crate) fn switching(&self) -> Option<Switching> {
        self.progress().clone()
    }

    fn progress(&self) -> MutexGuard<'_, Option<Switching>> {
        self.switching
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Keeps the originals in `store` from now on, copying none: for startup, once the Server
    /// settings are read, to use the store they name before anything reads an image. While the
    /// Server runs, [`Self::switch_to`] moves them instead.
    pub(crate) fn keep_in(&self, store: Arc<dyn Blobs>) {
        self.make_current(store);
    }

    fn make_current(&self, store: Arc<dyn Blobs>) {
        *self.store.write().unwrap_or_else(PoisonError::into_inner) = store;
    }

    // ponytail: a panic in here leaves the mirror in use and the switch shown as running until
    // the Server restarts, the old store still holding everything.
    async fn finish_switch(&self, old: Arc<dyn Blobs>, target: Arc<dyn Blobs>, persist: Persist) {
        let finished = match self.copy_originals(old.as_ref(), target.as_ref()).await {
            Ok(()) => persist()
                .await
                .map_err(|err| format!("the new image store couldn't be saved: {err}")),
            Err(why) => Err(why),
        };
        match finished {
            Ok(()) => {
                self.make_current(target);
                *self.progress() = None;
            }
            Err(why) => {
                tracing::warn!("the switch to another image store failed: {why}");
                self.make_current(old);
                if let Some(switching) = self.progress().as_mut() {
                    switching.failed = Some(why);
                }
            }
        }
    }

    /// Copies each original under [`ORIGINAL_DIRS`] in `old` that `target` lacks, one object at
    /// a time.
    async fn copy_originals(&self, old: &dyn Blobs, target: &dyn Blobs) -> Result<(), String> {
        let mut keys = Vec::new();
        for dir in ORIGINAL_DIRS {
            let listed = old.list(&format!("{dir}/")).await;
            keys.extend(listed.map_err(|err| format!("couldn't list {dir}: {err}"))?);
        }
        if let Some(switching) = self.progress().as_mut() {
            switching.total = keys.len();
        }
        for key in keys {
            {
                // A delete of `key` while it is copied would otherwise leave it in `target`.
                let _writing = self.write_lock(&key).lock().await;
                copy(old, target, &key)
                    .await
                    .map_err(|err| format!("couldn't copy {key}: {err}"))?;
            }
            if let Some(switching) = self.progress().as_mut() {
                switching.copied += 1;
            }
        }
        Ok(())
    }
}

/// Clears the mark of a running switch when dropped holding a store: when `switch_to` refuses,
/// or its caller drops it, before the copy starts.
struct Unmark<'a>(Option<&'a ImageStore>);

impl Drop for Unmark<'_> {
    fn drop(&mut self) {
        if let Some(store) = self.0 {
            *store.progress() = None;
        }
    }
}

async fn copy(old: &dyn Blobs, target: &dyn Blobs, key: &str) -> io::Result<()> {
    if target.exists(key).await? {
        return Ok(());
    }
    match old.get(key).await? {
        Some(bytes) => target.put(key, bytes).await,
        // Deleted since it was listed.
        None => Ok(()),
    }
}

/// Writes, reads back, deletes and looks for an object in `target`, saying in plain words what
/// failed.
async fn probe(target: &dyn Blobs) -> Result<(), String> {
    target
        .put(PROBE_KEY, PROBE_BYTES.to_vec())
        .await
        .map_err(|err| format!("couldn't write to it: {err}"))?;
    let read_back = match target.get(PROBE_KEY).await {
        Ok(Some(bytes)) if bytes == PROBE_BYTES => Ok(()),
        Ok(Some(_)) => Err("it gave back other bytes than were written".to_owned()),
        Ok(None) => Err("it lost an object just written".to_owned()),
        Err(err) => Err(format!("couldn't read from it: {err}")),
    };
    if let Err(failed) = read_back {
        // The written object would otherwise stay in the refused store; a failed delete says no
        // more than the read already did.
        drop(target.delete(PROBE_KEY).await);
        return Err(failed);
    }
    target
        .delete(PROBE_KEY)
        .await
        .map_err(|err| format!("couldn't delete from it: {err}"))?;
    match target.exists(PROBE_KEY).await {
        Ok(false) => Ok(()),
        Ok(true) => Err("it still holds an object after deleting it".to_owned()),
        Err(err) => Err(format!("couldn't check an object was deleted: {err}")),
    }
}

/// The store in use while a switch runs: it reads from the old store and writes to both, so
/// neither misses a write made before the switch ends either way.
#[derive(Debug)]
struct Mirror {
    old: Arc<dyn Blobs>,
    new: Arc<dyn Blobs>,
}

impl Blobs for Mirror {
    fn put(&self, key: &str, bytes: Vec<u8>) -> BoxFuture<'_, io::Result<()>> {
        let key = key.to_owned();
        Box::pin(async move {
            self.new.put(&key, bytes.clone()).await?;
            self.old.put(&key, bytes).await
        })
    }

    fn put_file(&self, key: &str, file: NamedTempFile) -> BoxFuture<'_, io::Result<()>> {
        let key = key.to_owned();
        Box::pin(async move {
            // Read into the new store first: the old one may rename `file` away.
            let size = tokio::fs::metadata(file.path()).await?.len();
            if size > MAX_BYTES as u64 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("{key} is over the store's object size limit"),
                ));
            }
            let bytes = tokio::fs::read(file.path()).await?;
            self.new.put(&key, bytes).await?;
            self.old.put_file(&key, file).await
        })
    }

    fn local_path(&self, key: &str) -> Option<PathBuf> {
        self.old.local_path(key)
    }

    fn get(&self, key: &str) -> BoxFuture<'_, io::Result<Option<Vec<u8>>>> {
        self.old.get(key)
    }

    fn delete(&self, key: &str) -> BoxFuture<'_, io::Result<()>> {
        let key = key.to_owned();
        Box::pin(async move {
            self.new.delete(&key).await?;
            self.old.delete(&key).await
        })
    }

    fn list(&self, prefix: &str) -> BoxFuture<'_, io::Result<Vec<String>>> {
        self.old.list(prefix)
    }

    fn exists(&self, key: &str) -> BoxFuture<'_, io::Result<bool>> {
        self.old.exists(key)
    }
}

#[cfg(test)]
#[path = "switch_tests.rs"]
mod tests;
