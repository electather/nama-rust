use std::{
    io,
    path::Path,
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

use super::Cache;

#[tokio::test]
async fn a_subtitle_is_made_by_its_source_once_then_served_from_the_cache() {
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path());
    let key = Path::new("versions/7/2.vtt");

    let made = cache.get_or_make(key, async { Ok::<_, io::Error>(b"WEBVTT\n".to_vec()) });
    let made = made.await.unwrap();
    let again = cache.get_or_make(key, async { Err(io::Error::other("made again")) });
    let again = again.await.unwrap();

    assert_eq!(made, b"WEBVTT\n");
    assert_eq!(again, b"WEBVTT\n");
}

#[tokio::test]
async fn two_first_requests_at_once_share_one_making() {
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path());
    let key = Path::new("versions/7/2.vtt");
    let makings = AtomicUsize::new(0);
    let make = || async {
        makings.fetch_add(1, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(50)).await;
        Ok::<_, io::Error>(b"WEBVTT\n".to_vec())
    };

    let (first, second) = tokio::join!(
        cache.get_or_make(key, make()),
        cache.get_or_make(key, make())
    );

    assert_eq!(first.unwrap(), b"WEBVTT\n");
    assert_eq!(second.unwrap(), b"WEBVTT\n");
    assert_eq!(makings.load(Ordering::SeqCst), 1);
}
