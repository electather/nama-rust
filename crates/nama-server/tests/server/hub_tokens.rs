//! The web app at the Hub trading a token the Hub signed for one of the Account's Users here for a
//! Device with a bearer token (ADR 0105), here signed by an in-process fake Hub.

use crate::support::{fake_hub, refused, server};

use connectrpc::{ConnectError, ErrorCode, client::HttpClient};
use fake_hub::{FakeHub, SERVER_ID, in_5_minutes};
use nama_proto::nama::v1::{
    ListDevicesRequest, RefusalReason, RevokeDeviceRequest, SetSwitchPinRequest,
    SignInServiceClient, SignInWithHubTokenRequest, SignInWithHubTokenResponse, UserServiceClient,
    WhoAmIRequest,
};
use refused::assert_refused;
use serde_json::json;
use server::TestServer;

async fn trade(
    server: &TestServer,
    hub_token: &str,
) -> Result<SignInWithHubTokenResponse, ConnectError> {
    Ok(
        SignInServiceClient::new(HttpClient::plaintext(), server.anonymous_client_config())
            .sign_in_with_hub_token(SignInWithHubTokenRequest {
                hub_token: hub_token.to_owned(),
                device_name: "Hub".to_owned(),
                ..Default::default()
            })
            .await?
            .into_owned(),
    )
}

/// Whom the Device holding `token` acts as.
async fn who_am_i(server: &TestServer, token: &str) -> Result<i64, ConnectError> {
    let who = SignInServiceClient::new(HttpClient::plaintext(), server.client_config_as(token))
        .who_am_i(WhoAmIRequest::default())
        .await?;
    Ok(who.into_owned().user.into_option().unwrap_or_default().id)
}

#[tokio::test]
async fn a_hub_token_signs_in_a_device_its_user_lists_revokes_and_keeps_while_the_hub_is_down() {
    let (server, hub) = FakeHub::linked_server().await;
    let grace = server.user_with_default_permissions("Grace").await;
    let hub_token = hub.signed(&json!({
        "sub": grace.id.to_string(), "aud": SERVER_ID, "exp": in_5_minutes(), "jti": "once",
    }));

    let traded = trade(&server, &hub_token).await.unwrap();

    let device = traded.device.into_option().unwrap();
    assert_eq!(device.name, "Hub");
    let devices = UserServiceClient::new(HttpClient::plaintext(), grace.client_config());
    let listed = devices
        .list_devices(ListDevicesRequest::default())
        .await
        .unwrap()
        .into_owned()
        .devices;
    assert!(
        listed.iter().any(|listed| listed.id == device.id),
        "{listed:?}"
    );
    hub.stop();
    assert_eq!(who_am_i(&server, &traded.token).await.unwrap(), grace.id);
    devices
        .revoke_device(RevokeDeviceRequest {
            device_id: device.id,
            ..Default::default()
        })
        .await
        .unwrap();
    let revoked = who_am_i(&server, &traded.token).await.unwrap_err();
    assert_eq!(revoked.code, ErrorCode::Unauthenticated, "{revoked:?}");
}

#[tokio::test]
async fn a_hub_token_for_another_server_expired_unnamed_used_twice_or_forged_is_refused() {
    let (server, hub) = FakeHub::linked_server().await;
    let grace = server.user_with_default_permissions("Grace").await.id;
    let token = |aud: &str, exp: u64, jti: Option<&str>| {
        hub.signed(&json!({ "sub": grace.to_string(), "aud": aud, "exp": exp, "jti": jti }))
    };
    let once = token(SERVER_ID, in_5_minutes(), Some("once"));
    trade(&server, &once).await.unwrap();
    // Unused claims under another token's signature, so only the signature refuses it.
    let unused = token(SERVER_ID, in_5_minutes(), Some("forged"));
    let (unused_claims, _) = unused.rsplit_once('.').unwrap();
    let (_, once_signature) = once.rsplit_once('.').unwrap();
    let forged = format!("{unused_claims}.{once_signature}");

    for refused in [
        once,
        token("another server", in_5_minutes(), Some("other")),
        token(SERVER_ID, in_5_minutes() - 3600, Some("expired")),
        token(SERVER_ID, in_5_minutes(), None),
        forged,
    ] {
        assert_refused(
            trade(&server, &refused).await,
            ErrorCode::Unauthenticated,
            RefusalReason::REFUSAL_REASON_HUB_TOKEN_REFUSED,
        );
    }
}

/// Sets a Switch PIN, a change that needs Recent Authentication, as the Device holding `token`.
async fn set_switch_pin(server: &TestServer, token: &str) -> Result<(), ConnectError> {
    SignInServiceClient::new(HttpClient::plaintext(), server.client_config_as(token))
        .set_switch_pin(SetSwitchPinRequest {
            switch_pin: "2468".to_owned(),
            ..Default::default()
        })
        .await?;
    Ok(())
}

#[tokio::test]
async fn a_device_traded_within_ten_minutes_of_its_accounts_hub_sign_in_has_recent_authentication()
{
    let (server, hub) = FakeHub::linked_server().await;
    let grace = server.user_with_default_permissions("Grace").await.id;
    let signed_in_ago = |secs: u64| {
        let auth_time = in_5_minutes() - 300 - secs;
        hub.signed(&json!({
            "sub": grace.to_string(), "aud": SERVER_ID, "exp": in_5_minutes(),
            "jti": format!("signed in {secs}s ago"), "auth_time": auth_time,
        }))
    };
    let just_now = trade(&server, &signed_in_ago(5)).await.unwrap().token;
    let eleven_minutes_ago = trade(&server, &signed_in_ago(660)).await.unwrap().token;
    let unsaid = hub.signed(&json!({
        "sub": grace.to_string(), "aud": SERVER_ID, "exp": in_5_minutes(), "jti": "unsaid",
    }));
    let unsaid = trade(&server, &unsaid).await.unwrap().token;

    set_switch_pin(&server, &just_now).await.unwrap();
    for lacking in [eleven_minutes_ago, unsaid] {
        assert_refused(
            set_switch_pin(&server, &lacking).await,
            ErrorCode::FailedPrecondition,
            RefusalReason::REFUSAL_REASON_RECENT_AUTHENTICATION_REQUIRED,
        );
    }
}
