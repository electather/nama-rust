use std::{
    fs::{File, FileTimes},
    sync::{Arc, atomic::Ordering},
    time::{Duration, SystemTime},
};

use axum::{body::Body, extract::Request, http::header};
use image::{RgbImage, RgbaImage};
use tempfile::TempDir;
use tokio::sync::{Notify, Semaphore};

use super::{
    super::{
        ImageStore, Resize,
        testing::{Held, InMemory, body, counted, get, in_memory, on_disk, png},
    },
    copies, evict_listed, touch,
};
use crate::db::{self, Database};

const KEY: &[&str] = &["artwork", "tmdb", "/poster.jpg"];

const AT_320: Resize = Resize {
    width: 320,
    cache_limit: u64::MAX,
};

#[tokio::test]
async fn a_second_copy_ahead_of_an_image_no_wider_than_asked_does_not_read_it_again() {
    let data = tempfile::tempdir().unwrap();
    let (store, blobs) = counted(&data).await;
    store
        .write(KEY, &png(RgbImage::new(300, 450)))
        .await
        .unwrap();
    store.copy_ahead(KEY, &[AT_320]).await;
    assert_eq!(blobs.gets(), 2, "one read for each format");

    store.copy_ahead(KEY, &[AT_320]).await;

    assert_eq!(blobs.gets(), 2);
}

#[tokio::test]
async fn a_second_copy_ahead_of_a_blob_that_is_no_image_does_not_read_it_again() {
    let data = tempfile::tempdir().unwrap();
    let (store, blobs) = counted(&data).await;
    store.write(KEY, b"not an image").await.unwrap();
    store.copy_ahead(KEY, &[AT_320]).await;

    store.copy_ahead(KEY, &[AT_320]).await;

    assert_eq!(blobs.gets(), 2);
}

#[tokio::test]
async fn a_transparent_png_is_read_once_as_a_jpeg_and_still_resized_as_a_jpeg_xl() {
    let data = tempfile::tempdir().unwrap();
    let (store, blobs) = counted(&data).await;
    let transparent = RgbaImage::from_pixel(1000, 1500, [0, 0, 0, 0].into());
    store.write(KEY, &png(transparent)).await.unwrap();
    store.copy_ahead(KEY, &[AT_320]).await;

    store.copy_ahead(KEY, &[AT_320]).await;
    let jxl = Request::get("/")
        .header(header::ACCEPT, "image/jxl")
        .body(Body::empty())
        .unwrap();
    let sent = store
        .serve(KEY, "image/png", Some(AT_320), jxl)
        .await
        .unwrap();

    assert_eq!(blobs.gets(), 2, "one read for each format");
    assert_eq!(sent.headers()[header::CONTENT_TYPE], "image/jxl");
}

#[tokio::test]
async fn a_failed_read_is_tried_again_by_the_next_copy_ahead() {
    let data = tempfile::tempdir().unwrap();
    let (store, blobs) = counted(&data).await;
    store
        .write(KEY, &png(RgbImage::new(300, 450)))
        .await
        .unwrap();
    blobs.gets_fail.store(true, Ordering::SeqCst);
    store.copy_ahead(KEY, &[AT_320]).await;
    blobs.gets_fail.store(false, Ordering::SeqCst);

    store.copy_ahead(KEY, &[AT_320]).await;
    store.copy_ahead(KEY, &[AT_320]).await;

    assert_eq!(
        blobs.gets(),
        4,
        "two failed reads, then one for each format"
    );
}

#[tokio::test]
async fn a_failed_resize_is_tried_again_by_the_next_copy_ahead() {
    let data = tempfile::tempdir().unwrap();
    let (store, blobs) = counted(&data).await;
    store.write(KEY, b"\x89PNG\r\n\x1a\nbroken").await.unwrap();
    store.copy_ahead(KEY, &[AT_320]).await;

    store.copy_ahead(KEY, &[AT_320]).await;

    assert_eq!(blobs.gets(), 4);
}

