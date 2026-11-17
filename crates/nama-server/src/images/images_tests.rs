use axum::{body::Body, extract::Request};

use std::sync::Arc;

use tempfile::TempDir;

use axum::http::{HeaderMap, HeaderValue, header};

use super::{Format, ImageStore, file_name, serving::format_asked, unescape};
use crate::db::Database;

const KEY: &[&str] = &["artwork", "tmdb", "/poster.jpg"];

async fn store_in(data: &TempDir) -> ImageStore {
    let db = Database::open(data.path()).await.unwrap();
    ImageStore::new(data.path(), Arc::new(db))
}

#[tokio::test]
async fn a_failed_write_leaves_nothing_behind() {
    let data = tempfile::tempdir().unwrap();
    let store = store_in(&data).await;
    // A folder where the image goes, so the write can't put it there.
    let blocked = data.path().join("artwork/tmdb/%2Fposter.jpg");
    std::fs::create_dir_all(blocked.join("in-the-way")).unwrap();

    let written = store.write(KEY, b"\xff\xd8").await;

    assert!(written.is_err());
    let left: Vec<_> = std::fs::read_dir(blocked.parent().unwrap())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(left, ["%2Fposter.jpg"]);
}

#[tokio::test]
async fn writing_a_stored_key_again_keeps_the_first_file_and_its_etag() {
    let data = tempfile::tempdir().unwrap();
    let store = store_in(&data).await;
    let get = || Request::get("/").body(Body::empty()).unwrap();
    store.write(KEY, b"\xff\xd8first").await.unwrap();
    let first = store.serve(KEY, "image/jpeg", None, get()).await.unwrap();
    let etag = first.headers()[header::ETAG].clone();

    store.write(KEY, b"\xff\xd8second!").await.unwrap();

    let mut again = get();
    again.headers_mut().insert(header::IF_NONE_MATCH, etag);
    let response = store.serve(KEY, "image/jpeg", None, again).await.unwrap();
    assert_eq!(response.status(), 304);
    let folder = data.path().join("artwork/tmdb");
    assert_eq!(
        std::fs::read(folder.join("%2Fposter.jpg")).unwrap(),
        b"\xff\xd8first"
    );
    assert_eq!(std::fs::read_dir(folder).unwrap().count(), 1);
}

#[tokio::test]
async fn deleting_an_image_twice_leaves_none_stored() {
    let data = tempfile::tempdir().unwrap();
    let store = store_in(&data).await;
    store.write(KEY, b"\xff\xd8").await.unwrap();

    store.delete(KEY).await.unwrap();

    assert!(!store.contains(KEY).await.unwrap());
    store.delete(KEY).await.unwrap();
}

#[tokio::test]
async fn an_image_not_stored_is_not_found() {
    let data = tempfile::tempdir().unwrap();
    let request = Request::get("/").body(Body::empty()).unwrap();

    let response = store_in(&data)
        .await
        .serve(KEY, "image/jpeg", None, request)
        .await
        .unwrap();

    assert_eq!(response.status(), 404);
}

#[tokio::test]
async fn listing_gives_the_key_of_each_image_stored_under_a_folder() {
    let data = tempfile::tempdir().unwrap();
    let store = store_in(&data).await;
    store.write(KEY, b"\xff\xd8").await.unwrap();
    store.write(&["elsewhere", "x"], b"\xff\xd8").await.unwrap();
    // As a write leaves it while it is under way.
    std::fs::write(data.path().join("artwork/tmdb/.tmpAbC123"), b"").unwrap();

    let listed = store.list("artwork").await.unwrap();

    assert_eq!(listed, [KEY]);
    assert_eq!(
        store.list("nothing-yet").await.unwrap(),
        Vec::<Vec<String>>::new()
    );
}

#[test]
fn jpeg_xl_is_accepted_where_listed_at_a_quality_above_zero() {
    let accepts = |values: &[&'static str]| {
        let mut headers = HeaderMap::new();
        for value in values {
            headers.append(header::ACCEPT, HeaderValue::from_static(value));
        }
        format_asked(&headers) == Format::Jxl
    };

    assert!(accepts(&["image/avif, Image/JXL ;q=0.5"]));
    assert!(accepts(&["image/webp", "image/jxl"]));
    assert!(!accepts(&["image/jxl;q=0", "image/*"]));
    assert!(!accepts(&["image/jxl; q=0.000"]));
    assert!(!accepts(&["image/*,*/*;q=0.8"]));
    assert!(!accepts(&[]));
}

#[test]
fn a_file_name_unescapes_to_its_text_and_no_other_name_does() {
    for text in ["/poster.jpg", "a b/%.ünï", "tmdb"] {
        assert_eq!(unescape(&file_name(text)).as_deref(), Some(text));
    }
    for name in [".tmpAbC123", "%2f", "%2", "%ZZ", "a%2Fb%"] {
        assert_eq!(unescape(name), None, "{name}");
    }
}

#[tokio::test]
async fn sweeping_uploads_deletes_every_leftover_and_keeps_stored_images() {
    let data = tempfile::tempdir().unwrap();
    let store = store_in(&data).await;
    store.write(KEY, b"\xff\xd8").await.unwrap();
    let uploads = data.path().join("uploads");
    std::fs::create_dir_all(&uploads).unwrap();
    std::fs::write(uploads.join(".tmpAbC123"), b"half an upload").unwrap();
    std::fs::write(uploads.join("poster.jpg"), b"\xff\xd8").unwrap();

    store.sweep_uploads().await.unwrap();

    assert_eq!(std::fs::read_dir(&uploads).unwrap().count(), 0);
    assert!(store.contains(KEY).await.unwrap());
}

#[tokio::test]
async fn sweeping_uploads_without_an_uploads_folder_is_no_error() {
    let data = tempfile::tempdir().unwrap();

    store_in(&data).await.sweep_uploads().await.unwrap();
}
