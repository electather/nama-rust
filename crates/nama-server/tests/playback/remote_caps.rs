//! A User's cap for remote playback: the most their Device outside the local network is sent,
//! whatever Playback Limit it picks.

use nama_proto::nama::v1::{
    GetDefaultPermissionsRequest, ListUsersRequest, SetDefaultPermissionsRequest, track,
};

use super::*;

/// The Admin's `UserService` client.
fn admin_users(fx: &Fixture) -> UserServiceClient<HttpClient> {
    UserServiceClient::new(HttpClient::plaintext(), fx.server.client_config())
}

#[tokio::test]
async fn a_user_capped_by_default_is_sent_at_most_the_cap_remotely_whatever_they_pick() {
    let behind = behind_proxy().ffmpeg(system_ffmpeg());
    let (fx, version) = Fixture::alien_written_on(behind, ffmpeg::write_1080p_h264).await;
    let p720 = PlaybackLimit::PLAYBACK_LIMIT_720P;
    (admin_users(&fx).set_default_permissions(SetDefaultPermissionsRequest {
        remote_playback_limit: p720.into(),
        ..Default::default()
    }))
    .await
    .unwrap();
    let bishop = fx.server.user_with_default_permissions("Bishop").await;
    let original = picking(version, PlaybackLimit::PLAYBACK_LIMIT_ORIGINAL);
    let remotely = bishop
        .client_config()
        .with_default_header("x-forwarded-for", "203.0.113.7");

    let capped = ask_for_plan_as(remotely.clone(), original.clone())
        .await
        .unwrap();
    let local = ask_for_plan_as(bishop.client_config(), original.clone())
        .await
        .unwrap();

    assert_eq!(capped.method, PlaybackMethod::PLAYBACK_METHOD_TRANSCODE);
    assert_eq!(capped.playback_limit, p720);
    assert_eq!(capped.remote_playback_limit, p720);
    let cap = reason_of(&capped, PlaybackReasonKind::PLAYBACK_REASON_KIND_REMOTE_CAP);
    assert_eq!(param(cap, "playbackLimit"), Some("PLAYBACK_LIMIT_720P"));
    let sent = (capped.tracks.first()).and_then(|track| match &track.kind {
        Some(track::Kind::Video(video)) => Some((video.width, video.height)),
        _ => None,
    });
    assert_eq!(sent, Some((1280, 720)));
    assert_eq!(local.method, PlaybackMethod::PLAYBACK_METHOD_REMUX);
    assert_eq!(local.playback_limit, PlaybackLimit::PLAYBACK_LIMIT_ORIGINAL);
    assert_eq!(
        local.remote_playback_limit,
        PlaybackLimit::PLAYBACK_LIMIT_UNSPECIFIED
    );

    (admin_users(&fx).set_permissions(SetPermissionsRequest {
        user_id: bishop.id,
        remote_playback_limit: PlaybackLimit::PLAYBACK_LIMIT_ORIGINAL.into(),
        ..Default::default()
    }))
    .await
    .unwrap();
    let cleared = ask_for_plan_as(remotely, original).await.unwrap();

    assert_eq!(cleared.method, PlaybackMethod::PLAYBACK_METHOD_REMUX);
    assert_eq!(
        cleared.playback_limit,
        PlaybackLimit::PLAYBACK_LIMIT_ORIGINAL
    );
}

#[tokio::test]
async fn caps_read_back_from_the_users_list_and_the_default_permissions() {
    let fx = Fixture::alien().await;
    let bishop = fx.server.user_with_default_permissions("Bishop").await;
    let p480 = PlaybackLimit::PLAYBACK_LIMIT_480P;
    let users = admin_users(&fx);

    (users.set_permissions(SetPermissionsRequest {
        user_id: bishop.id,
        remote_playback_limit: p480.into(),
        ..Default::default()
    }))
    .await
    .unwrap();
    (users.set_default_permissions(SetDefaultPermissionsRequest {
        remote_playback_limit: p480.into(),
        ..Default::default()
    }))
    .await
    .unwrap();

    let listed = (users.list_users(ListUsersRequest::default()).await)
        .unwrap()
        .into_owned()
        .users;
    let bishops = listed.iter().find(|user| user.id == bishop.id).unwrap();
    assert_eq!(bishops.remote_playback_limit, p480);
    let uncapped = PlaybackLimit::PLAYBACK_LIMIT_ORIGINAL;
    assert!(
        (listed.iter())
            .filter(|user| user.id != bishop.id)
            .all(|user| user.remote_playback_limit == uncapped),
        "{listed:?}"
    );
    let defaults = (users
        .get_default_permissions(GetDefaultPermissionsRequest::default())
        .await)
        .unwrap()
        .into_owned();
    assert_eq!(defaults.remote_playback_limit, p480);
}

#[tokio::test]
async fn a_cap_this_server_doesnt_know_is_refused() {
    let fx = Fixture::alien().await;
    let bishop = fx.server.user_with_default_permissions("Bishop").await;

    let refused = (admin_users(&fx).set_permissions(SetPermissionsRequest {
        user_id: bishop.id,
        remote_playback_limit: EnumValue::Unknown(99),
        ..Default::default()
    }))
    .await
    .unwrap_err();

    assert_eq!(refused.code, ErrorCode::InvalidArgument);
}
