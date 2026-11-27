//! What the image store's unit tests share: a blob store in memory, and requests to serve.

use std::{
    collections::BTreeMap,
    io::{self, Cursor},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

use axum::{
    body::{Body, to_bytes},
    extract::Request,
    response::Response,
};
use image::ImageFormat;
use tempfile::{NamedTempFile, TempDir};
use tokio::sync::{Notify, Semaphore};

use super::{Blobs, ImageStore};
use crate::db::{BoxFuture, Database};

/// A store that keeps its objects in memory, so nothing it holds is on disk, and counts its
/// reads, failing each while `gets_fail` is set and holding each back while `held` is set.
#[derive(Debug, Default)]
pub(super) struct InMemory {
    pub(super) objects: Mutex<BTreeMap<String, Vec<u8>>>,
    pub(super) gets: AtomicUsize,
    pub(super) gets_fail: AtomicBool,
    pub(super) held: Option<Held>,
}

/// Reads held back: each tells `reading` once it has its bytes, then waits for a permit of `open`.
#[derive(Debug)]
pub(super) struct Held {
    pub(super) reading: Notify,
    pub(super) open: Semaphore,
}

impl InMemory {
    fn objects(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, Vec<u8>>> {
        self.objects.lock().unwrap()
    }

    pub(super) fn gets(&self) -> usize {
        self.gets.load(Ordering::SeqCst)
    }
}

impl Blobs for InMemory {
    fn put(&self, key: &str, bytes: Vec<u8>) -> BoxFuture<'_, io::Result<()>> {
        self.objects().insert(key.to_owned(), bytes);
        Box::pin(async { Ok(()) })
    }

    fn put_file(&self, key: &str, file: NamedTempFile) -> BoxFuture<'_, io::Result<()>> {
        let read = std::fs::read(file.path()).map(|bytes| {
            self.objects().insert(key.to_owned(), bytes);
        });
        Box::pin(async { read })
    }

    fn get(&self, key: &str) -> BoxFuture<'_, io::Result<Option<Vec<u8>>>> {
        self.gets.fetch_add(1, Ordering::SeqCst);
        let found = if self.gets_fail.load(Ordering::SeqCst) {
            Err(io::Error::other("the store is down"))
        } else {
            Ok(self.objects().get(key).cloned())
        };
        Box::pin(async move {
            if let Some(held) = &self.held {
                held.reading.notify_one();
                drop(held.open.acquire().await.unwrap());
            }
            found
        })
    }

    fn delete(&self, key: &str) -> BoxFuture<'_, io::Result<()>> {
        self.objects().remove(key);
        Box::pin(async { Ok(()) })
    }

    fn list(&self, prefix: &str) -> BoxFuture<'_, io::Result<Vec<String>>> {
        let keys = self
            .objects()
            .keys()
            .filter(|key| key.starts_with(prefix))
            .cloned()
            .collect();
        Box::pin(async { Ok(keys) })
    }

    fn exists(&self, key: &str) -> BoxFuture<'_, io::Result<bool>> {
        let found = self.objects().contains_key(key);
        Box::pin(async move { Ok(found) })
    }
}

pub(super) async fn in_memory(data: &TempDir) -> ImageStore {
    counted(data).await.0
}

/// A store over an [`InMemory`] one, and that one, to count its reads.
pub(super) async fn counted(data: &TempDir) -> (ImageStore, Arc<InMemory>) {
    let db = Database::open(data.path()).await.unwrap();
    let blobs = Arc::new(InMemory::default());
    let store = ImageStore::over(data.path(), Arc::new(db), Arc::<InMemory>::clone(&blobs));
    (store, blobs)
}

pub(super) async fn on_disk(data: &TempDir) -> ImageStore {
    let db = Database::open(data.path()).await.unwrap();
    ImageStore::new(data.path(), Arc::new(db))
}

pub(super) fn get() -> Request {
    Request::get("/").body(Body::empty()).unwrap()
}

pub(super) async fn body(response: Response) -> Vec<u8> {
    to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec()
}

/// `image` as a PNG.
pub(super) fn png(image: impl Into<image::DynamicImage>) -> Vec<u8> {
    let mut png = Cursor::new(Vec::new());
    image.into().write_to(&mut png, ImageFormat::Png).unwrap();
    png.into_inner()
}
