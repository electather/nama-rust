use std::{
    collections::BTreeMap,
    io::{self, Cursor},
    sync::{Arc, Mutex},
    time::Duration,
};

use axum::{
    body::{Body, to_bytes},
    extract::Request,
    http::{HeaderValue, header},
    response::Response,
};
use http_body_util::BodyExt as _;
use image::{ImageFormat, ImageReader, RgbImage};
use tempfile::{NamedTempFile, TempDir};

use super::super::{Blobs, ImageStore, MAX_BYTES, Resize, WHOLE_READS};
use crate::db::{BoxFuture, Database};

const KEY: &[&str] = &["artwork", "tmdb", "/poster.jpg"];

/// A store that keeps its objects in memory, so nothing it holds is on disk.
#[derive(Debug, Default)]
struct InMemory(Mutex<BTreeMap<String, Vec<u8>>>);

impl InMemory {
    fn objects(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, Vec<u8>>> {
        self.0.lock().unwrap()
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
        let found = self.objects().get(key).cloned();
        Box::pin(async { Ok(found) })
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

async fn in_memory(data: &TempDir) -> ImageStore {
    let db = Database::open(data.path()).await.unwrap();
    ImageStore::over(data.path(), Arc::new(db), Arc::new(InMemory::default()))
}

async fn on_disk(data: &TempDir) -> ImageStore {
    let db = Database::open(data.path()).await.unwrap();
    ImageStore::new(data.path(), Arc::new(db))
}

fn get() -> Request {
    Request::get("/").body(Body::empty()).unwrap()
}

async fn etag(store: &ImageStore) -> HeaderValue {
    let response = store.serve(KEY, "image/jpeg", None, get()).await.unwrap();
    assert_eq!(response.status(), 200);
    response.headers()[header::ETAG].clone()
}

async fn body(response: Response) -> Vec<u8> {
    to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec()
}

#[tokio::test]
async fn an_original_is_sent_from_its_store_with_its_headers() {
    let data = tempfile::tempdir().unwrap();
    let store = in_memory(&data).await;
    store.write(KEY, b"\xff\xd8poster").await.unwrap();

    let response = store.serve(KEY, "image/jpeg", None, get()).await.unwrap();

    assert_eq!(response.status(), 200);
    let headers = response.headers();
    assert_eq!(headers[header::CONTENT_TYPE], "image/jpeg");
    assert_eq!(headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
    assert_eq!(
        headers[header::CACHE_CONTROL],
        "private, max-age=31536000, immutable"
    );
    assert_eq!(body(response).await, b"\xff\xd8poster");
}

#[tokio::test]
async fn the_etag_of_a_key_stays_when_it_is_written_again() {
    let data = tempfile::tempdir().unwrap();
    let store = in_memory(&data).await;
    store.write(KEY, b"\xff\xd8first").await.unwrap();
    let first = etag(&store).await;

    store.delete(KEY).await.unwrap();
    store.write(KEY, b"\xff\xd8second").await.unwrap();

    assert_eq!(etag(&store).await, first);
}

#[tokio::test]
async fn the_etag_of_a_key_is_the_same_in_every_store() {
    let (memory, disk) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (memory, disk) = (in_memory(&memory).await, on_disk(&disk).await);
    memory.write(KEY, b"\xff\xd8poster").await.unwrap();
    disk.write(KEY, b"\xff\xd8poster").await.unwrap();

    assert_eq!(etag(&memory).await, etag(&disk).await);
}

#[tokio::test]
async fn a_matching_if_none_match_is_not_modified_in_every_store() {
    let (memory, disk) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    for store in [in_memory(&memory).await, on_disk(&disk).await] {
        store.write(KEY, b"\xff\xd8poster").await.unwrap();
        let tag = etag(&store).await;
        let mut again = get();
        let listed = format!("\"other\", W/{}", tag.to_str().unwrap());
        again
            .headers_mut()
            .insert(header::IF_NONE_MATCH, listed.parse().unwrap());

        let response = store.serve(KEY, "image/jpeg", None, again).await.unwrap();

        assert_eq!(response.status(), 304);
        assert_eq!(response.headers()[header::ETAG], tag);
        assert_eq!(
            response.headers()[header::CACHE_CONTROL],
            "private, max-age=31536000, immutable"
        );
        assert_eq!(body(response).await, b"");
    }
}

#[tokio::test]
async fn an_original_on_local_disk_answers_range_with_its_keys_etag() {
    let (memory, disk) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (memory, disk) = (in_memory(&memory).await, on_disk(&disk).await);
    memory.write(KEY, b"\xff\xd8poster").await.unwrap();
    disk.write(KEY, b"\xff\xd8poster").await.unwrap();
    let mut ranged = get();
    ranged
        .headers_mut()
        .insert(header::RANGE, HeaderValue::from_static("bytes=2-5"));

    let response = disk.serve(KEY, "image/jpeg", None, ranged).await.unwrap();

    assert_eq!(response.status(), 206);
    let headers = response.headers();
    assert_eq!(headers[header::ACCEPT_RANGES], "bytes");
    assert_eq!(headers[header::CONTENT_TYPE], "image/jpeg");
    assert_eq!(headers[header::ETAG], etag(&memory).await);
    assert_eq!(body(response).await, b"post");
}

#[tokio::test]
async fn an_original_answers_if_match_by_its_keys_etag_in_every_store() {
    let (memory, disk) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    for store in [in_memory(&memory).await, on_disk(&disk).await] {
        store.write(KEY, b"\xff\xd8poster").await.unwrap();
        let mut matching = get();
        let headers = matching.headers_mut();
        headers.insert(header::IF_MATCH, etag(&store).await);
        headers.insert(
            header::IF_UNMODIFIED_SINCE,
            HeaderValue::from_static("Thu, 01 Jan 1970 00:00:00 GMT"),
        );

        let response = store
            .serve(KEY, "image/jpeg", None, matching)
            .await
            .unwrap();

        assert_eq!(response.status(), 200);
        assert!(!response.headers().contains_key(header::LAST_MODIFIED));
        assert_eq!(body(response).await, b"\xff\xd8poster");
    }
}

#[tokio::test]
async fn a_large_original_on_local_disk_is_sent_in_chunks() {
    let data = tempfile::tempdir().unwrap();
    let store = on_disk(&data).await;
    store.write(KEY, &vec![0; MAX_BYTES]).await.unwrap();

    let response = store.serve(KEY, "image/jpeg", None, get()).await.unwrap();

    let first = response.into_body().frame().await.unwrap().unwrap();
    let chunk = first.into_data().unwrap();
    assert!(
        chunk.len() < MAX_BYTES,
        "sent {} bytes at once",
        chunk.len()
    );
}

#[tokio::test]
async fn a_whole_read_past_the_cap_waits_for_one_to_be_sent() {
    let data = tempfile::tempdir().unwrap();
    let store = in_memory(&data).await;
    store.write(KEY, b"\xff\xd8poster").await.unwrap();
    let mut held = Vec::new();
    for _ in 0..WHOLE_READS {
        held.push(store.serve(KEY, "image/jpeg", None, get()).await.unwrap());
    }
    let waiting = store.serve(KEY, "image/jpeg", None, get());
    tokio::pin!(waiting);

    let past_the_cap = tokio::time::timeout(Duration::from_millis(50), &mut waiting).await;
    assert!(past_the_cap.is_err(), "a read past the cap didn't wait");
    body(held.pop().unwrap()).await;
    let response = waiting.await.unwrap();

    assert_eq!(body(response).await, b"\xff\xd8poster");
}

#[tokio::test]
async fn a_missing_key_is_not_found_even_if_its_etag_is_sent() {
    let data = tempfile::tempdir().unwrap();
    let store = in_memory(&data).await;
    store.write(KEY, b"\xff\xd8poster").await.unwrap();
    let tag = etag(&store).await;
    store.delete(KEY).await.unwrap();
    let mut again = get();
    again.headers_mut().insert(header::IF_NONE_MATCH, tag);

    let plain = store.serve(KEY, "image/jpeg", None, get()).await.unwrap();
    let asked = store.serve(KEY, "image/jpeg", None, again).await.unwrap();

    assert_eq!(plain.status(), 404);
    assert_eq!(asked.status(), 404);
}

#[tokio::test]
async fn an_image_is_resized_from_a_store_not_on_disk() {
    let data = tempfile::tempdir().unwrap();
    let store = in_memory(&data).await;
    let mut png = Cursor::new(Vec::new());
    RgbImage::new(1000, 1500)
        .write_to(&mut png, ImageFormat::Png)
        .unwrap();
    store.write(KEY, png.get_ref()).await.unwrap();
    let resize = Resize {
        width: 320,
        cache_limit: u64::MAX,
    };

    let response = store
        .serve(KEY, "image/png", Some(resize), get())
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()[header::CONTENT_TYPE], "image/jpeg");
    let copy = ImageReader::new(Cursor::new(body(response).await))
        .with_guessed_format()
        .unwrap();
    assert_eq!(copy.into_dimensions().unwrap(), (320, 480));
    assert!(!data.path().join("artwork").exists());
}
