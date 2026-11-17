use std::{
    collections::BTreeMap,
    io::{self, Cursor},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use axum::body::Body;
use image::{ImageFormat, RgbImage};
use tempfile::{NamedTempFile, TempDir};
use tokio::sync::oneshot;

use crate::{
    db::{BoxFuture, Database},
    images::{Blobs, ImageStore, MAX_BYTES, ORIGINAL_DIRS, Persist, SwitchRefused, Switching},
};

use super::Mirror;

/// A store that keeps its objects in memory, refusing to put a key `refused` matches.
#[derive(Debug)]
struct InMemory {
    objects: Mutex<BTreeMap<String, Vec<u8>>>,
    refused: fn(&str) -> bool,
}

impl Default for InMemory {
    fn default() -> Self {
        Self::refusing(|_key| false)
    }
}

impl InMemory {
    fn refusing(refused: fn(&str) -> bool) -> Self {
        Self {
            objects: Mutex::default(),
            refused,
        }
    }

    fn holding(keys: &[&str]) -> Self {
        let store = Self::default();
        for key in keys {
            store
                .objects()
                .insert((*key).to_owned(), key.as_bytes().to_vec());
        }
        store
    }

    fn objects(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, Vec<u8>>> {
        self.objects.lock().unwrap()
    }

    fn keys(&self) -> Vec<String> {
        self.objects().keys().cloned().collect()
    }

    fn store(&self, key: &str, bytes: Vec<u8>) -> io::Result<()> {
        if (self.refused)(key) {
            return Err(io::Error::other("refused"));
        }
        self.objects().insert(key.to_owned(), bytes);
        Ok(())
    }
}

impl Blobs for InMemory {
    fn put(&self, key: &str, bytes: Vec<u8>) -> BoxFuture<'_, io::Result<()>> {
        let stored = self.store(key, bytes);
        Box::pin(async { stored })
    }

    fn put_file(&self, key: &str, file: NamedTempFile) -> BoxFuture<'_, io::Result<()>> {
        let stored = std::fs::read(file.path()).and_then(|bytes| self.store(key, bytes));
        Box::pin(async { stored })
    }

    fn get(&self, key: &str) -> BoxFuture<'_, io::Result<Option<Vec<u8>>>> {
        let found = self.objects().get(key).cloned();
        Box::pin(async { Ok(found) })
    }

    fn delete(&self, key: &str) -> BoxFuture<'_, io::Result<()>> {
        self.objects().remove(key);
        Box::pin(async { Ok(()) })
    }

    fn list(&self, prefix: &str) -> BoxFuture<'_, io::Result<Vec<String>>> {
        let keys = self
            .keys()
            .into_iter()
            .filter(|key| key.starts_with(prefix))
            .collect();
        Box::pin(async { Ok(keys) })
    }

    fn exists(&self, key: &str) -> BoxFuture<'_, io::Result<bool>> {
        let found = self.objects().contains_key(key);
        Box::pin(async move { Ok(found) })
    }
}

/// A store that reads back other bytes than were written, as a misconfigured gateway might.
#[derive(Debug, Default)]
struct Garbling(InMemory);

impl Blobs for Garbling {
    fn put(&self, key: &str, bytes: Vec<u8>) -> BoxFuture<'_, io::Result<()>> {
        self.0.put(key, bytes)
    }

    fn put_file(&self, key: &str, file: NamedTempFile) -> BoxFuture<'_, io::Result<()>> {
        self.0.put_file(key, file)
    }

    fn get(&self, key: &str) -> BoxFuture<'_, io::Result<Option<Vec<u8>>>> {
        let found = self.0.objects().get(key).map(|_| b"garbled".to_vec());
        Box::pin(async { Ok(found) })
    }

    fn delete(&self, key: &str) -> BoxFuture<'_, io::Result<()>> {
        self.0.delete(key)
    }

    fn list(&self, prefix: &str) -> BoxFuture<'_, io::Result<Vec<String>>> {
        self.0.list(prefix)
    }

    fn exists(&self, key: &str) -> BoxFuture<'_, io::Result<bool>> {
        self.0.exists(key)
    }
}

const ORIGINALS: &[&str] = &[
    "artwork/tmdb/%2Fposter.jpg",
    "artwork-cache/tmdb/%2Foffered.jpg",
    "custom-images/movie/1/v1",
    "plugin-logos/tmdb/v1",
];

async fn store_over(data: &TempDir, old: &Arc<InMemory>) -> ImageStore {
    let db = Database::open(data.path()).await.unwrap();
    ImageStore::over(data.path(), Arc::new(db), as_blobs(old))
}

fn as_blobs(store: &Arc<InMemory>) -> Arc<dyn Blobs> {
    Arc::<InMemory>::clone(store)
}

