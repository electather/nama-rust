//! A Device signed in as several Users (ADR 0106): adding a User with a second sign-in, listing
//! its Users, switching between them and signing one out, through the generated clients, over a
//! real socket and against a real SQLite database.

#[path = "support/refused.rs"]
mod refused;
#[path = "support/server.rs"]
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
mod server;

use connectrpc::{
    ConnectError, ErrorCode,
    client::{ClientConfig, HttpClient},
};
use nama_proto::nama::v1::{
    ApproveSignInCodeRequest, ChangePasswordRequest, ListDevicesRequest, PollSignInCodeRequest,
    RefusalReason, RevokeDeviceRequest, SessionDelivery, SignInRequest, SignInServiceClient,
    StartSignInCodeRequest, SwitchUserRequest, User, UserServiceClient, WhoAmIRequest,
    WhoAmIResponse,
};
use refused::assert_refused;
use server::{PASSWORD, SignedIn, TestServer, subscribe};

fn sign_in_service(server: &TestServer, token: &str) -> SignInServiceClient<HttpClient> {
    SignInServiceClient::new(HttpClient::plaintext(), server.client_config_as(token))
}

fn user_service(server: &TestServer, token: &str) -> UserServiceClient<HttpClient> {
    UserServiceClient::new(HttpClient::plaintext(), server.client_config_as(token))
}