#[tokio::test]
async fn deleting_an_image_while_it_is_copied_ahead_waits_and_leaves_no_copies() {
    let data = tempfile::tempdir().unwrap();
    let db = Database::open(data.path()).await.unwrap();
    let blobs = Arc::new(InMemory {
        held: Some(Held {
            reading: Notify::new(),
            open: Semaphore::new(0),
        }),
        ..InMemory::default()
    });
    let store = ImageStore::over(data.path(), Arc::new(db), Arc::<InMemory>::clone(&blobs));
    store
        .write(KEY, &png(RgbImage::new(1000, 1500)))
        .await
        .unwrap();
    let copying = tokio::spawn({
        let store = store.clone();
        async move { store.copy_ahead(KEY, &[AT_320]).await }
    });
    let held = blobs.held.as_ref().unwrap();
    held.reading.notified().await;

    let mut deleting = tokio::spawn({
        let store = store.clone();
        async move { store.delete(KEY).await }
    });

    // The read is held, so a delete that finishes meanwhile didn't wait for the resize.
    let waited = tokio::time::timeout(Duration::from_millis(100), &mut deleting).await;
    assert!(waited.is_err(), "the delete didn't wait for the resize");
    held.open.add_permits(1);
    copying.await.unwrap();
    deleting.await.unwrap().unwrap();
    assert!(!store.copies_path(KEY).exists());
    assert_eq!(store.thumbhashes("artwork").await.unwrap(), []);
}

#[tokio::test]
async fn a_larger_image_stored_after_deleting_one_left_unresized_is_resized() {
    let data = tempfile::tempdir().unwrap();
    let store = in_memory(&data).await;
    store
        .write(KEY, &png(RgbImage::new(300, 450)))
        .await
        .unwrap();
    store.copy_ahead(KEY, &[AT_320]).await;
    store.delete(KEY).await.unwrap();
    store
        .write(KEY, &png(RgbImage::new(1000, 1500)))
        .await
        .unwrap();

    let sent = store
        .serve(KEY, "image/png", Some(AT_320), get())
        .await
        .unwrap();

    assert_eq!(sent.headers()[header::CONTENT_TYPE], "image/jpeg");
}

#[tokio::test]
async fn an_image_marked_unresized_is_sent_as_stored_with_the_same_headers() {
    let data = tempfile::tempdir().unwrap();
    let (store, blobs) = counted(&data).await;
    let original = png(RgbImage::new(300, 450));
    store.write(KEY, &original).await.unwrap();
    let first = store
        .serve(KEY, "image/png", Some(AT_320), get())
        .await
        .unwrap();
    let read = blobs.gets();

    let again = store
        .serve(KEY, "image/png", Some(AT_320), get())
        .await
        .unwrap();

    assert_eq!(blobs.gets(), read + 1, "read only to send it");
    assert_eq!(again.status(), first.status());
    assert_eq!(again.headers(), first.headers());
    assert_eq!(body(again).await, original);
}

/// A copy 320 px wide, in a cache of 1 MB holding a file of `filler` bytes already.
fn at_320_beside(data: &TempDir, filler: u64) -> Resize {
    let cache = data.path().join("resized");
    std::fs::create_dir_all(&cache).unwrap();
    let file = File::create(cache.join("filler")).unwrap();
    file.set_len(filler).unwrap();
    Resize {
        width: 320,
        cache_limit: 1_000_000,
    }
}

#[tokio::test]
async fn no_copy_is_made_ahead_once_the_cache_holds_80_percent_but_a_request_still_gets_one() {
    let data = tempfile::tempdir().unwrap();
    let store = on_disk(&data).await;
    store
        .write(KEY, &png(RgbImage::new(1000, 1500)))
        .await
        .unwrap();
    let resize = at_320_beside(&data, 800_000);

    store.copy_ahead(KEY, &[resize]).await;

    assert!(!store.copies_path(KEY).exists(), "no copy made ahead");
    assert!(
        data.path().join("resized/filler").exists(),
        "nothing evicted"
    );
    let sent = store
        .serve(KEY, "image/png", Some(resize), get())
        .await
        .unwrap();
    assert_eq!(sent.headers()[header::CONTENT_TYPE], "image/jpeg");
}

#[tokio::test]
async fn a_copy_is_made_ahead_while_the_cache_holds_under_80_percent() {
    let data = tempfile::tempdir().unwrap();
    let store = on_disk(&data).await;
    store
        .write(KEY, &png(RgbImage::new(1000, 1500)))
        .await
        .unwrap();
    let resize = at_320_beside(&data, 799_999);

    store.copy_ahead(KEY, &[resize]).await;

    assert!(store.copies_path(KEY).join("320.jpg").exists());
}