fn saved() -> Persist {
    Box::new(|| Box::pin(async { Ok(()) }))
}

/// A `Persist` that waits for the sender to be dropped or sent to, so a test can act while the
/// switch is between copying and taking effect.
fn held() -> (Persist, oneshot::Sender<()>) {
    let (release, released) = oneshot::channel::<()>();
    let persist: Persist = Box::new(move || {
        Box::pin(async move {
            // Dropped or sent to, it releases the switch either way.
            let _released: Result<(), _> = released.await;
            Ok(())
        })
    });
    (persist, release)
}

/// The switch's progress once it is no longer running.
async fn finished(store: &ImageStore) -> Option<Switching> {
    for _ in 0..1000 {
        match store.switching() {
            Some(running) if running.failed.is_none() => {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            done => return done,
        }
    }
    panic!("the switch never finished");
}

/// Waits until every original is copied, the switch waiting on a `held` persist.
async fn copied(store: &ImageStore) -> Switching {
    for _ in 0..1000 {
        match store.switching() {
            Some(running) if running.total > 0 && running.copied == running.total => {
                return running;
            }
            _ => tokio::time::sleep(Duration::from_millis(5)).await,
        }
    }
    panic!("the switch never copied everything");
}

#[tokio::test]
async fn a_store_that_fails_its_probe_is_refused_and_nothing_is_copied() {
    let data = tempfile::tempdir().unwrap();
    let old = Arc::new(InMemory::holding(ORIGINALS));
    let store = store_over(&data, &old).await;
    let target = Arc::new(InMemory::refusing(|_key| true));

    let refused = store.switch_to(as_blobs(&target), "next", saved()).await;

    assert!(matches!(refused, Err(SwitchRefused::FailedProbe(_))));
    assert_eq!(target.keys(), Vec::<String>::new());
    assert_eq!(store.switching(), None);
    store.write(&["artwork", "after"], b"x").await.unwrap();
    assert_eq!(target.keys(), Vec::<String>::new());
}

#[tokio::test]
async fn a_probe_that_reads_back_other_bytes_leaves_no_object_behind() {
    let data = tempfile::tempdir().unwrap();
    let old = Arc::new(InMemory::holding(ORIGINALS));
    let store = store_over(&data, &old).await;
    let target = Arc::new(Garbling::default());

    let blobs: Arc<dyn Blobs> = Arc::<Garbling>::clone(&target);
    let refused = store.switch_to(blobs, "next", saved()).await;

    assert!(matches!(refused, Err(SwitchRefused::FailedProbe(_))));
    assert_eq!(target.0.keys(), Vec::<String>::new());
}

#[tokio::test]
async fn a_switch_copies_every_original_and_nothing_else_then_uses_the_new_store() {
    let data = tempfile::tempdir().unwrap();
    let mut held_keys = ORIGINALS.to_vec();
    held_keys.extend(["elsewhere/x", "artworks/y"]);
    let old = Arc::new(InMemory::holding(&held_keys));
    let store = store_over(&data, &old).await;
    let target = Arc::new(InMemory::default());
    let (persist, release) = held();

    store
        .switch_to(as_blobs(&target), "next", persist)
        .await
        .unwrap();
    let progress = copied(&store).await;
    release.send(()).unwrap();

    assert_eq!(progress.to, "next");
    assert_eq!(progress.total, ORIGINALS.len());
    assert_eq!(finished(&store).await, None);
    let mut originals = ORIGINALS.to_vec();
    originals.sort_unstable();
    assert_eq!(target.keys(), originals);
    for key in ORIGINALS {
        assert_eq!(target.objects()[*key], key.as_bytes());
    }
    store.write(&["artwork", "after"], b"x").await.unwrap();
    assert!(target.objects().contains_key("artwork/after"));
    assert!(!old.objects().contains_key("artwork/after"));
}

#[tokio::test]
async fn every_original_dir_is_a_folder_an_image_is_written_to() {
    assert_eq!(
        ORIGINAL_DIRS,
        [
            "artwork",
            "artwork-cache",
            "custom-images",
            "local-images",
            "plugin-logos"
        ]
    );
}

#[tokio::test]
async fn writes_during_a_switch_land_in_both_stores() {
    let data = tempfile::tempdir().unwrap();
    let old = Arc::new(InMemory::holding(ORIGINALS));
    let store = store_over(&data, &old).await;
    let target = Arc::new(InMemory::default());
    let (persist, release) = held();
    store
        .switch_to(as_blobs(&target), "next", persist)
        .await
        .unwrap();
    copied(&store).await;

    store.write(&["artwork", "new"], b"x").await.unwrap();
    let received = store
        .receive(&["custom-images", "movie", "2"], Body::from(png()), false)
        .await
        .unwrap();
    release.send(()).unwrap();

    let uploaded = format!("custom-images/movie/2/{}", received.version);
    for written in ["artwork/new", uploaded.as_str()] {
        assert!(old.objects().contains_key(written), "{written} not in old");
        assert!(
            target.objects().contains_key(written),
            "{written} not in new"
        );
    }
    assert_eq!(finished(&store).await, None);
}

#[tokio::test]
async fn a_second_switch_while_one_runs_is_refused() {
    let data = tempfile::tempdir().unwrap();
    let old = Arc::new(InMemory::holding(ORIGINALS));
    let store = store_over(&data, &old).await;
    let (persist, release) = held();
    store
        .switch_to(Arc::new(InMemory::default()), "next", persist)
        .await
        .unwrap();

    let second = store
        .switch_to(Arc::new(InMemory::default()), "next", saved())
        .await;

    assert!(matches!(second, Err(SwitchRefused::Running)));
    release.send(()).unwrap();
    assert_eq!(finished(&store).await, None);
}

#[tokio::test]
async fn a_switch_dropped_before_its_copy_starts_leaves_none_running() {
    let data = tempfile::tempdir().unwrap();
    let old = Arc::new(InMemory::holding(ORIGINALS));
    let store = store_over(&data, &old).await;
    let writing = store.write_lock("artwork/held").lock().await;

    let switch = store.switch_to(Arc::new(InMemory::default()), "next", saved());
    let dropped = tokio::time::timeout(Duration::from_millis(50), switch).await;

    assert!(
        dropped.is_err(),
        "the switch didn't wait for the write lock"
    );
    drop(writing);
    assert_eq!(store.switching(), None);
    store
        .switch_to(Arc::new(InMemory::default()), "next", saved())
        .await
        .unwrap();
    assert_eq!(finished(&store).await, None);
}

#[tokio::test]
async fn a_failed_copy_keeps_the_old_store_and_never_saves_the_new_one() {
    let data = tempfile::tempdir().unwrap();
    let old = Arc::new(InMemory::holding(ORIGINALS));
    let store = store_over(&data, &old).await;
    let target = Arc::new(InMemory::refusing(|key| key.starts_with("custom-images/")));
    let persisted = Arc::new(AtomicBool::new(false));
    let persist: Persist = {
        let persisted = Arc::clone(&persisted);
        Box::new(move || mark(&persisted))
    };

    store
        .switch_to(as_blobs(&target), "next", persist)
        .await
        .unwrap();

    let failed = finished(&store).await.unwrap().failed.unwrap();
    assert!(failed.contains("custom-images/movie/1/v1"), "{failed}");
    assert!(!persisted.load(Ordering::SeqCst));
    store.write(&["artwork", "after"], b"x").await.unwrap();
    assert!(old.objects().contains_key("artwork/after"));
    assert!(!target.objects().contains_key("artwork/after"));
}

#[tokio::test]
async fn a_mirror_refuses_a_file_over_the_object_limit_and_stores_it_in_neither() {
    let (old, new) = (Arc::new(InMemory::default()), Arc::new(InMemory::default()));
    let mirror = Mirror {
        old: as_blobs(&old),
        new: as_blobs(&new),
    };

    let refused = mirror
        .put_file("artwork/big", temp_file(&vec![0; MAX_BYTES + 1]))
        .await
        .unwrap_err();

    assert_eq!(refused.kind(), io::ErrorKind::FileTooLarge);
    assert!(refused.to_string().contains("artwork/big"), "{refused}");
    assert_eq!(old.keys(), Vec::<String>::new());
    assert_eq!(new.keys(), Vec::<String>::new());
}

#[tokio::test]
async fn a_mirror_stores_a_file_at_the_object_limit_in_both_stores() {
    let (old, new) = (Arc::new(InMemory::default()), Arc::new(InMemory::default()));
    let mirror = Mirror {
        old: as_blobs(&old),
        new: as_blobs(&new),
    };

    mirror
        .put_file("artwork/full", temp_file(&vec![7; MAX_BYTES]))
        .await
        .unwrap();

    assert_eq!(old.objects()["artwork/full"].len(), MAX_BYTES);
    assert_eq!(new.objects()["artwork/full"].len(), MAX_BYTES);
}

fn temp_file(bytes: &[u8]) -> NamedTempFile {
    let file = NamedTempFile::new().unwrap();
    std::fs::write(file.path(), bytes).unwrap();
    file
}

fn mark(persisted: &AtomicBool) -> BoxFuture<'static, io::Result<()>> {
    persisted.store(true, Ordering::SeqCst);
    Box::pin(async { Ok(()) })
}

fn png() -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    RgbImage::new(2, 2)
        .write_to(&mut bytes, ImageFormat::Png)
        .unwrap();
    bytes.into_inner()
}
