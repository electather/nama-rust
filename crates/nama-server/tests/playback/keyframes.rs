//! Remuxing with a keyframe scan by a scripted ffprobe: an MKV's Cues spare it, one that fails
//! falls back to a Transcode, and a Version's keyframes are listed once.

use super::*;

/// Starts a Server with `ffmpeg` and Alien as [`Fixture::alien_mkv`] writes it, but copied through
/// a pipe, which leaves its Cues out, so its first Remux lists its keyframes by reading it all.
#[cfg(unix)]
async fn alien_mkv_without_cues(ffmpeg: Ffmpeg) -> (Fixture, i64) {
    Fixture::alien_written(ffmpeg, |file| {
        let cued = file.with_extension("cued.mkv");
        ffmpeg::write_h264_with_5_1(&cued, "aac")?;
        let copied = std::process::Command::new("ffmpeg")
            .args(["-v", "error", "-i"])
            .arg(&cued)
            .args(["-c", "copy", "-f", "matroska", "pipe:1"])
            .stdout(fs::File::create(file)?)
            .status()?;
        assert!(copied.success());
        fs::remove_file(cued)
    })
    .await
}

/// A Server whose ffprobe, a shell script in `bin`, runs `first` and then the ffprobe on `PATH`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
#[cfg(unix)]
fn ffprobe_after(bin: &Path, first: &str) -> Ffmpeg {
    let ffmpeg = system_ffmpeg();
    let real = ffmpeg.ffprobe.unwrap().path;
    let ffprobe = format!("#!/bin/sh\n{first}\nexec '{}' \"$@\"\n", real.display());
    Ffmpeg {
        ffprobe: Ok(script(bin, "ffprobe", &ffprobe)),
        ..ffmpeg
    }
}

#[cfg(unix)]
fn ffprobe_logging_its_runs(bin: &Path, log: &Path) -> Ffmpeg {
    ffprobe_after(bin, &format!("echo \"$*\" >> '{}'", log.display()))
}

/// An ffprobe that fails to list keyframes, logging each listing to `bin/runs.log`.
#[cfg(unix)]
fn ffprobe_failing_keyframes(bin: &Path) -> Ffmpeg {
    let fail = format!(
        "case \"$*\" in *packet=pts_time,flags*) echo list >> '{}'; echo 'read failed' >&2; \
         exit 1;; esac",
        bin.join("runs.log").display()
    );
    ffprobe_after(bin, &fail)
}

// The failing ffprobe is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn a_remux_whose_keyframes_cant_be_listed_transcodes_and_says_why() {
    let bin = tempfile::tempdir().unwrap();
    let (fx, version) = alien_mkv_without_cues(ffprobe_failing_keyframes(bin.path())).await;

    let plan = fx.transcode(version).await;

    reason_of(
        &plan,
        PlaybackReasonKind::PLAYBACK_REASON_KIND_KEYFRAMES_UNLISTED,
    );
    fx.fetch_ok(&plan.url).await;
}

// The failing ffprobe is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn a_remux_whose_keyframes_cant_be_listed_is_refused_as_busy_at_the_transcode_limit() {
    let bin = tempfile::tempdir().unwrap();
    let (fx, version) = alien_mkv_without_cues(ffprobe_failing_keyframes(bin.path())).await;
    fx.allow_one_transcode().await;
    fx.transcode(version).await;

    let planned = fx.plan(version).await;

    assert_refused(
        planned,
        ErrorCode::ResourceExhausted,
        RefusalReason::REFUSAL_REASON_SERVER_BUSY,
    );
}

// The failing ffprobe is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn a_version_whose_keyframes_couldnt_be_listed_transcodes_next_time_without_listing_them() {
    let bin = tempfile::tempdir().unwrap();
    let (fx, version) = alien_mkv_without_cues(ffprobe_failing_keyframes(bin.path())).await;
    fx.transcode(version).await;

    let plan = fx.transcode(version).await;

    reason_of(
        &plan,
        PlaybackReasonKind::PLAYBACK_REASON_KIND_KEYFRAMES_UNLISTED,
    );
    let runs = fs::read_to_string(bin.path().join("runs.log")).unwrap();
    assert_eq!(runs.lines().count(), 1, "{runs}");
}

// The logging ffprobe is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn remuxing_a_version_again_reuses_the_keyframes_listed_the_first_time() {
    let bin = tempfile::tempdir().unwrap();
    let log = bin.path().join("runs.log");
    let (fx, version) = alien_mkv_without_cues(ffprobe_logging_its_runs(bin.path(), &log)).await;

    let first = fx.plan(version).await.unwrap();
    let second = fx.plan(version).await.unwrap();

    assert_ne!(first.url, second.url, "each plan runs on its own");
    assert_eq!(
        fx.fetch_ok(&first.url).await,
        fx.fetch_ok(&second.url).await
    );
    let runs = fs::read_to_string(&log).unwrap();
    let listings = runs.lines().filter(|run| run.contains("packet=pts_time"));
    assert_eq!(listings.count(), 1, "{runs}");
}

// The logging ffprobe is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn an_mkv_with_cues_remuxes_on_its_first_play_without_reading_it_all() {
    let bin = tempfile::tempdir().unwrap();
    let log = bin.path().join("runs.log");
    let (fx, version) = Fixture::alien_mkv(ffprobe_logging_its_runs(bin.path(), &log), "aac").await;

    let plan = fx.plan(version).await.unwrap();

    assert_eq!(plan.method, PlaybackMethod::PLAYBACK_METHOD_REMUX);
    let playlist = String::from_utf8(fx.fetch_ok(&plan.url).await).unwrap();
    for file in segments(&playlist).1 {
        fx.fetch_ok(&beside(&plan.url, file)).await;
    }
    let runs = fs::read_to_string(&log).unwrap();
    assert!(!runs.contains("packet="), "{runs}");
}
