//! A plan whose ffmpeg stops before writing anything is dropped before the Device hears of it,
//! and the Server plans the next method in the same request (#1126).

use super::*;

const FAILED_ON_SERVER: PlaybackReasonKind =
    PlaybackReasonKind::PLAYBACK_REASON_KIND_METHOD_FAILED_ON_SERVER;

/// Starts a Server with [`ffmpeg_writing_nothing`]'s ffprobe and an ffmpeg, a shell script in
/// `bin`, that runs `transcode` for a Transcode and exits 1 at once for a Remux, and a Movies
/// Library holding Alien in 1080p and 2160p, and returns it with the ids of those Versions, which
/// Chrome Remuxes and Transcodes.
#[cfg(unix)]
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn alien_with_a_failing_remux(bin: &Path, transcode: &str) -> (Fixture, [i64; 2]) {
    let ffmpeg = format!(
        "#!/bin/sh\ncase \"$*\" in *-encoders*) echo ' V....D libx264  H.264' ;; \
         *-filters*) ;; *libx264*) {transcode} ;; *) echo 'Unrecognized option' >&2; exit 1 ;; \
         esac\n"
    );
    let ffmpeg = Ffmpeg {
        ffmpeg: Ok(script(bin, "ffmpeg-failing", &ffmpeg)),
        ..ffmpeg_writing_nothing(bin, "libx264")
    };
    let fx = Fixture::start_with_ffmpeg(LibraryKind::LIBRARY_KIND_MOVIES, ffmpeg).await;
    fx.write(ALIEN_1080P, b"video");
    fx.write(ALIEN_2160P, b"video");
    fx.scan_and_match().await;
    let versions = fx.version_ids(ALIEN).await.try_into().unwrap();
    (fx, versions)
}

/// The methods `plan`'s reasons say failed on the Server, in order.
fn failed_on_server(plan: &PlaybackPlan) -> Vec<&str> {
    (plan.reasons.iter())
        .filter(|reason| reason.kind == FAILED_ON_SERVER)
        .filter_map(|reason| param(reason, "method"))
        .collect()
}

// The fake ffmpeg and ffprobe are shell scripts.
#[cfg(unix)]
#[tokio::test]
async fn a_remux_whose_ffmpeg_fails_at_once_transcodes_saying_why_and_only_the_transcode_is_told() {
    let bin = tempfile::tempdir().unwrap();
    let (fx, [h264, _]) = alien_with_a_failing_remux(bin.path(), "exec sleep 600").await;
    let playback = [
        EventType::EVENT_TYPE_PLAYBACK_STARTED,
        EventType::EVENT_TYPE_PLAYBACK_CHANGED,
        EventType::EVENT_TYPE_PLAYBACK_ENDED,
    ];
    let mut subscription = subscribe(&fx.server.client_config(), &playback).await;

    let plan = fx.transcode(h264).await;

    assert_eq!(failed_on_server(&plan), ["PLAYBACK_METHOD_REMUX"]);
    assert!(reason_of(&plan, FAILED_ON_SERVER).decided);
    let told = PlaybackStarted {
        playback: admins_playback(h264, plan).into(),
        ..Default::default()
    };
    assert_eq!(subscription.next().await, told.into());
}

// The fake ffmpeg and ffprobe are shell scripts.
#[cfg(unix)]
#[tokio::test]
async fn a_version_whose_ffmpeg_fails_at_once_is_unplayable_within_seconds_leaving_no_folder() {
    let bin = tempfile::tempdir().unwrap();
    let (fx, [h264, _]) = alien_with_a_failing_remux(bin.path(), "exit 1").await;

    let asked = std::time::Instant::now();
    let plan = fx.plan(h264).await.unwrap();
    let waited = asked.elapsed();

    assert_eq!(plan.method, PlaybackMethod::PLAYBACK_METHOD_UNSPECIFIED);
    assert_eq!(
        failed_on_server(&plan),
        ["PLAYBACK_METHOD_REMUX", "PLAYBACK_METHOD_TRANSCODE"]
    );
    // Short of the 5 s the Server waits for a plan's first file at most.
    assert!(waited < Duration::from_secs(5), "{waited:?}");
    let segment_folder = fx.server.data_dir().join("transcodes");
    let left: Vec<_> = fs::read_dir(segment_folder)
        .map_or_default(|entries| entries.map(|entry| entry.unwrap().file_name()).collect());
    assert_eq!(left, Vec::<std::ffi::OsString>::new());
}

// The fake ffmpeg and ffprobe are shell scripts.
#[cfg(unix)]
#[tokio::test]
async fn transcodes_whose_ffmpeg_fails_at_once_dont_count_toward_the_transcode_limit() {
    let bin = tempfile::tempdir().unwrap();
    let (fx, [_, hevc]) = alien_with_a_failing_remux(bin.path(), "exit 1").await;
    fx.allow_one_transcode().await;

    let first = fx.plan(hevc).await.unwrap();
    let second = fx.plan(hevc).await.unwrap();

    for plan in [first, second] {
        assert_eq!(failed_on_server(&plan), ["PLAYBACK_METHOD_TRANSCODE"]);
    }
}
