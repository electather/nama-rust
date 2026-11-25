//! A Device signed in as several Users (ADR 0106): adding a User with a second sign-in, listing
//! its Users, switching between them, guarding a switch with a Switch PIN and signing one out,
//! through the generated clients, over a real socket and against a real SQLite database.

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
    ApproveSignInCodeRequest, ChangePasswordRequest, ClearSwitchPinRequest, DeleteUserRequest,
    ListDevicesRequest, PollSignInCodeRequest, ReauthenticateRequest, RefusalReason,
    RevokeDeviceRequest, SessionDelivery, SetSwitchPinRequest, SignInRequest, SignInServiceClient,
    StartSignInCodeRequest, SwitchUserRequest, User, UserServiceClient, WhoAmIRequest,
    WhoAmIResponse, reauthenticate_request::Proof,
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
    switch_with_pin(server, token, user, "").await
}

async fn switch_with_pin(
    server: &TestServer,
    token: &str,
    user: i64,
    switch_pin: &str,
) -> Result<(), ConnectError> {
    sign_in_service(server, token)
        .switch_user(SwitchUserRequest {
            user_id: user,
            switch_pin: switch_pin.to_owned(),
            ..Default::default()
        })
        .await?;
    Ok(())
}

/// Sets the Switch PIN of the User the Device holding the session `token` acts as.
async fn set_switch_pin(
    server: &TestServer,
    token: &str,
    switch_pin: &str,
) -> Result<(), ConnectError> {
    sign_in_service(server, token)
        .set_switch_pin(SetSwitchPinRequest {
            switch_pin: switch_pin.to_owned(),
            ..Default::default()
        })
        .await?;
    Ok(())
}

