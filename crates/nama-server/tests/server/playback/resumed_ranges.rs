//! A `<video>` resuming a Version asks for the rest of the file with `If-Range`, so it gets the
//! whole file again, not the new file's bytes spliced in, once the file was replaced.

use std::time::{SystemTime, UNIX_EPOCH};

use super::*;

/// Replaces the file at `path` with `bytes`, last modified at `modified`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn rewrite(path: &Path, bytes: &[u8], modified: SystemTime) {
    fs::write(path, bytes).unwrap();
    fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(modified)
        .unwrap();
}

/// A time long past, `seconds` after the epoch.
fn at(seconds: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(seconds)
}

/// The Fixture with Alien, and where its Version is served from.
#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "a test helper fails its test by panicking"
)]
async fn alien() -> (Fixture, String) {
    let fx = Fixture::alien().await;
    let movie = fx.get_movie(fx.library, ALIEN).await.unwrap();
    let url = version_url(&movie.versions[0]);
    (fx, url)
}

#[tokio::test]
async fn a_versions_etag_changes_with_its_files_size_or_mtime() {
    let (fx, url) = alien().await;
    let file = fx.folder.join(ALIEN_1080P);
    let token = Some(fx.server.admin_token.as_str());
    let etag = async || {
        fetch(&fx.server, &url, token)
            .await
            .header("etag")
            .to_owned()
    };
    rewrite(&file, b"video", at(1_000_000));
    let first = etag().await;

    rewrite(&file, b"video", at(2_000_000));
    let touched = etag().await;
    rewrite(&file, b"videos", at(1_000_000));
    let grown = etag().await;

    assert!(first.starts_with('"'), "not a strong ETag: {first}");
    assert_ne!(touched, first);
    assert_ne!(grown, first);
}

#[tokio::test]
async fn a_range_whose_if_range_is_the_current_etag_or_last_modified_is_served_as_those_bytes() {
    let (fx, url) = alien().await;
    let token = Some(fx.server.admin_token.as_str());
    let whole = fetch(&fx.server, &url, token).await;

    for validator in [whole.header("etag"), whole.header("last-modified")] {
        let headers = [("range", "bytes=1-3"), ("if-range", validator)];
        let reply = fetch_with(&fx.server, &url, token, &headers).await;

        assert_eq!(reply.status, 206, "If-Range: {validator}");
        assert_eq!(reply.body, b"ide");
        assert_eq!(reply.header("content-range"), "bytes 1-3/5");
    }
}

/// The new file may be shorter than where the old one was resumed from.
#[tokio::test]
async fn a_range_whose_if_range_names_a_replaced_file_is_served_the_whole_new_file() {
    let (fx, url) = alien().await;
    let file = fx.folder.join(ALIEN_1080P);
    let token = Some(fx.server.admin_token.as_str());
    rewrite(&file, b"video", at(1_000_000));
    let old = fetch(&fx.server, &url, token).await;
    rewrite(&file, b"new", at(2_000_000));

    for validator in [old.header("etag"), old.header("last-modified")] {
        for range in ["bytes=1-2", "bytes=4-"] {
            let headers = [("range", range), ("if-range", validator)];
            let reply = fetch_with(&fx.server, &url, token, &headers).await;

            assert_eq!(reply.status, 200, "Range: {range}, If-Range: {validator}");
            assert_eq!(reply.body, b"new");
            assert_eq!(reply.header("content-range"), "");
        }
    }
}

#[tokio::test]
async fn a_resumed_range_of_a_file_gone_since_the_scan_is_not_found() {
    let (fx, url) = alien().await;
    let token = Some(fx.server.admin_token.as_str());
    let old = fetch(&fx.server, &url, token).await;
    fs::remove_file(fx.folder.join(ALIEN_1080P)).unwrap();

    let headers = [("range", "bytes=1-3"), ("if-range", old.header("etag"))];
    let reply = fetch_with(&fx.server, &url, token, &headers).await;

    assert_eq!(reply.status, 404);
}
