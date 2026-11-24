//! Serving a Version's file as it is: off the local network, a User capped for remote playback
//! gets it only through a plan that Direct Plays it, which kept to their cap.

use super::*;

const REMOTE: &str = "203.0.113.7";
const LOCAL: &str = "192.168.1.20";
const ALIEN_720P: &str = "Alien (1979)/Alien (1979) - 720p.mp4";

/// An ffprobe, a shell script in `bin`, that reads every file as an H.264 and AAC MP4, which
/// [`chrome`] Direct Plays, 720p when its name says so and 1080p otherwise.
fn ffprobe_reading_height_by_name(bin: &Path) -> Ffmpeg {
    let ffprobe = r#"#!/bin/sh
case "$*" in *720p*) size='"width": 1280, "height": 720' ;; *) size='"width": 1920, "height": 1080' ;; esac
cat <<EOF
{"streams": [{"codec_name": "h264", "codec_type": "video", $size},
             {"codec_name": "aac", "codec_type": "audio", "disposition": {"default": 1}}],
 "format": {"format_name": "mov,mp4,m4a,3gp,3g2,mj2", "duration": "1.000000"}}
EOF
"#;
    Ffmpeg {
        ffprobe: Ok(script(bin, "ffprobe", ffprobe)),
        ..Ffmpeg::default()
    }
}

/// Alien in 1080p and 720p, probed with [`ffprobe_reading_height_by_name`] in `bin`, on a Server
/// behind a trusted proxy, with those Versions' ids, and Bishop, capped at 720p remotely.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn capped_bishop(bin: &Path) -> (Fixture, [i64; 2], SignedIn) {
    let builder = behind_proxy().ffmpeg(ffprobe_reading_height_by_name(bin));
    let fx = Fixture::start_on(LibraryKind::LIBRARY_KIND_MOVIES, builder).await;
    for file in [ALIEN_1080P, ALIEN_720P] {
        fx.write(file, b"video");
    }
    fx.scan_and_probe().await;
    let versions = fx.version_ids(ALIEN).await.try_into().unwrap();
    let bishop = fx.server.user_with_default_permissions("Bishop").await;
    (UserServiceClient::new(HttpClient::plaintext(), fx.server.client_config()))
        .set_permissions(SetPermissionsRequest {
            user_id: bishop.id,
            remote_playback_limit: PlaybackLimit::PLAYBACK_LIMIT_720P.into(),
            ..Default::default()
        })
        .await
        .unwrap();
    (fx, versions, bishop)
}

/// The status of `token`'s request for the Version `version`'s file, relayed for `client`.
async fn status_for(fx: &Fixture, version: i64, token: &str, client: &str) -> u16 {
    let path = format!("/media/versions/{version}");
    let headers = [("x-forwarded-for", client)];
    (fetch_with(&fx.server, &path, Some(token), &headers).await).status
}

#[tokio::test]
async fn a_capped_user_off_the_local_network_is_refused_a_version_no_plan_direct_plays() {
    let bin = tempfile::tempdir().unwrap();
    let (fx, [p1080, _], bishop) = capped_bishop(bin.path()).await;

    assert_eq!(status_for(&fx, p1080, &bishop.token, REMOTE).await, 403);
}

#[tokio::test]
async fn a_capped_user_off_the_local_network_is_sent_a_version_their_plan_direct_plays() {
    let bin = tempfile::tempdir().unwrap();
    let (fx, [_, p720], bishop) = capped_bishop(bin.path()).await;
    let remotely = (bishop.client_config()).with_default_header("x-forwarded-for", REMOTE);
    let original = picking(p720, PlaybackLimit::PLAYBACK_LIMIT_ORIGINAL);
    let plan = ask_for_plan_as(remotely, original).await.unwrap();
    assert_eq!(plan.method, PlaybackMethod::PLAYBACK_METHOD_DIRECT_PLAY);
    assert_eq!(
        plan.remote_playback_limit,
        PlaybackLimit::PLAYBACK_LIMIT_720P
    );

    assert_eq!(status_for(&fx, p720, &bishop.token, REMOTE).await, 200);
}

#[tokio::test]
async fn a_capped_user_who_leaves_the_local_network_is_refused_a_version_planned_on_it() {
    let bin = tempfile::tempdir().unwrap();
    let (fx, [p1080, _], bishop) = capped_bishop(bin.path()).await;
    let locally = (bishop.client_config()).with_default_header("x-forwarded-for", LOCAL);
    let original = picking(p1080, PlaybackLimit::PLAYBACK_LIMIT_ORIGINAL);
    let plan = ask_for_plan_as(locally, original).await.unwrap();
    assert_eq!(plan.method, PlaybackMethod::PLAYBACK_METHOD_DIRECT_PLAY);

    assert_eq!(status_for(&fx, p1080, &bishop.token, REMOTE).await, 403);
}

#[tokio::test]
async fn a_capped_user_on_the_local_network_is_sent_a_version_planned_on_it() {
    let bin = tempfile::tempdir().unwrap();
    let (fx, [p1080, _], bishop) = capped_bishop(bin.path()).await;
    let locally = (bishop.client_config()).with_default_header("x-forwarded-for", LOCAL);
    let original = picking(p1080, PlaybackLimit::PLAYBACK_LIMIT_ORIGINAL);
    let plan = ask_for_plan_as(locally, original).await.unwrap();
    assert_eq!(plan.method, PlaybackMethod::PLAYBACK_METHOD_DIRECT_PLAY);

    assert_eq!(status_for(&fx, p1080, &bishop.token, LOCAL).await, 200);
}

#[tokio::test]
async fn a_capped_user_on_the_local_network_is_sent_a_version_with_no_plan() {
    let bin = tempfile::tempdir().unwrap();
    let (fx, [p1080, _], bishop) = capped_bishop(bin.path()).await;

    let status = status_for(&fx, p1080, &bishop.token, LOCAL).await;

    assert_eq!(status, 200);
}

#[tokio::test]
async fn an_uncapped_user_off_the_local_network_is_sent_a_version_with_no_plan() {
    let bin = tempfile::tempdir().unwrap();
    let (fx, [p1080, _], _) = capped_bishop(bin.path()).await;

    let status = status_for(&fx, p1080, &fx.server.admin_token, REMOTE).await;

    assert_eq!(status, 200);
}