/// Signs `name` in with their password on the Device holding the session `token`, answering the
/// session the sign-in hands out.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn sign_in_on(server: &TestServer, token: &str, name: &str) -> String {
    sign_in_service(server, token)
        .sign_in(SignInRequest {
            name: name.to_owned(),
            password: PASSWORD.to_owned(),
            device_name: "Living room TV".to_owned(),
            delivery: SessionDelivery::SESSION_DELIVERY_TOKEN.into(),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .token
}

async fn who_am_i(server: &TestServer, token: &str) -> Result<WhoAmIResponse, ConnectError> {
    Ok(sign_in_service(server, token)
        .who_am_i(WhoAmIRequest::default())
        .await?
        .into_owned())
}

async fn switch(server: &TestServer, token: &str, user: i64) -> Result<(), ConnectError> {
    sign_in_service(server, token)
        .switch_user(SwitchUserRequest {
            user_id: user,
            ..Default::default()
        })
        .await?;
    Ok(())
}

/// Signs `user` out of the Device `device`, as whoever `who` calls the Server as.
async fn revoke(who: ClientConfig, user: i64, device: i64) -> Result<(), ConnectError> {
    UserServiceClient::new(HttpClient::plaintext(), who)
        .revoke_device(RevokeDeviceRequest {
            user_id: Some(user),
            device_id: device,
            ..Default::default()
        })
        .await?;
    Ok(())
}

/// The `(id, name)` of each User `me` lists on the Device.
fn device_users(me: &WhoAmIResponse) -> Vec<(i64, &str)> {
    (me.device_users.iter())
        .map(|user| (user.id, user.name.as_str()))
        .collect()
}

/// The id of the User the Device acts as, if any.
fn acting_as(me: &WhoAmIResponse) -> Option<i64> {
    me.user.as_option().map(|user: &User| user.id)
}

/// A Server with Ada signed in on a Device, then Grace added to it by a second sign-in.
async fn ada_and_grace_on_one_device() -> (TestServer, SignedIn, SignedIn) {
    let server = TestServer::builder().start().await;
    let ada = server.user("Ada", &[]).await;
    let grace = server.user("Grace", &[]).await;
    let added = sign_in_on(&server, &ada.token, "Grace").await;
    assert_eq!(added, ada.token, "the Device's session changed");
    (server, ada, grace)
}

#[tokio::test]
async fn a_second_sign_in_on_a_device_adds_its_user_and_the_device_lists_exactly_its_users() {
    let (server, ada, grace) = ada_and_grace_on_one_device().await;

    let me = who_am_i(&server, &ada.token).await.unwrap();

    assert_eq!(acting_as(&me), Some(grace.id));
    assert_eq!(device_users(&me), [(ada.id, "Ada"), (grace.id, "Grace")]);
    assert_eq!(me.device.name, "Phone", "the Device was renamed");
}

#[tokio::test]
async fn approving_a_sign_in_code_for_a_device_that_polls_with_its_session_adds_the_approver() {
    let server = TestServer::builder().start().await;
    let ada = server.user("Ada", &[]).await;
    let grace = server.user("Grace", &[]).await;
    let tv = sign_in_service(&server, &ada.token);
    let started = tv
        .start_sign_in_code(StartSignInCodeRequest {
            delivery: SessionDelivery::SESSION_DELIVERY_TOKEN.into(),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned();

    sign_in_service(&server, &grace.token)
        .approve_sign_in_code(ApproveSignInCodeRequest {
            code: started.code,
            ..Default::default()
        })
        .await
        .unwrap();
    let polled = tv
        .poll_sign_in_code(PollSignInCodeRequest {
            handle: started.handle,
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned();

    assert_eq!(polled.token, ada.token, "the Device's session changed");
    let me = who_am_i(&server, &ada.token).await.unwrap();
    assert_eq!(acting_as(&me), Some(grace.id));
    assert_eq!(device_users(&me), [(ada.id, "Ada"), (grace.id, "Grace")]);
}

#[tokio::test]
async fn switching_to_a_listed_user_acts_as_them_and_to_an_unlisted_one_is_refused() {
    let (server, ada, _) = ada_and_grace_on_one_device().await;
    let mallory = server.user("Mallory", &[]).await;

    switch(&server, &ada.token, ada.id).await.unwrap();
    let after_switch = who_am_i(&server, &ada.token).await.unwrap();
    let unlisted = switch(&server, &ada.token, mallory.id).await;

    assert_eq!(acting_as(&after_switch), Some(ada.id));
    assert_refused(
        unlisted,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_USER_NOT_FOUND,
    );
    let me = who_am_i(&server, &ada.token).await.unwrap();
    assert_eq!(acting_as(&me), Some(ada.id), "a refused switch switched");
}

#[tokio::test]
async fn switching_to_a_user_leaves_them_without_recent_authentication() {
    let (server, ada, _) = ada_and_grace_on_one_device().await;

    switch(&server, &ada.token, ada.id).await.unwrap();
    let changed = user_service(&server, &ada.token)
        .change_password(ChangePasswordRequest {
            new_password: "another long password".to_owned(),
            ..Default::default()
        })
        .await;

    assert_refused(
        changed,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_RECENT_AUTHENTICATION_REQUIRED,
    );
}

#[tokio::test]
async fn switching_ends_the_streams_the_device_opened_as_the_user_it_acted_as() {
    let (server, ada, _) = ada_and_grace_on_one_device().await;
    let mut as_grace = subscribe(&server.client_config_as(&ada.token), &[]).await;

    switch(&server, &ada.token, ada.id).await.unwrap();

    assert!(
        matches!(as_grace.message().await, Ok(None)),
        "the stream is still open"
    );
}

#[tokio::test]
async fn revoking_one_user_leaves_the_others_signed_in_and_revoking_the_last_signs_the_device_out()
{
    let (server, ada, grace) = ada_and_grace_on_one_device().await;
    let device = who_am_i(&server, &ada.token).await.unwrap().device.id;

    let as_device = server.client_config_as(&ada.token);
    revoke(as_device.clone(), grace.id, device).await.unwrap();
    let after_one = who_am_i(&server, &ada.token).await.unwrap();
    let acting_as_none = user_service(&server, &ada.token)
        .list_devices(ListDevicesRequest::default())
        .await;
    switch(&server, &ada.token, ada.id).await.unwrap();
    revoke(as_device, ada.id, device).await.unwrap();
    let after_last = who_am_i(&server, &ada.token).await;

    assert_eq!(acting_as(&after_one), None);
    assert_eq!(device_users(&after_one), [(ada.id, "Ada")]);
    assert_refused(
        acting_as_none,
        ErrorCode::Unauthenticated,
        RefusalReason::REFUSAL_REASON_SIGN_IN_REQUIRED,
    );
    assert_refused(
        after_last,
        ErrorCode::Unauthenticated,
        RefusalReason::REFUSAL_REASON_SIGN_IN_REQUIRED,
    );
}

#[tokio::test]
async fn an_admin_revoking_one_user_from_a_device_leaves_it_acting_as_the_other() {
    let (server, ada, grace) = ada_and_grace_on_one_device().await;
    let device = who_am_i(&server, &ada.token).await.unwrap().device.id;
    switch(&server, &ada.token, ada.id).await.unwrap();

    revoke(server.client_config(), grace.id, device)
        .await
        .unwrap();
    let me = who_am_i(&server, &ada.token).await.unwrap();

    assert_eq!(acting_as(&me), Some(ada.id));
    assert_eq!(device_users(&me), [(ada.id, "Ada")]);
}