/// Clears the Switch PIN of the User `user`, or of the caller's own User if `None`, as whoever
/// `who` calls the Server as.
async fn clear_switch_pin(who: ClientConfig, user: Option<i64>) -> Result<(), ConnectError> {
    SignInServiceClient::new(HttpClient::plaintext(), who)
        .clear_switch_pin(ClearSwitchPinRequest {
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
async fn signing_in_again_as_a_user_already_on_the_device_lists_them_once_and_acts_as_them() {
    let (server, ada, grace) = ada_and_grace_on_one_device().await;

    let again = sign_in_on(&server, &ada.token, "Ada").await;

    assert_eq!(again, ada.token, "the Device's session changed");
    let me = who_am_i(&server, &ada.token).await.unwrap();
    assert_eq!(acting_as(&me), Some(ada.id));
    assert_eq!(device_users(&me), [(ada.id, "Ada"), (grace.id, "Grace")]);
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

#[tokio::test]
async fn a_second_sign_in_ends_the_streams_the_device_opened_as_the_user_it_acted_as() {
    let server = TestServer::builder().start().await;
    let ada = server.user("Ada", &[]).await;
    server.user("Grace", &[]).await;
    let mut as_ada = subscribe(&server.client_config_as(&ada.token), &[]).await;

    sign_in_on(&server, &ada.token, "Grace").await;

    assert!(
        matches!(as_ada.message().await, Ok(None)),
        "the stream is still open"
    );
}

#[tokio::test]
async fn deleting_a_user_signs_them_out_of_a_shared_device_and_leaves_the_others() {
    let (server, ada, grace) = ada_and_grace_on_one_device().await;

    UserServiceClient::new(HttpClient::plaintext(), server.client_config())
        .delete_user(DeleteUserRequest {
            user_id: grace.id,
            ..Default::default()
        })
        .await
        .unwrap();
    let me = who_am_i(&server, &ada.token).await.unwrap();

    assert_eq!(acting_as(&me), None);
    assert_eq!(device_users(&me), [(ada.id, "Ada")]);
}

/// Ada and Grace on one Device, Grace with the Switch PIN [`GRACES_PIN`], acting as Ada.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn grace_with_a_switch_pin() -> (TestServer, SignedIn, SignedIn) {
    let (server, ada, grace) = ada_and_grace_on_one_device().await;
    set_switch_pin(&server, &ada.token, GRACES_PIN)
        .await
        .unwrap();
    switch(&server, &ada.token, ada.id).await.unwrap();
    (server, ada, grace)
}

const GRACES_PIN: &str = "4821";

/// The tries a wrong Switch PIN's refusal says are left.
#[expect(
    clippy::indexing_slicing,
    reason = "a test helper fails its test by panicking"
)]
fn tries_left(refused: Result<(), ConnectError>) -> String {
    let refusal = assert_refused(
        refused,
        ErrorCode::PermissionDenied,
        RefusalReason::REFUSAL_REASON_WRONG_SWITCH_PIN,
    );
    assert_eq!(refusal.field, "switch_pin");
    refusal.params["triesLeft"].clone()
}

#[tokio::test]
async fn switching_to_a_user_with_a_switch_pin_needs_it_and_refuses_a_wrong_one() {
    let (server, ada, grace) = grace_with_a_switch_pin().await;

    let without = switch(&server, &ada.token, grace.id).await;
    let wrong = switch_with_pin(&server, &ada.token, grace.id, "0000").await;
    let acting_after_refusals = acting_as(&who_am_i(&server, &ada.token).await.unwrap());
    switch_with_pin(&server, &ada.token, grace.id, GRACES_PIN)
        .await
        .unwrap();
    let to_ada_without_one = switch(&server, &ada.token, ada.id).await;

    let refusal = assert_refused(
        without,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
    );
    assert_eq!(refusal.field, "switch_pin");
    assert_eq!(tries_left(wrong), "4");
    assert_eq!(
        acting_after_refusals,
        Some(ada.id),
        "a refused switch switched"
    );
    to_ada_without_one.expect("Grace's Switch PIN guards Ada");
}

#[tokio::test]
async fn changing_a_switch_pin_replaces_the_old_one() {
    let (server, ada, grace) = ada_and_grace_on_one_device().await;
    set_switch_pin(&server, &ada.token, "1111").await.unwrap();
    set_switch_pin(&server, &ada.token, "222222").await.unwrap();
    switch(&server, &ada.token, ada.id).await.unwrap();

    let old = switch_with_pin(&server, &ada.token, grace.id, "1111").await;
    let new = switch_with_pin(&server, &ada.token, grace.id, "222222").await;

    assert_eq!(tries_left(old), "4");
    new.unwrap();
}

#[tokio::test]
async fn a_switch_pin_that_isnt_4_to_6_digits_is_refused() {
    let (server, ada, _) = ada_and_grace_on_one_device().await;

    for pin in ["", "123", "1234567", "12a4", "１２３４"] {
        let refusal = assert_refused(
            set_switch_pin(&server, &ada.token, pin).await,
            ErrorCode::InvalidArgument,
            RefusalReason::REFUSAL_REASON_SWITCH_PIN_INVALID,
        );
        assert_eq!(refusal.field, "switch_pin", "{pin}");
    }
}

#[tokio::test]
async fn setting_a_switch_pin_needs_recent_authentication() {
    let (server, ada, grace) = ada_and_grace_on_one_device().await;
    switch(&server, &ada.token, ada.id).await.unwrap();
    switch(&server, &ada.token, grace.id).await.unwrap();

    let set = set_switch_pin(&server, &ada.token, GRACES_PIN).await;

    assert_refused(
        set,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_RECENT_AUTHENTICATION_REQUIRED,
    );
}

#[tokio::test]
async fn a_user_who_clears_their_switch_pin_is_switched_to_without_one() {
    let (server, ada, grace) = ada_and_grace_on_one_device().await;
    set_switch_pin(&server, &ada.token, GRACES_PIN)
        .await
        .unwrap();

    clear_switch_pin(server.client_config_as(&ada.token), None)
        .await
        .unwrap();
    switch(&server, &ada.token, ada.id).await.unwrap();

    switch(&server, &ada.token, grace.id).await.unwrap();
}

#[tokio::test]
async fn a_user_manager_clears_another_users_switch_pin_and_another_user_cant() {
    let (server, ada, grace) = grace_with_a_switch_pin().await;
    let mallory = server.user("Mallory", &[]).await;

    let by_mallory = clear_switch_pin(mallory.client_config(), Some(grace.id)).await;
    let still_needed = switch(&server, &ada.token, grace.id).await;
    clear_switch_pin(server.client_config(), Some(grace.id))
        .await
        .unwrap();

    assert_refused(
        by_mallory,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_USER_NOT_FOUND,
    );
    assert_refused(
        still_needed,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
    );
    switch(&server, &ada.token, grace.id).await.unwrap();
}

#[tokio::test]
async fn five_wrong_switch_pins_in_a_row_sign_the_user_out_of_the_device_until_they_sign_in() {
    let (server, ada, grace) = grace_with_a_switch_pin().await;

    let mut left = Vec::new();
    for _ in 0..5 {
        left.push(tries_left(
            switch_with_pin(&server, &ada.token, grace.id, "0000").await,
        ));
    }
    let after = who_am_i(&server, &ada.token).await.unwrap();
    let with_the_right_one = switch_with_pin(&server, &ada.token, grace.id, GRACES_PIN).await;
    sign_in_on(&server, &ada.token, "Grace").await;
    switch(&server, &ada.token, ada.id).await.unwrap();
    let wrong_after_signing_in = switch_with_pin(&server, &ada.token, grace.id, "0000").await;

    assert_eq!(left, ["4", "3", "2", "1", "0"]);
    assert_eq!(acting_as(&after), Some(ada.id));
    assert_eq!(device_users(&after), [(ada.id, "Ada")]);
    assert_refused(
        with_the_right_one,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_USER_NOT_FOUND,
    );
    assert_eq!(tries_left(wrong_after_signing_in), "4");
}

#[tokio::test]
async fn a_right_switch_pin_starts_the_count_of_wrong_ones_again() {
    let (server, ada, grace) = grace_with_a_switch_pin().await;
    for _ in 0..4 {
        let _wrong = switch_with_pin(&server, &ada.token, grace.id, "0000").await;
    }

    switch_with_pin(&server, &ada.token, grace.id, GRACES_PIN)
        .await
        .unwrap();
    switch(&server, &ada.token, ada.id).await.unwrap();
    let wrong = switch_with_pin(&server, &ada.token, grace.id, "0000").await;

    assert_eq!(tries_left(wrong), "4");
}

#[tokio::test]
async fn a_switch_pin_neither_signs_a_device_in_nor_reauthenticates() {
    let (server, ada, grace) = grace_with_a_switch_pin().await;
    switch_with_pin(&server, &ada.token, grace.id, GRACES_PIN)
        .await
        .unwrap();

    let signed_in =
        SignInServiceClient::new(HttpClient::plaintext(), server.anonymous_client_config())
            .sign_in(SignInRequest {
                name: "Grace".to_owned(),
                password: GRACES_PIN.to_owned(),
                delivery: SessionDelivery::SESSION_DELIVERY_TOKEN.into(),
                ..Default::default()
            })
            .await;
    let reauthenticated = sign_in_service(&server, &ada.token)
        .reauthenticate(ReauthenticateRequest {
            proof: Some(Proof::Password(GRACES_PIN.to_owned())),
            ..Default::default()
        })
        .await;

    assert_refused(
        signed_in,
        ErrorCode::Unauthenticated,
        RefusalReason::REFUSAL_REASON_WRONG_NAME_OR_PASSWORD,
    );
    assert_refused(
        reauthenticated,
        ErrorCode::Unauthenticated,
        RefusalReason::REFUSAL_REASON_WRONG_PASSWORD,
    );
}

#[tokio::test]
async fn a_switch_pin_is_stored_only_as_an_argon2id_hash() {
    let (server, ..) = grace_with_a_switch_pin().await;

    let stored: Vec<String> = sqlx::query_scalar("SELECT credential FROM switch_pins")
        .fetch_all(server.db().reader())
        .await
        .unwrap();

    assert_eq!(stored.len(), 1);
    assert!(stored[0].starts_with("$argon2id$"), "{stored:?}");
    assert!(!stored[0].contains(GRACES_PIN), "{stored:?}");
}
