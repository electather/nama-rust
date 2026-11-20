use std::{
    io,
    path::Path,
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
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
