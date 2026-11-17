use std::{
    fs,
    path::Path,
    sync::Arc,
    time::{Duration, UNIX_EPOCH},
};

use super::tests::{Fixture, summary};
use crate::{
    library::LibraryKind,
    playback::{Probe, Sidecar, Sidecars, Track, Tracks},
};

const VIDEO: &str = "Alien (1979)/Alien (1979).mkv";
const ENGLISH: &str = "Alien (1979)/Alien (1979).en.srt";
const FRENCH: &str = "Alien (1979)/Alien (1979).fr.forced.ass";

/// The Version id of the stored file at `relative`.
async fn version(media: &Fixture, relative: &str) -> i64 {
    let stored = media.contents.stored(media.library).await.unwrap();
    stored[media.path(relative).to_str().unwrap()].id
}

/// The sidecars stored beside the Version `version`, with what their names say.
async fn sidecars(media: &Fixture, version: i64) -> Vec<(Sidecar, Track)> {
    Sidecars::new(Arc::clone(&media.db)).of(version).await
}

/// The sizes of the sidecars stored beside `VIDEO`, by `/`-separated path relative to the Library
/// folder, whatever separator the platform uses.
async fn stored(media: &Fixture) -> Vec<(String, i64)> {
    let version = version(media, VIDEO).await;
    (sidecars(media, version).await.into_iter())
        .map(|(sidecar, _)| {
            let path = Path::new(&sidecar.path).strip_prefix(&media.root).unwrap();
            let parts: Vec<&str> = (path.components())
                .map(|part| part.as_os_str().to_str().unwrap())
                .collect();
            (parts.join("/"), sidecar.size)
        })
        .collect()
}

/// What probing found in the stored file at `relative`, if it has been probed.
async fn probe(media: &Fixture, relative: &str) -> Option<Probe> {
    let stored = media.contents.stored(media.library).await.unwrap();
    let file = stored[media.path(relative).to_str().unwrap()];
    let tracks = Tracks::new(Arc::clone(&media.db));
    tracks.of(&[file]).await.unwrap().remove(&file.id)
}

#[tokio::test]
async fn a_scan_stores_the_sidecars_beside_a_version_with_what_their_names_say() {
    let media = Fixture::new(LibraryKind::Movies).await;
    media.write(VIDEO, 10);
    media.write(ENGLISH, 3);
    media.write(FRENCH, 4);

    let scanned = media.scan().await;

    assert_eq!(scanned, summary(1, 0, 0, 0));
    let files: Vec<String> = media
        .files()
        .await
        .into_iter()
        .map(|(path, _)| path)
        .collect();
    assert_eq!(files, [VIDEO]);
    let version = version(&media, VIDEO).await;
    let tracks: Vec<_> = (sidecars(&media, version).await.into_iter())
        .map(|(_, track)| (track.codec, track.language, track.forced))
        .collect();
    assert_eq!(
        tracks,
        [
            ("subrip".to_owned(), Some("en".to_owned()), false),
            ("ass".to_owned(), Some("fr".to_owned()), true),
        ]
    );
}

// The fake ffprobe is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn a_scan_follows_sidecars_added_edited_and_removed_without_probing_the_video_again() {
    let bin = tempfile::tempdir().unwrap();
    let ffprobe = r#"echo '{"format": {"format_name": "matroska,webm"}, "streams": [{"codec_type": "audio", "codec_name": "ac3", "channels": 2}]}'"#;
    let media = Fixture::with_ffmpeg(
        LibraryKind::Movies,
        super::tests::fake_ffprobe(bin.path(), ffprobe),
    )
    .await;
    let scan = async || {
        let scanned = media.scan().await;
        media.probing.as_ref().unwrap().idle().await;
        scanned
    };
    let runs = || {
        fs::read_to_string(bin.path().join("runs"))
            .unwrap()
            .lines()
            .count()
    };
    media.write(VIDEO, 10);
    media.write(ENGLISH, 3);
    scan().await;
    let probed = probe(&media, VIDEO).await;
    assert!(probed.is_some(), "the video was never probed");
    let ffprobe_runs = runs();

    media.write(FRENCH, 4);
    assert_eq!(scan().await, summary(0, 0, 0, 1));
    assert_eq!(
        stored(&media).await,
        [(ENGLISH.to_owned(), 3), (FRENCH.to_owned(), 4)]
    );

    media.write(ENGLISH, 5);
    let edited = UNIX_EPOCH + Duration::from_secs(1000);
    fs::File::options()
        .write(true)
        .open(media.path(ENGLISH))
        .unwrap()
        .set_modified(edited)
        .unwrap();
    scan().await;
    assert_eq!(
        stored(&media).await,
        [(ENGLISH.to_owned(), 5), (FRENCH.to_owned(), 4)]
    );
    let version = version(&media, VIDEO).await;
    let modified: Vec<i64> = (sidecars(&media, version).await.into_iter())
        .map(|(sidecar, _)| sidecar.modified_ns)
        .collect();
    assert_eq!(modified[0], 1_000_000_000_000);

    fs::remove_file(media.path(FRENCH)).unwrap();
    scan().await;
    assert_eq!(stored(&media).await, [(ENGLISH.to_owned(), 5)]);

    assert_eq!(runs(), ffprobe_runs, "the video was probed again");
    assert_eq!(probe(&media, VIDEO).await, probed);
}

#[tokio::test]
async fn removing_a_video_removes_its_sidecars() {
    let media = Fixture::new(LibraryKind::Movies).await;
    media.write(VIDEO, 10);
    media.write(ENGLISH, 3);
    media.scan().await;
    let version = version(&media, VIDEO).await;
    assert_eq!(sidecars(&media, version).await.len(), 1);

    fs::remove_file(media.path(VIDEO)).unwrap();
    media.scan().await;

    assert_eq!(sidecars(&media, version).await, []);
}

#[tokio::test]
async fn sidecars_in_a_folder_that_cant_be_read_are_kept() {
    let media = Fixture::new(LibraryKind::Movies).await;
    media.write(VIDEO, 10);
    media.write(ENGLISH, 3);
    media.scan().await;

    // The Library folder is gone, as when a drive isn't mounted.
    let parked = media.root.with_file_name("parked");
    fs::rename(&media.root, &parked).unwrap();
    media.scan().await;

    assert_eq!(stored(&media).await, [(ENGLISH.to_owned(), 3)]);
}
