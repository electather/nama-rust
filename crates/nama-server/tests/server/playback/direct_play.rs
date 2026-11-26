//! Serving a Version's file as it is: off the local network, a User capped for remote playback
//! gets it only through a plan that Direct Plays it, which kept to their cap where the picture's
//! size is known.

use nama_proto::nama::v1::{ListPlaysRequest, ReportProgressRequest, WatchStateServiceClient};

use super::{
    switching_users::{sign_in_on, switch_back_to},
    *,
};

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

/// Alien in 1080p and 720p, probed with [`ffprobe_reading_height_by_name`] in `bin`, on the Server
/// `builder` starts, which must trust the proxy as [`behind_proxy`]'s does, with those Versions'
/// ids, and Bishop, capped at 720p remotely.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn capped_bishop(builder: Builder, bin: &Path) -> (Fixture, [i64; 2], SignedIn) {
    let builder = builder.ffmpeg(ffprobe_reading_height_by_name(bin));
    let fx = Fixture::start_on(LibraryKind::LIBRARY_KIND_MOVIES, builder).await;
    for file in [ALIEN_1080P, ALIEN_720P] {
        fx.write(file, b"video");
    }
    fx.scan_and_probe().await;
    let versions = fx.version_ids(ALIEN).await.try_into().unwrap();
    let bishop = capped_at_720p(&fx, "Bishop").await;
    (fx, versions, bishop)
}

/// `name`, a new User on `fx`'s Server, capped at 720p remotely.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn capped_at_720p(fx: &Fixture, name: &str) -> SignedIn {
    let user = fx.server.user_with_default_permissions(name).await;
    (UserServiceClient::new(HttpClient::plaintext(), fx.server.client_config()))
        .set_permissions(SetPermissionsRequest {
            user_id: user.id,
            remote_playback_limit: PlaybackLimit::PLAYBACK_LIMIT_720P.into(),
            ..Default::default()
        })
        .await
        .unwrap();
    user
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
    let (fx, [p1080, _], bishop) = capped_bishop(behind_proxy(), bin.path()).await;

    assert_eq!(status_for(&fx, p1080, &bishop.token, REMOTE).await, 403);
}

#[tokio::test]
async fn a_capped_user_off_the_local_network_is_sent_a_version_their_plan_direct_plays() {
    let bin = tempfile::tempdir().unwrap();
    let (fx, [_, p720], bishop) = capped_bishop(behind_proxy(), bin.path()).await;
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

// A plan belongs to the User it was made for (ADR 0106), so the cap is checked as the User the
// Device acts as now.
#[tokio::test]
async fn a_direct_play_planned_under_a_cap_lets_no_user_the_device_switches_to_past_theirs() {
    let bin = tempfile::tempdir().unwrap();
    let (fx, [_, p720], bishop) = capped_bishop(behind_proxy(), bin.path()).await;
    capped_at_720p(&fx, "Hicks").await;
    let remotely = (bishop.client_config()).with_default_header("x-forwarded-for", REMOTE);
    let original = picking(p720, PlaybackLimit::PLAYBACK_LIMIT_ORIGINAL);
    let plan = ask_for_plan_as(remotely, original).await.unwrap();
    assert_eq!(plan.method, PlaybackMethod::PLAYBACK_METHOD_DIRECT_PLAY);

    sign_in_on(&fx.server, &bishop.token, "Hicks").await;
    let as_hicks = status_for(&fx, p720, &bishop.token, REMOTE).await;
    switch_back_to(&fx.server, &bishop.token, &bishop).await;

    assert_eq!(as_hicks, 403);
    assert_eq!(status_for(&fx, p720, &bishop.token, REMOTE).await, 200);
}

// The cap can't be kept without the picture's size, so it's best-effort there (ADR 0026, #2134).
#[tokio::test]
async fn a_capped_user_off_the_local_network_direct_plays_and_seeks_in_an_unprobed_version() {
    let fx = Fixture::start_on(LibraryKind::LIBRARY_KIND_MOVIES, behind_proxy()).await;
    fx.write(ALIEN_1080P, b"video");
    fx.scan_and_probe().await;
    let version = fx.version_ids(ALIEN).await[0];
    let bishop = capped_at_720p(&fx, "Bishop").await;
    let remotely = (bishop.client_config()).with_default_header("x-forwarded-for", REMOTE);
    let original = picking(version, PlaybackLimit::PLAYBACK_LIMIT_ORIGINAL);

    let plan = ask_for_plan_as(remotely, original).await.unwrap();

    assert_eq!(plan.method, PlaybackMethod::PLAYBACK_METHOD_DIRECT_PLAY);
    let unapplied = PlaybackReasonKind::PLAYBACK_REASON_KIND_REMOTE_CAP_PICTURE_SIZE_UNKNOWN;
    assert_eq!(
        param(reason_of(&plan, unapplied), "playbackLimit"),
        Some("PLAYBACK_LIMIT_720P")
    );
    let path = format!("/media/versions/{version}");
    let seek = [("x-forwarded-for", REMOTE), ("range", "bytes=2-")];
    let status = (fetch_with(&fx.server, &path, Some(&bishop.token), &seek).await).status;
    assert_eq!(status, 206);
}

/// Reports, as `who`, being paused a second into the Version `version`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn report_paused_as(who: ClientConfig, version: i64) {
    WatchStateServiceClient::new(HttpClient::plaintext(), who)
        .report_progress(ReportProgressRequest {
            version_id: version,
            position_ms: 1_000,
            duration_ms: 60_000,
            ..Default::default()
        })
        .await
        .unwrap();
}

