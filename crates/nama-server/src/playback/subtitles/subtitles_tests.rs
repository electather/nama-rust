use std::{
    fs::{File, FileTimes},
    io,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, SystemTime},
};

use super::{Cache, cache_key, fonts_key};
use crate::{ffmpeg::TextSubtitle, library::Stored};

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

async fn put(cache: &Cache, key: &Path) {
    let made = cache.get_or_make(key, async { Ok::<_, io::Error>(b"cached".to_vec()) });
    made.await.unwrap();
}

#[tokio::test]
async fn a_burn_in_reads_from_the_cache_only_once_its_ass_and_every_font_are_there() {
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path());
    let file = Stored {
        id: 7,
        size: 1,
        modified_ns: 1,
        parser_version: 0,
    };
    let subtitle = TextSubtitle::Stream {
        subtitle: 0,
        track: 2,
        fonts: 2,
    };
    put(&cache, &cache_key(file, "2.ass")).await;
    put(&cache, &fonts_key(file).join("0")).await;

    let missing_a_font = cache.cached_burn_in(file, &subtitle);
    put(&cache, &fonts_key(file).join("1")).await;
    let cached = cache.cached_burn_in(file, &subtitle);

    assert_eq!(missing_a_font, None);
    let folder = dir.path().join("subtitles").join(cache_key(file, ""));
    let expected = TextSubtitle::Cached {
        ass: folder.join("2.ass").display().to_string(),
        fonts: folder.join("fonts").display().to_string(),
    };
    assert_eq!(cached, Some(expected));
}

/// Caches 6 bytes at `key`, written at the epoch and last served `secs` seconds after it, and
/// returns where.
async fn put_served_at(cache: &Cache, key: &str, secs: u64) -> PathBuf {
    put(cache, Path::new(key)).await;
    let path = cache.dir.join(key);
    let served = SystemTime::UNIX_EPOCH + Duration::from_secs(secs);
    let times = FileTimes::new()
        .set_modified(SystemTime::UNIX_EPOCH)
        .set_accessed(served);
    File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(times)
        .unwrap();
    path
}

/// A cache of three Versions: `a`, 12 bytes last served at 4, `b`, 6 at 2, and `c`, 6 at 3. Returns
/// the path of each one's file served last.
async fn three_versions(cache: &Cache) -> [PathBuf; 3] {
    put_served_at(cache, "versions/a/2.sup", 1).await;
    let a = put_served_at(cache, "versions/a/3.sup", 4).await;
    let b = put_served_at(cache, "versions/b/2.sup", 2).await;
    let c = put_served_at(cache, "versions/c/2.sup", 3).await;
    [a, b, c]
}

fn cached_versions(cache: &Cache) -> Vec<String> {
    let mut versions: Vec<String> = std::fs::read_dir(cache.dir.join("versions"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    versions.sort();
    versions
}

#[tokio::test]
async fn the_versions_served_least_recently_go_until_the_cache_fits_by_their_files_served_last() {
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path());
    let [a, ..] = three_versions(&cache).await;
    let before = cache.size().await.unwrap();

    cache.evict(13, &a).await.unwrap();

    assert_eq!(before, 24);
    assert_eq!(cached_versions(&cache), ["a"]);
    assert_eq!(cache.size().await.unwrap(), 12);
}

#[tokio::test]
async fn the_version_just_served_stays_even_when_it_was_served_least_recently() {
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path());
    let [_, b, _] = three_versions(&cache).await;

    cache.evict(13, &b).await.unwrap();

    assert_eq!(cached_versions(&cache), ["b"]);
}

#[tokio::test]
async fn a_version_being_made_stays() {
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path());
    let [a, b, _] = three_versions(&cache).await;
    // As a text pass over `b` holds it.
    let lock = Arc::clone(
        cache
            .making_locks()
            .entry(b.parent().unwrap().to_owned())
            .or_default(),
    );
    let making = lock.try_lock_owned().unwrap();

    cache.evict(13, &a).await.unwrap();
    drop(making);

    assert_eq!(cached_versions(&cache), ["a", "b"]);
}

#[tokio::test]
async fn a_version_with_a_subtitle_being_made_on_its_own_stays() {
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path());
    let [a, b, _] = three_versions(&cache).await;
    // As `get_or_make` holds it while ffmpeg reads one image Track out.
    let making = Arc::clone(cache.making_locks().entry(b).or_default());

    cache.evict(13, &a).await.unwrap();
    drop(making);

    assert_eq!(cached_versions(&cache), ["a", "b"]);
}

#[tokio::test]
async fn a_cache_that_fits_its_limit_keeps_everything() {
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path());
    let [a, ..] = three_versions(&cache).await;

    cache.evict(24, &a).await.unwrap();

    assert_eq!(cached_versions(&cache), ["a", "b", "c"]);
}

#[tokio::test]
async fn an_empty_cache_takes_no_bytes() {
    let dir = tempfile::tempdir().unwrap();

    assert_eq!(Cache::new(dir.path()).size().await.unwrap(), 0);
}

#[tokio::test]
async fn a_version_read_ahead_and_never_served_goes_before_any_served() {
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path());
    let [a, ..] = three_versions(&cache).await;
    // Written by a pass just now, so newer than any served.
    put(&cache, Path::new("versions/d/2.sup")).await;

    cache.evict(24, &a).await.unwrap();

    assert_eq!(cached_versions(&cache), ["a", "b", "c"]);
}

#[tokio::test]
async fn a_version_whose_pass_failed_stays_so_its_file_isnt_read_again() {
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path());
    let [a, ..] = three_versions(&cache).await;
    put_served_at(
        &cache,
        &format!("versions/b/{}", super::text_pass::FAILED),
        2,
    )
    .await;

    cache.evict(13, &a).await.unwrap();

    assert_eq!(cached_versions(&cache), ["a", "b"]);
}
