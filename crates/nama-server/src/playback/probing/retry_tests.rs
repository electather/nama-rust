use std::{fs, io, sync::Arc, time::Duration};

use super::{
    ProbeError,
    testing::{FakeReader, by_size, failed, one_audio_track},
    tests::{Fixture, chapters_read},
};
use crate::{
    ffmpeg::{Program, RunError},
    playback::tracks::{PROBE_ATTEMPTS, PROBE_VERSION, PassingReason, Probe, ProbeFailure},
};

/// What ffprobe that couldn't be started answers, as when it's gone during a package upgrade.
fn couldnt_run() -> ProbeError {
    ProbeError::Run(RunError::Run(
        Program::Ffprobe,
        io::ErrorKind::NotFound.into(),
    ))
}

/// The file at `path` as [`by_size`] reads it if it's 10 bytes or fewer, else ffprobe failing.
fn failing_over_10_bytes(path: &str) -> Result<Probe, ProbeError> {
    if fs::metadata(path).unwrap().len() <= 10 {
        Ok(by_size(path))
    } else {
        Err(failed())
    }
}

#[tokio::test]
async fn an_unchanged_file_is_probed_again_only_once_the_probe_version_is_bumped() {
    let reader = FakeReader::by_size();
    let media = Fixture::with_reader(Arc::clone(&reader)).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write(alien, 10).await;
    media.probe_changed().await;
    media.probe_changed().await;
    assert_eq!(reader.probes(), 1, "an unchanged file was probed again");

    let version = media.file_id(alien).await;
    media.tracks.read_by_older_probe(version).await;
    media.probe_changed().await;

    assert_eq!(reader.probes(), 2);
    assert_eq!(
        media.tracks.probe_version_of(version).await,
        Some(PROBE_VERSION)
    );
    media.probe_changed().await;
    assert_eq!(reader.probes(), 2, "probed again at the current version");
}

#[tokio::test]
async fn a_version_read_by_the_probe_before_chapters_gets_them_when_probed_again() {
    let reader = FakeReader::answering(|_| {
        Ok(Probe {
            chapters: chapters_read(),
            ..one_audio_track(2)
        })
    });
    let media = Fixture::with_reader(reader).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write(alien, 10).await;
    let file = media.stored(alien).await;
    let without_chapters = Probe {
        container: "matroska,webm".to_owned(),
        duration_ms: Some(3_600_000),
        bitrate: None,
        tracks: vec![],
        font_attachments: vec![],
        chapters: vec![],
    };
    media.tracks.store(file, without_chapters).await.unwrap();
    media.tracks.read_by_older_probe(file.id).await;

    media.probe_changed().await;

    let stored = media.probe(alien).await.map(|probe| probe.chapters);
    assert_eq!(stored, Some(chapters_read()));
}

#[tokio::test]
async fn a_file_replaced_by_another_is_probed_again_and_its_tracks_replaced() {
    let reader = FakeReader::by_size();
    let media = Fixture::with_reader(Arc::clone(&reader)).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write(alien, 10).await;
    media.probe_changed().await;
    assert_eq!(media.channels(alien).await, Some(10));

    media.write(alien, 20).await;
    media.probe_changed().await;

    assert_eq!(reader.probes(), 2);
    assert_eq!(media.channels(alien).await, Some(20));
}

#[tokio::test]
async fn a_file_touched_without_changing_size_is_probed_again() {
    let reader = FakeReader::by_size();
    let media = Fixture::with_reader(Arc::clone(&reader)).await;
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

    assert_eq!(reader.probes(), 2);
    assert_eq!(media.channels(alien).await, Some(10));
}

#[tokio::test]
async fn a_file_replaced_by_one_that_cant_be_probed_loses_its_old_tracks() {
    let media = Fixture::with_reader(FakeReader::answering(failing_over_10_bytes)).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write(alien, 10).await;
    media.probe_changed().await;
    assert_eq!(media.channels(alien).await, Some(10));

    media.write(alien, 20).await;
    media.probe_changed().await;

    let version = media.file_id(alien).await;
    assert_eq!(media.tracks.probe_version_of(version).await, None);
}

