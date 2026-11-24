use std::{fs, io::Write as _};

use tempfile::{NamedTempFile, TempDir};

use super::{Blobs as _, LocalDisk};

fn disk() -> (TempDir, LocalDisk) {
    let data = tempfile::tempdir().unwrap();
    let disk = LocalDisk::new(data.path());
    (data, disk)
}

#[tokio::test]
async fn a_put_object_is_got_back_from_the_file_its_key_names() {
    let (data, disk) = disk();

    disk.put("artwork/tmdb/%2Fposter.jpg", b"poster".to_vec())
        .await
        .unwrap();

    let got = disk.get("artwork/tmdb/%2Fposter.jpg").await.unwrap();
    assert_eq!(got.as_deref(), Some(&b"poster"[..]));
    let file = data.path().join("artwork/tmdb/%2Fposter.jpg");
    assert_eq!(fs::read(file).unwrap(), b"poster");
    assert_eq!(disk.get("artwork/tmdb/missing").await.unwrap(), None);
}

#[tokio::test]
async fn a_put_replaces_the_object_at_its_key() {
    let (data, disk) = disk();
    disk.put("a/b", b"first".to_vec()).await.unwrap();

    disk.put("a/b", b"second".to_vec()).await.unwrap();

    let got = disk.get("a/b").await.unwrap();
    assert_eq!(got.as_deref(), Some(&b"second"[..]));
    assert_eq!(fs::read_dir(data.path().join("a")).unwrap().count(), 1);
}

#[tokio::test]
async fn put_file_renames_the_file_into_place() {
    let (data, disk) = disk();
    let uploads = data.path().join("uploads");
    fs::create_dir_all(&uploads).unwrap();
    let mut file = NamedTempFile::new_in(&uploads).unwrap();
    file.write_all(b"upload").unwrap();

    disk.put_file("custom-images/movie/v1", file).await.unwrap();

    let stored = data.path().join("custom-images/movie/v1");
    assert_eq!(fs::read(stored).unwrap(), b"upload");
    assert_eq!(fs::read_dir(uploads).unwrap().count(), 0);
}

#[tokio::test]
async fn listing_skips_temporary_files_and_resized_copies() {
    let (data, disk) = disk();
    for key in [
        "artwork/tmdb/a",
        "artwork/tvdb/b",
        "artworks/c",
        "elsewhere/d",
    ] {
        disk.put(key, b"x".to_vec()).await.unwrap();
    }
    // As a write leaves it while it is under way, and as resizing keeps a copy.
    fs::write(data.path().join("artwork/tmdb/.tmpAbC123"), b"").unwrap();
    fs::create_dir_all(data.path().join("resized/artwork/tmdb/a")).unwrap();
    fs::write(data.path().join("resized/artwork/tmdb/a/640.jpg"), b"").unwrap();

    let mut listed = disk.list("artwork/").await.unwrap();
    listed.sort();

    assert_eq!(listed, ["artwork/tmdb/a", "artwork/tvdb/b"]);
    let mut by_start = disk.list("art").await.unwrap();
    by_start.sort();
    assert_eq!(by_start, ["artwork/tmdb/a", "artwork/tvdb/b", "artworks/c"]);
    assert_eq!(disk.list("resized/").await.unwrap(), Vec::<String>::new());
    assert_eq!(
        disk.list("nothing-yet/").await.unwrap(),
        Vec::<String>::new()
    );
}

#[tokio::test]
async fn listing_an_empty_prefix_is_refused() {
    let (_data, disk) = disk();
    disk.put("a/b", b"x".to_vec()).await.unwrap();

    let listed = disk.list("").await;

    assert_eq!(listed.unwrap_err().kind(), std::io::ErrorKind::InvalidInput);
}

#[tokio::test]
async fn only_a_stored_object_exists() {
    let (data, disk) = disk();
    disk.put("a/b", b"x".to_vec()).await.unwrap();
    fs::create_dir_all(data.path().join("a/folder")).unwrap();

    assert!(disk.exists("a/b").await.unwrap());
    assert!(!disk.exists("a/missing").await.unwrap());
    assert!(!disk.exists("a/folder").await.unwrap());
}

#[tokio::test]
async fn a_key_reaching_outside_the_store_is_refused() {
    let (_data, disk) = disk();

    for key in ["../a", "a/../b", "/a", "a//b", "a/.tmp"] {
        let put = disk.put(key, b"x".to_vec()).await;
        assert_eq!(
            put.unwrap_err().kind(),
            std::io::ErrorKind::InvalidInput,
            "{key}"
        );
    }
}

#[tokio::test]
async fn a_delete_removes_the_folders_it_leaves_empty_and_no_others() {
    let (data, disk) = disk();
    disk.put("custom-images/library/1/v1", b"x".to_vec())
        .await
        .unwrap();
    disk.put("custom-images/movie/2/v1", b"x".to_vec())
        .await
        .unwrap();

    disk.delete("custom-images/library/1/v1").await.unwrap();

    assert!(!data.path().join("custom-images/library").exists());
    assert!(data.path().join("custom-images/movie/2/v1").exists());
    disk.delete("custom-images/movie/2/v1").await.unwrap();
    assert!(!data.path().join("custom-images").exists());
    assert!(data.path().exists());
}
