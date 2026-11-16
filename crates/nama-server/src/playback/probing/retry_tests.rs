use std::{fs, time::Duration};

use super::tests::Fixture;
#[cfg(unix)]
use super::tests::{counting_ffprobe, fake_ffprobe, probe_runs};
use crate::playback::tracks::{PROBE_ATTEMPTS, PROBE_VERSION, PassingReason, ProbeFailure};

// The fake ffprobe is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn an_unchanged_file_is_probed_again_only_once_the_probe_version_is_bumped() {
    let bin = tempfile::tempdir().unwrap();
    let media = Fixture::with_ffmpeg(counting_ffprobe(bin.path())).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write(alien, 10).await;
    media.probe_changed().await;
    media.probe_changed().await;
    assert_eq!(
        probe_runs(bin.path()),
        1,
        "an unchanged file was probed again"
    );

    let version = media.file_id(alien).await;
    media.tracks.read_by_older_probe(version).await;
    media.probe_changed().await;

    assert_eq!(probe_runs(bin.path()), 2);
    assert_eq!(
        media.tracks.probe_version_of(version).await,
        Some(PROBE_VERSION)
    );
    media.probe_changed().await;
    assert_eq!(
        probe_runs(bin.path()),
        2,
        "probed again at the current version"
    );
}

// The fake ffprobe is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn a_file_replaced_by_another_is_probed_again_and_its_tracks_replaced() {
    let bin = tempfile::tempdir().unwrap();
    let media = Fixture::with_ffmpeg(counting_ffprobe(bin.path())).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write(alien, 10).await;
    media.probe_changed().await;
    assert_eq!(media.channels(alien).await, Some(10));

    media.write(alien, 20).await;
    media.probe_changed().await;

    assert_eq!(probe_runs(bin.path()), 2);
    assert_eq!(media.channels(alien).await, Some(20));
}

// The fake ffprobe is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn a_file_touched_without_changing_size_is_probed_again() {
    let bin = tempfile::tempdir().unwrap();
    let media = Fixture::with_ffmpeg(counting_ffprobe(bin.path())).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write(alien, 10).await;
    media.probe_changed().await;

    let touched = std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
    fs::File::options()
        .write(true)
        .open(media.path(alien))
        .unwrap()
        .set_modified(touched)
        .unwrap();
    media.store(alien).await;
    media.probe_changed().await;

    assert_eq!(probe_runs(bin.path()), 2);
    assert_eq!(media.channels(alien).await, Some(10));
}

// The fake ffprobe is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn a_file_replaced_by_one_that_cant_be_probed_loses_its_old_tracks() {
    let bin = tempfile::tempdir().unwrap();
    // Reads a file of up to 10 bytes as one audio Track, and fails on a bigger one.
    let ffmpeg = fake_ffprobe(
        bin.path(),
        r#"for file; do :; done
[ "$(wc -c < "$file")" -le 10 ] || exit 1
echo '{"format": {"format_name": "matroska,webm", "duration": "1.000000"}, "streams": [{"codec_type": "audio", "codec_name": "ac3", "channels": 2}]}'"#,
    );
    let media = Fixture::with_ffmpeg(ffmpeg).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write(alien, 10).await;
    media.probe_changed().await;
    assert_eq!(media.channels(alien).await, Some(2));

    media.write(alien, 20).await;
    media.probe_changed().await;

    let version = media.file_id(alien).await;
    assert_eq!(media.tracks.probe_version_of(version).await, None);
}

// The fake ffprobe is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn a_file_that_cant_be_probed_is_probed_again_only_once_it_changes() {
    let bin = tempfile::tempdir().unwrap();
    let runs = bin.path().join("runs");
    // Logs each run, and reads a file of up to 10 bytes as one audio Track, failing on a
    // bigger one.
    let ffmpeg = fake_ffprobe(
        bin.path(),
        &format!(
            r#"echo run >> '{}'
for file; do :; done
[ "$(wc -c < "$file")" -le 10 ] || exit 1
echo '{{"format": {{"format_name": "matroska,webm", "duration": "1.000000"}}, "streams": [{{"codec_type": "audio", "codec_name": "ac3", "channels": 2}}]}}'"#,
            runs.display()
        ),
    );
    let media = Fixture::with_ffmpeg(ffmpeg).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write(alien, 20).await;
    media.probe_changed().await;
    media.probe_changed().await;
    assert_eq!(
        probe_runs(bin.path()),
        1,
        "an unreadable file was probed again"
    );

    media.write(alien, 10).await;
    media.probe_changed().await;

    assert_eq!(probe_runs(bin.path()), 2);
    assert_eq!(media.channels(alien).await, Some(2));
}

// The fake ffprobe is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn a_file_that_couldnt_be_probed_is_probed_again_once_the_probe_version_is_bumped() {
    let bin = tempfile::tempdir().unwrap();
    let runs = bin.path().join("runs");
    let ffmpeg = fake_ffprobe(
        bin.path(),
        &format!("echo run >> '{}'\nexit 1", runs.display()),
    );
    let media = Fixture::with_ffmpeg(ffmpeg).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write(alien, 10).await;
    media.probe_changed().await;

    media
        .tracks
        .failed_by_older_probe(media.file_id(alien).await)
        .await;
    media.probe_changed().await;

    assert_eq!(probe_runs(bin.path()), 2);
}

