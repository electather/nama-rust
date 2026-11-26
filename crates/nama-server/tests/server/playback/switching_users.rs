//! A Device switching between the Users signed in to it (ADR 0106): a plan belongs to the User it
//! was made for, so the Device gets it only while it acts as them.

use nama_proto::nama::v1::{
    SessionDelivery, SignInRequest, SignInServiceClient, SwitchUserRequest,
};
use server::PASSWORD;

use super::*;

/// Signs `name` in on the Device holding the session `token`, which then acts as them.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
pub(super) async fn sign_in_on(server: &TestServer, token: &str, name: &str) {
    SignInServiceClient::new(HttpClient::plaintext(), server.client_config_as(token))
        .sign_in(SignInRequest {
            name: name.to_owned(),
            password: PASSWORD.to_owned(),
            device_name: "Living room TV".to_owned(),
            delivery: SessionDelivery::SESSION_DELIVERY_TOKEN.into(),
            ..Default::default()
        })
        .await
        .unwrap();
}

/// Switches the Device holding the session `token` back to `user`, who has no Switch PIN.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
pub(super) async fn switch_back_to(server: &TestServer, token: &str, user: &SignedIn) {
    SignInServiceClient::new(HttpClient::plaintext(), server.client_config_as(token))
        .switch_user(SwitchUserRequest {
            user_id: user.id,
            ..Default::default()
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn a_remux_is_served_to_its_device_only_while_it_acts_as_the_user_it_was_planned_for() {
    let (fx, version) = Fixture::alien_mkv(system_ffmpeg(), "aac").await;
    let ripley = fx.server.user_with_default_permissions("Ripley").await;
    fx.server.create_user("Hicks").await;
    let original = picking(version, PlaybackLimit::PLAYBACK_LIMIT_ORIGINAL);
    let plan = ask_for_plan_as(ripley.client_config(), original)
        .await
        .unwrap();
    assert_eq!(plan.method, PlaybackMethod::PLAYBACK_METHOD_REMUX);
    let init = beside(&plan.url, "init.mp4");
    let status = || async { fetch(&fx.server, &init, Some(&ripley.token)).await.status };

    sign_in_on(&fx.server, &ripley.token, "Hicks").await;
    let as_hicks = status().await;
    switch_back_to(&fx.server, &ripley.token, &ripley).await;

    assert_eq!(as_hicks, 404);
    assert_eq!(status().await, 200);
}
