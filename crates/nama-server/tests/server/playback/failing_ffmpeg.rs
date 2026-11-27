//! A plan whose ffmpeg stops before writing anything is dropped before the Device hears of it,
//! and the Server plans the next method in the same request (#1126). This proves it over a real
//! process and the wire; `Plans`' unit tests cover the rest of replanning in memory.

#[cfg(unix)]
use super::*;

#[cfg(unix)]
const FAILED_ON_SERVER: PlaybackReasonKind =
    PlaybackReasonKind::PLAYBACK_REASON_KIND_METHOD_FAILED_ON_SERVER;

/// Starts a Server with [`ffmpeg_writing_nothing`]'s ffprobe and an ffmpeg, a shell script in
/// `bin`, that keeps running for a Transcode and exits 1 at once for a Remux, and a Movies
/// Library holding Alien in 1080p and 2160p, and returns it with the ids of those Versions, which
/// Chrome Remuxes and Transcodes.
#[cfg(unix)]
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn alien_with_a_failing_remux(bin: &Path) -> (Fixture, [i64; 2]) {
    let ffmpeg = "#!/bin/sh\ncase \"$*\" in *-encoders*) echo ' V....D libx264  H.264' ;; \
                  *-filters*) ;; *libx264*) exec sleep 600 ;; \
                  *) echo 'Unrecognized option' >&2; exit 1 ;; esac\n";
    let ffmpeg = Ffmpeg {
        ffmpeg: Ok(script(bin, "ffmpeg-failing", ffmpeg)),
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
#[cfg(unix)]
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
    let (fx, [h264, _]) = alien_with_a_failing_remux(bin.path()).await;
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
