//! An unplayable plan lists the other Versions of its title that would play on the Device.

use nama_proto::nama::v1::PlanPlaybackResponse;

use super::*;

const ALIEN_720P: &str = "Alien (1979)/Alien (1979) - 720p.mp4";
const ALIEN_480P: &str = "Alien (1979)/Alien (1979) - 480p.mkv";

/// A Server whose ffmpeg and ffprobe are shell scripts in `bin`. ffprobe writes each run's
/// arguments to `bin/probes.log`, and reads a file whose name says 2160p as a Dolby Vision
/// profile 5 MKV, which neither [`chrome`] decodes nor a Server Transcodes, one that says 720p as
/// an H.264 and AAC MP4, which Chrome Direct Plays, and any other as an H.264 and AAC MKV, which
/// it Remuxes, but fails on one that says 480p. ffmpeg lists libx264 and no filters.
fn ffmpeg_reading_alien_by_name(bin: &Path) -> Ffmpeg {
    let ffprobe = format!(
        r#"#!/bin/sh
echo "$*" >> '{}'
format=matroska,webm
video='"codec_name": "h264"'
case "$*" in
*480p*) exit 1 ;;
*2160p*) video='"codec_name": "hevc", "side_data_list": [{{"side_data_type": "DOVI configuration record", "dv_profile": 5, "dv_bl_signal_compatibility_id": 0}}]' ;;
*720p*) format=mov,mp4,m4a,3gp,3g2,mj2 ;;
esac
cat <<EOF
{{"streams": [{{$video, "codec_type": "video"}},
             {{"codec_name": "aac", "codec_type": "audio", "disposition": {{"default": 1}}}}],
 "format": {{"format_name": "$format", "duration": "1.000000"}}}}
EOF
"#,
        bin.join("probes.log").display()
    );
    let ffmpeg =
        "#!/bin/sh\ncase \"$*\" in *-encoders*) echo ' V....D libx264  H.264' ;; *) ;; esac\n";
    Ffmpeg {
        ffmpeg: Ok(script(bin, "ffmpeg", ffmpeg)),
        ffprobe: Ok(script(bin, "ffprobe", &ffprobe)),
        gpus: vec![],
    }
}

/// Starts a Server with [`ffmpeg_reading_alien_by_name`] in `bin`, and a Movies Library holding
/// Alien in 1080p, 2160p and 720p and the files `more`, scanned, Matched and probed.
async fn alien_in_versions(bin: &Path, more: &[&str]) -> Fixture {
    let ffmpeg = ffmpeg_reading_alien_by_name(bin);
    let fx = Fixture::start_with_ffmpeg(LibraryKind::LIBRARY_KIND_MOVIES, ffmpeg).await;
    for file in [ALIEN_1080P, ALIEN_2160P, ALIEN_720P].iter().chain(more) {
        fx.write(file, b"video");
    }
    fx.scan_and_probe().await;
    fx
}

/// The Admin's answer to asking for a plan for the Version `version` on [`chrome`].
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn planned(fx: &Fixture, version: i64) -> PlanPlaybackResponse {
    PlaybackServiceClient::new(HttpClient::plaintext(), fx.server.client_config())
        .plan_playback(PlanPlaybackRequest {
            version_id: version,
            device_decoding: chrome().into(),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
}

/// How many times ffprobe ran on a file whose name says `name`.
fn probes_of(bin: &Path, name: &str) -> usize {
    // No log until the first run.
    let probes = fs::read_to_string(bin.join("probes.log")).unwrap_or_default();
    probes.lines().filter(|probe| probe.contains(name)).count()
}

#[tokio::test]
async fn a_dolby_vision_only_versions_unplayable_plan_lists_the_versions_that_play_best_first() {
    let bin = tempfile::tempdir().unwrap();
    let fx = alien_in_versions(bin.path(), &[]).await;
    let [h264_1080p, dolby_vision, h264_720p] = fx.version_ids(ALIEN).await.try_into().unwrap();

    let planned = planned(&fx, dolby_vision).await;

    let plan = planned.plan.into_option().unwrap();
    assert_eq!(plan.method, PlaybackMethod::PLAYBACK_METHOD_UNSPECIFIED);
    // The 720p Direct Plays, so it comes before the 1080p, which Remuxes.
    assert_eq!(planned.playable_versions, [h264_720p, h264_1080p]);
}

#[tokio::test]
async fn a_playable_plan_lists_no_other_versions() {
    let bin = tempfile::tempdir().unwrap();
    let fx = alien_in_versions(bin.path(), &[]).await;
    let [_, _, h264_720p] = fx.version_ids(ALIEN).await.try_into().unwrap();

    let planned = planned(&fx, h264_720p).await;

    let plan = planned.plan.into_option().unwrap();
    assert_eq!(plan.method, PlaybackMethod::PLAYBACK_METHOD_DIRECT_PLAY);
    assert_eq!(planned.playable_versions, Vec::<i64>::new());
}

#[tokio::test]
async fn an_unprobed_version_isnt_listed_on_an_unplayable_plan_nor_probed_for_it() {
    let bin = tempfile::tempdir().unwrap();
    let fx = alien_in_versions(bin.path(), &[ALIEN_480P]).await;
    let [h264_1080p, dolby_vision, _unprobed, h264_720p] =
        fx.version_ids(ALIEN).await.try_into().unwrap();
    let probes_before = probes_of(bin.path(), "480p");

    let planned = planned(&fx, dolby_vision).await;

    assert_eq!(planned.playable_versions, [h264_720p, h264_1080p]);
    assert_eq!(probes_of(bin.path(), "480p"), probes_before);
}