#[tokio::test]
async fn copies_ahead_resume_once_deleting_an_image_frees_the_cache() {
    let data = tempfile::tempdir().unwrap();
    let store = on_disk(&data).await;
    let other: &[&str] = &["artwork", "tmdb", "/other.jpg"];
    for key in [KEY, other] {
        let wide = png(RgbImage::new(1000, 1500));
        store.write(key, &wide).await.unwrap();
    }
    store.copy_ahead(other, &[AT_320]).await;
    let files = walkdir::WalkDir::new(store.copies_path(other)).into_iter();
    // Files only: a folder's own size (4 KiB on ext4) would leave the limit well above the copies.
    let taken: u64 = (files.map(Result::unwrap))
        .filter(|entry| entry.file_type().is_file())
        .map(|file| file.metadata().unwrap().len())
        .sum();
    let full = Resize {
        width: 320,
        cache_limit: taken,
    };
    store.copy_ahead(KEY, &[full]).await;
    assert!(!store.copies_path(KEY).exists(), "skipped while full");

    store.delete(other).await.unwrap();
    store.copy_ahead(KEY, &[full]).await;

    assert!(store.copies_path(KEY).join("320.jpg").exists());
}

#[tokio::test]
async fn deleting_a_folder_while_an_image_under_it_is_copied_ahead_leaves_no_thumbhash_or_copies() {
    let data = tempfile::tempdir().unwrap();
    let db = Database::open(data.path()).await.unwrap();
    let blobs = Arc::new(InMemory {
        held: Some(Held {
            reading: Notify::new(),
            open: Semaphore::new(0),
        }),
        ..InMemory::default()
    });
    let store = ImageStore::over(data.path(), Arc::new(db), Arc::<InMemory>::clone(&blobs));
    store
        .write(KEY, &png(RgbImage::new(1000, 1500)))
        .await
        .unwrap();
    let copying = tokio::spawn({
        let store = store.clone();
        async move { store.copy_ahead(KEY, &[AT_320]).await }
    });
    let held = blobs.held.as_ref().unwrap();
    held.reading.notified().await;

    let mut deleting = tokio::spawn({
        let store = store.clone();
        async move { store.delete_dir(&KEY[..2]).await }
    });

    // The delete forgets the folder, then waits on the image's write lock, which the resize holds.
    let waited = tokio::time::timeout(Duration::from_millis(100), &mut deleting).await;
    assert!(waited.is_err(), "the delete didn't wait for the resize");
    held.open.add_permits(1);
    copying.await.unwrap();
    deleting.await.unwrap().unwrap();

    assert_eq!(store.thumbhashes("artwork").await.unwrap(), []);
    assert!(!store.copies_path(&KEY[..2]).exists());
}

#[test]
fn eviction_counts_a_copy_deleted_after_listing_as_evicted_and_carries_on_to_the_limit() {
    let cache = tempfile::tempdir().unwrap();
    let copy = |name: &str, sent_secs: u64| {
        let path = cache.path().join(name);
        let file = File::create(&path).unwrap();
        file.set_len(100).unwrap();
        let sent = SystemTime::UNIX_EPOCH + Duration::from_secs(sent_secs);
        file.set_times(FileTimes::new().set_accessed(sent)).unwrap();
        path
    };
    let (deleted, next, made) = (copy("oldest", 1), copy("next", 2), copy("made", 3));
    let listed = copies(cache.path()).unwrap();
    std::fs::remove_file(&deleted).unwrap();

    let left = evict_listed(listed, 100, &made).unwrap();

    assert_eq!(left, 100);
    assert!(!next.exists());
    assert!(made.exists());
}

#[test]
fn touching_a_copy_after_the_clock_moves_marks_it_sent_later() {
    let copy = tempfile::NamedTempFile::new().unwrap();
    let accessed = || std::fs::metadata(copy.path()).unwrap().accessed().unwrap();

    touch(copy.path()).unwrap();
    let first = accessed();
    db::advance_clock(Duration::from_secs(5));
    touch(copy.path()).unwrap();

    assert!(accessed() >= first + Duration::from_secs(5));
}
