//! Remuxing with a keyframe scan by a scripted ffprobe: an MKV's Cues and an MP4's sample tables
//! spare it, one that fails falls back to a Transcode, and a Version's keyframes are listed once.

use super::*;

/// Starts a Server with `ffmpeg` and Alien as [`Fixture::alien_mkv`] writes it, but copied through
/// a pipe, which leaves its Cues out, so its first Remux lists its keyframes by reading it all.
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
fn ffprobe_after(bin: &Path, first: &str) -> Ffmpeg {
    let ffmpeg = system_ffmpeg();
    let real = ffmpeg.ffprobe.unwrap().path;
    let ffprobe = format!("#!/bin/sh\n{first}\nexec '{}' \"$@\"\n", real.display());
    Ffmpeg {
        ffprobe: Ok(script(bin, "ffprobe", &ffprobe)),
        ..ffmpeg
    }
}

fn ffprobe_logging_its_runs(bin: &Path, log: &Path) -> Ffmpeg {
    ffprobe_after(bin, &format!("echo \"$*\" >> '{}'", log.display()))
}

/// An ffprobe that fails to list keyframes, logging each listing to `bin/runs.log`.
fn ffprobe_failing_keyframes(bin: &Path) -> Ffmpeg {
    let fail = format!(
        "case \"$*\" in *packet=pts_time,flags*) echo list >> '{}'; echo 'read failed' >&2; \
         exit 1;; esac",
        bin.join("runs.log").display()
    );
    ffprobe_after(bin, &fail)
}

// The failing ffprobe is a shell script.
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

// The hanging ffprobe is a shell script.
#[tokio::test]
async fn a_first_play_whose_listing_hangs_transcodes_after_the_wait_and_the_next_play_remuxes() {
    let bin = tempfile::tempdir().unwrap();
    let (log, go) = (bin.path().join("runs.log"), bin.path().join("go"));
    let hang = format!(
        "case \"$*\" in *packet=pts_time,flags*) echo list >> '{}'; \
         while [ ! -e '{}' ]; do sleep 0.1; done;; esac",
        log.display(),
        go.display()
    );
    let (fx, version) = alien_mkv_without_cues(ffprobe_after(bin.path(), &hang)).await;

    let started = std::time::Instant::now();
    let first = fx.transcode(version).await;
    let waited = started.elapsed();
    fs::write(&go, "").unwrap();
    let second = fx.plan(version).await.unwrap();

    reason_of(
        &first,
        PlaybackReasonKind::PLAYBACK_REASON_KIND_KEYFRAMES_UNLISTED,
    );
    // At least Probing's `LISTING_WAIT`, and well short of the 30 s it waited before #1214.
    let about_the_wait = Duration::from_secs(5)..Duration::from_secs(15);
    assert!(about_the_wait.contains(&waited), "{waited:?}");
    assert_eq!(second.method, PlaybackMethod::PLAYBACK_METHOD_REMUX);
    let runs = fs::read_to_string(&log).unwrap();
    assert_eq!(runs.lines().count(), 1, "{runs}");
}

// The logging ffprobe is a shell script.
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

/// Starts a Server with `ffmpeg` and a Movies Library holding Alien as an MP4 that ffmpeg wrote
/// from [`ffmpeg::write_h264_with_5_1`]'s H.264 with B-frames, with its `moov` before `mdat` if
/// `faststart`, after it if not, and returns it with that Version's id.
#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "a test helper fails its test by panicking"
)]
async fn alien_mp4(ffmpeg: Ffmpeg, faststart: bool) -> (Fixture, i64) {
    let builder = TestServer::builder().ffmpeg(ffmpeg);
    let fx = Fixture::start_on(LibraryKind::LIBRARY_KIND_MOVIES, builder).await;
    let file = fx.folder.join("Alien (1979)/Alien (1979).mp4");
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    let mkv = tempfile::tempdir().unwrap();
    let mkv = mkv.path().join("alien.mkv");
    ffmpeg::write_h264_with_5_1(&mkv, "aac").unwrap();
    let status = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(&mkv)
        .args([
            "-c",
            "copy",
            "-movflags",
            if faststart {
                "+faststart"
            } else {
                "-faststart"
            },
        ])
        .arg(&file)
        .status()
        .unwrap();
    assert!(status.success());
    fx.scan_and_match().await;
    let version = fx.version_ids(ALIEN).await[0];
    (fx, version)
}

// The logging ffprobe is a shell script.
#[tokio::test]
async fn an_mp4_remuxes_on_its_first_play_without_a_whole_file_listing_wherever_its_moov_is() {
    for faststart in [true, false] {
        let bin = tempfile::tempdir().unwrap();
        let log = bin.path().join("runs.log");
        let ffprobe = ffprobe_logging_its_runs(bin.path(), &log);
        let (fx, version) = alien_mp4(ffprobe, faststart).await;

        let direct_play_failed = [PlaybackMethod::PLAYBACK_METHOD_DIRECT_PLAY];
        let plan = fx.plan_on_device(version, &direct_play_failed, 0).await;

        let plan = plan.unwrap();
        assert_eq!(
            plan.method,
            PlaybackMethod::PLAYBACK_METHOD_REMUX,
            "{faststart}"
        );
        fx.fetch_ok(&plan.url).await;
        let runs = fs::read_to_string(&log).unwrap();
        assert!(
            !runs.contains("packet=pts_time,flags"),
            "{faststart}: {runs}"
        );
    }
}

// The logging ffprobe is a shell script.
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