#[tokio::test]
async fn a_file_that_cant_be_probed_is_probed_again_only_once_it_changes() {
    let reader = FakeReader::answering(failing_over_10_bytes);
    let media = Fixture::with_reader(Arc::clone(&reader)).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write(alien, 20).await;
    media.probe_changed().await;
    media.probe_changed().await;
    assert_eq!(reader.probes(), 1, "an unreadable file was probed again");

    media.write(alien, 10).await;
    media.probe_changed().await;

    assert_eq!(reader.probes(), 2);
    assert_eq!(media.channels(alien).await, Some(10));
}

#[tokio::test]
async fn a_file_that_couldnt_be_probed_is_probed_again_once_the_probe_version_is_bumped() {
    let reader = FakeReader::answering(|_| Err(failed()));
    let media = Fixture::with_reader(Arc::clone(&reader)).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write(alien, 10).await;
    media.probe_changed().await;

    media
        .tracks
        .failed_by_older_probe(media.file_id(alien).await)
        .await;
    media.probe_changed().await;

    assert_eq!(reader.probes(), 2);
}

#[tokio::test]
async fn a_file_that_couldnt_be_probed_is_probed_again_once_the_failure_version_is_bumped() {
    let reader = FakeReader::answering(|_| Err(failed()));
    let media = Fixture::with_reader(Arc::clone(&reader)).await;
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

    assert_eq!(reader.probes(), 2);
}

#[tokio::test]
async fn a_file_ffprobe_couldnt_be_run_on_is_probed_again_next_pass() {
    let reader = FakeReader::answering(|_| Err(couldnt_run()));
    let media = Fixture::with_reader(Arc::clone(&reader)).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write(alien, 10).await;
    media.probe_changed().await;
    let version = media.file_id(alien).await;
    assert_eq!(media.tracks.failure_reason_of(version).await, None);

    reader.answer(|path| Ok(by_size(path)));
    media.probe_changed().await;

    assert_eq!(media.channels(alien).await, Some(10));
}

#[tokio::test]
async fn a_file_is_probed_once_ffprobe_runs_again_however_many_passes_it_couldnt() {
    let reader = FakeReader::answering(|_| Err(couldnt_run()));
    let media = Fixture::with_reader(Arc::clone(&reader)).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write(alien, 10).await;
    for _ in 0..=PROBE_ATTEMPTS {
        media.probe_changed().await;
    }

    reader.answer(|path| Ok(by_size(path)));
    media.probe_changed().await;

    assert_eq!(media.channels(alien).await, Some(10));
}

const TIMED_OUT: ProbeFailure = ProbeFailure::Passing(PassingReason::TimedOut);

#[tokio::test]
async fn a_file_that_timed_out_is_probed_again_next_pass() {
    let media = Fixture::with_reader(FakeReader::by_size()).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write(alien, 10).await;
    let file = media.stored(alien).await;
    for _ in 1..PROBE_ATTEMPTS {
        media.tracks.store_failure(file, TIMED_OUT).await.unwrap();
    }

    media.probe_changed().await;

    assert_eq!(media.channels(alien).await, Some(10));
}

#[tokio::test]
async fn a_file_that_keeps_timing_out_is_given_up_on() {
    let reader = FakeReader::by_size();
    let media = Fixture::with_reader(Arc::clone(&reader)).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write(alien, 10).await;
    let file = media.stored(alien).await;
    for _ in 0..PROBE_ATTEMPTS {
        media.tracks.store_failure(file, TIMED_OUT).await.unwrap();
    }

    media.probe_changed().await;
    assert_eq!(reader.probes(), 0, "probed after too many attempts");

    media.tracks.failed_by_older_failure_version(file.id).await;
    media.probe_changed().await;
    assert_eq!(media.channels(alien).await, Some(10));
}

#[tokio::test]
async fn a_file_gone_by_the_time_its_probed_is_probed_again_once_back() {
    // Moves the file away as a dropped share would, and fails on it.
    let reader = FakeReader::answering(|path| {
        fs::rename(path, format!("{path}.away")).unwrap();
        Err(failed())
    });
    let media = Fixture::with_reader(Arc::clone(&reader)).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    media.write(alien, 10).await;
    media.probe_changed().await;
    let version = media.file_id(alien).await;
    assert_eq!(media.tracks.failure_reason_of(version).await, None);

    reader.answer(|path| Ok(by_size(path)));
    // A rename keeps its size and modified time, as a share coming back would.
    let path = media.path(alien);
    fs::rename(path.with_extension("mkv.away"), &path).unwrap();
    media.probe_changed().await;

    assert_eq!(media.channels(alien).await, Some(10));
}
