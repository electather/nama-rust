//! The web app at the Hub trading a token the Hub signed for one of the Account's Users here for a
//! Device with a bearer token (ADR 0105), here signed by an in-process fake Hub.

#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/fake_hub.rs"]
mod fake_hub;
#[path = "support/refused.rs"]
mod refused;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use connectrpc::{ConnectError, ErrorCode, client::HttpClient};
use fake_hub::{FakeHub, SERVER_ID, in_5_minutes};
use nama_proto::nama::v1::{
    ListDevicesRequest, RefusalReason, RevokeDeviceRequest, SignInServiceClient,
    SignInWithHubTokenRequest, SignInWithHubTokenResponse, UserServiceClient, WhoAmIRequest,
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
async fn a_hub_token_for_another_server_expired_unnamed_or_used_twice_is_refused() {
    let (server, hub) = FakeHub::linked_server().await;
    let grace = server.user_with_default_permissions("Grace").await.id;
    let token = |aud: &str, exp: u64, jti: Option<&str>| {
        hub.signed(&json!({ "sub": grace.to_string(), "aud": aud, "exp": exp, "jti": jti }))
    };
    let once = token(SERVER_ID, in_5_minutes(), Some("once"));
    trade(&server, &once).await.unwrap();

    for refused in [
        once,
        token("another server", in_5_minutes(), Some("other")),
        token(SERVER_ID, in_5_minutes() - 3600, Some("expired")),
        token(SERVER_ID, in_5_minutes(), None),
    ] {
        assert_refused(
            trade(&server, &refused).await,
            ErrorCode::Unauthenticated,
            RefusalReason::REFUSAL_REASON_HUB_TOKEN_REFUSED,
        );
    }
}