// The fake ffprobe is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn a_file_that_couldnt_be_probed_is_probed_again_once_the_failure_version_is_bumped() {
    let bin = tempfile::tempdir().unwrap();
    let runs = bin.path().join("runs");
    let ffmpeg = fake_ffprobe(
        bin.path(),
        &format!("echo run >> '{}'\nexit 1", runs.display()),
    );
    let media = Fixture::with_ffmpeg(ffmpeg).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write(alien, 10).await;
    media.probe_changed().await;
    let version = media.file_id(alien).await;
    assert_eq!(
        media.tracks.failure_reason_of(version).await.as_deref(),
        Some("failed")
    );

    media.tracks.failed_by_older_failure_version(version).await;
    media.probe_changed().await;

    assert_eq!(probe_runs(bin.path()), 2);
}

// The fake ffprobe is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn a_file_ffprobe_couldnt_be_run_on_is_probed_again_next_pass() {
    let bin = tempfile::tempdir().unwrap();
    let media = Fixture::with_ffmpeg(counting_ffprobe(bin.path())).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write(alien, 10).await;
    fs::remove_file(bin.path().join("ffprobe")).unwrap();
    media.probe_changed().await;
    let version = media.file_id(alien).await;
    assert_eq!(media.tracks.failure_reason_of(version).await, None);

    counting_ffprobe(bin.path());
    media.probe_changed().await;

    assert_eq!(media.channels(alien).await, Some(10));
}

// The fake ffprobe is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn a_file_is_probed_once_ffprobe_runs_again_however_many_passes_it_couldnt() {
    let bin = tempfile::tempdir().unwrap();
    let media = Fixture::with_ffmpeg(counting_ffprobe(bin.path())).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write(alien, 10).await;
    fs::remove_file(bin.path().join("ffprobe")).unwrap();
    for _ in 0..=PROBE_ATTEMPTS {
        media.probe_changed().await;
    }

    counting_ffprobe(bin.path());
    media.probe_changed().await;

    assert_eq!(media.channels(alien).await, Some(10));
}

const TIMED_OUT: ProbeFailure = ProbeFailure::Passing(PassingReason::TimedOut);

// The fake ffprobe is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn a_file_that_timed_out_is_probed_again_next_pass() {
    let bin = tempfile::tempdir().unwrap();
    let media = Fixture::with_ffmpeg(counting_ffprobe(bin.path())).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write(alien, 10).await;
    let file = media.stored(alien).await;
    for _ in 1..PROBE_ATTEMPTS {
        media.tracks.store_failure(file, TIMED_OUT).await.unwrap();
    }

    media.probe_changed().await;

    assert_eq!(media.channels(alien).await, Some(10));
}

// The fake ffprobe is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn a_file_that_keeps_timing_out_is_given_up_on() {
    let bin = tempfile::tempdir().unwrap();
    let media = Fixture::with_ffmpeg(counting_ffprobe(bin.path())).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write(alien, 10).await;
    let file = media.stored(alien).await;
    for _ in 0..PROBE_ATTEMPTS {
        media.tracks.store_failure(file, TIMED_OUT).await.unwrap();
    }

    media.probe_changed().await;
    assert_eq!(probe_runs(bin.path()), 0, "probed after too many attempts");

    media.tracks.failed_by_older_failure_version(file.id).await;
    media.probe_changed().await;
    assert_eq!(media.channels(alien).await, Some(10));
}

// The fake ffprobe is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn a_file_gone_by_the_time_its_probed_is_probed_again_once_back() {
    let bin = tempfile::tempdir().unwrap();
    let gone = bin.path().join("gone");
    // The first time, moves the file away as a dropped share would, and fails on it.
    let ffmpeg = fake_ffprobe(
        bin.path(),
        &format!(
            r#"for file; do :; done
if [ ! -e '{gone}' ]; then touch '{gone}'; mv "$file" "$file.away"; exit 1; fi
echo "{{\"format\": {{\"format_name\": \"matroska,webm\", \"duration\": \"1.000000\"}}, \"streams\": [{{\"codec_type\": \"audio\", \"codec_name\": \"ac3\", \"channels\": $(wc -c < "$file")}}]}}""#,
            gone = gone.display()
        ),
    );
    let media = Fixture::with_ffmpeg(ffmpeg).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write(alien, 10).await;
    media.probe_changed().await;
    let version = media.file_id(alien).await;
    assert_eq!(media.tracks.failure_reason_of(version).await, None);

    // A rename keeps its size and modified time, as a share coming back would.
    let path = media.path(alien);
    fs::rename(path.with_extension("mkv.away"), &path).unwrap();
    media.probe_changed().await;

    assert_eq!(media.channels(alien).await, Some(10));
}