/// Waits until fewer than `n` of `fx`'s plans are playing, failing after a few sweeps of unused
/// plans, which come every 10 s.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn until_fewer_playing(fx: &Fixture, n: usize) {
    let server = ServerServiceClient::new(HttpClient::plaintext(), fx.server.client_config());
    let playing = || async { server.list_plays(ListPlaysRequest::default()).await };
    let swept = async {
        while playing().await.unwrap().into_owned().plays.len() >= n {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    };
    let waited = tokio::time::timeout(WAIT * 3, swept).await;
    assert!(waited.is_ok(), "{n} or more plans still playing");
}

#[tokio::test]
async fn a_capped_user_off_the_local_network_paused_past_a_minute_is_still_sent_their_direct_play()
{
    let bin = tempfile::tempdir().unwrap();
    let (fx, [_, p720], bishop) = capped_bishop(behind_proxy(), bin.path()).await;
    let remotely = (bishop.client_config()).with_default_header("x-forwarded-for", REMOTE);
    let original = picking(p720, PlaybackLimit::PLAYBACK_LIMIT_ORIGINAL);
    ask_for_plan_as(remotely.clone(), original.clone())
        .await
        .unwrap();
    // The admin's plan reports nothing, so it shows when a sweep has stopped what went unused.
    ask_for_plan_as(fx.server.client_config(), original)
        .await
        .unwrap();

    for _ in 0..2 {
        fx.server.advance_clock(Duration::from_secs(31));
        report_paused_as(remotely.clone(), p720).await;
    }
    until_fewer_playing(&fx, 2).await;

    let path = format!("/media/versions/{p720}");
    let seek = [("x-forwarded-for", REMOTE), ("range", "bytes=2-")];
    let status = (fetch_with(&fx.server, &path, Some(&bishop.token), &seek).await).status;
    assert_eq!(status, 206);
}

#[tokio::test]
async fn a_capped_user_who_leaves_the_local_network_is_refused_a_version_planned_on_it() {
    let bin = tempfile::tempdir().unwrap();
    let (fx, [p1080, _], bishop) = capped_bishop(behind_proxy(), bin.path()).await;
    let locally = (bishop.client_config()).with_default_header("x-forwarded-for", LOCAL);
    let original = picking(p1080, PlaybackLimit::PLAYBACK_LIMIT_ORIGINAL);
    let plan = ask_for_plan_as(locally, original).await.unwrap();
    assert_eq!(plan.method, PlaybackMethod::PLAYBACK_METHOD_DIRECT_PLAY);

    assert_eq!(status_for(&fx, p1080, &bishop.token, REMOTE).await, 403);
}

#[tokio::test]
async fn a_capped_user_on_the_local_network_is_sent_a_version_planned_on_it() {
    let bin = tempfile::tempdir().unwrap();
    let (fx, [p1080, _], bishop) = capped_bishop(behind_proxy(), bin.path()).await;
    let locally = (bishop.client_config()).with_default_header("x-forwarded-for", LOCAL);
    let original = picking(p1080, PlaybackLimit::PLAYBACK_LIMIT_ORIGINAL);
    let plan = ask_for_plan_as(locally, original).await.unwrap();
    assert_eq!(plan.method, PlaybackMethod::PLAYBACK_METHOD_DIRECT_PLAY);

    assert_eq!(status_for(&fx, p1080, &bishop.token, LOCAL).await, 200);
}

#[tokio::test]
async fn a_capped_user_on_the_local_network_is_sent_a_version_with_no_plan() {
    let bin = tempfile::tempdir().unwrap();
    let (fx, [p1080, _], bishop) = capped_bishop(behind_proxy(), bin.path()).await;

    let status = status_for(&fx, p1080, &bishop.token, LOCAL).await;

    assert_eq!(status, 200);
}

#[tokio::test]
async fn an_uncapped_user_off_the_local_network_is_sent_a_version_with_no_plan() {
    let bin = tempfile::tempdir().unwrap();
    let (fx, [p1080, _], _) = capped_bishop(behind_proxy(), bin.path()).await;

    let status = status_for(&fx, p1080, &fx.server.admin_token, REMOTE).await;

    assert_eq!(status, 200);
}
