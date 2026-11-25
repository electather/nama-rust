//! Downloading a Version's file to keep on the Device, for a User with `PERMISSION_DOWNLOAD`,
//! only while the Device's plan for it Direct Plays it.

use super::*;

/// Where a Version's file is downloaded from.
fn download_url(version: i64) -> String {
    format!("/media/downloads/{version}")
}

/// Alien, which Chrome Direct Plays, and Ada, who may download.
#[expect(
    clippy::indexing_slicing,
    reason = "a test helper fails its test by panicking"
)]
async fn alien_and_ada() -> (Fixture, i64, SignedIn) {
    let fx = Fixture::alien().await;
    let version = fx.version_ids(ALIEN).await[0];
    let ada = (fx.server)
        .user("ada", &[Permission::PERMISSION_DOWNLOAD])
        .await;
    (fx, version, ada)
}

#[tokio::test]
async fn a_user_with_download_gets_the_bytes_and_a_range_of_a_version_their_device_direct_plays() {
    let (fx, version, ada) = alien_and_ada().await;
    let plan = plan_as(&ada, version).await.unwrap();
    assert_eq!(plan.method, PlaybackMethod::PLAYBACK_METHOD_DIRECT_PLAY);
    let url = download_url(version);

    let whole = fetch(&fx.server, &url, Some(&ada.token)).await;
    let range = [("range", "bytes=1-3"), ("if-range", whole.header("etag"))];
    let part = fetch_with(&fx.server, &url, Some(&ada.token), &range).await;

    assert_eq!(whole.status, 200);
    assert_eq!(whole.body, fs::read(fx.folder.join(ALIEN_1080P)).unwrap());
    assert_eq!(whole.header("content-length"), whole.body.len().to_string());
    assert!(whole.header("etag").starts_with('"'), "no strong ETag");
    assert_eq!(part.status, 206);
    assert_eq!(part.body, whole.body[1..=3]);
    let length = whole.body.len();
    assert_eq!(part.header("content-range"), format!("bytes 1-3/{length}"));
}

#[tokio::test]
async fn a_user_without_download_is_refused_a_version_their_device_direct_plays() {
    let fx = Fixture::alien().await;
    let version = fx.version_ids(ALIEN).await[0];
    let bishop = fx.server.user_with_default_permissions("Bishop").await;
    let plan = plan_as(&bishop, version).await.unwrap();
    assert_eq!(plan.method, PlaybackMethod::PLAYBACK_METHOD_DIRECT_PLAY);

    let reply = fetch(&fx.server, &download_url(version), Some(&bishop.token)).await;

    assert_eq!(reply.status, 403);
    let refusal = assert_refused(
        Err::<(), ConnectError>(serde_json::from_slice(&reply.body).unwrap()),
        ErrorCode::PermissionDenied,
        RefusalReason::REFUSAL_REASON_PERMISSION_REQUIRED,
    );
    assert_eq!(refusal.params["permission"], "PERMISSION_DOWNLOAD");
}

#[tokio::test]
async fn a_version_the_device_has_no_direct_play_of_is_refused_as_a_conflict() {
    let (fx, version, ada) = alien_and_ada().await;

    let reply = fetch(&fx.server, &download_url(version), Some(&ada.token)).await;

    assert_eq!(reply.status, 409);
}

// The fake ffmpeg and ffprobe are shell scripts.
#[cfg(unix)]
#[tokio::test]
async fn a_version_the_device_remuxes_is_refused_as_a_conflict() {
    let bin = tempfile::tempdir().unwrap();
    let (fx, [h264, _]) = Fixture::alien_with_room_for_one_transcode(bin.path()).await;
    let ada = (fx.server)
        .user("ada", &[Permission::PERMISSION_DOWNLOAD])
        .await;
    let plan = plan_as(&ada, h264).await.unwrap();
    assert_eq!(plan.method, PlaybackMethod::PLAYBACK_METHOD_REMUX);

    let reply = fetch(&fx.server, &download_url(h264), Some(&ada.token)).await;

    assert_eq!(reply.status, 409);
}

#[tokio::test]
async fn an_unknown_version_is_not_found() {
    let (fx, version, ada) = alien_and_ada().await;

    let reply = fetch(&fx.server, &download_url(version + 1000), Some(&ada.token)).await;

    assert_eq!(reply.status, 404);
}
