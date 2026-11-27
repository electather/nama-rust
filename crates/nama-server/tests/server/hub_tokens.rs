//! The web app at the Hub trading a token the Hub signed for one of the Account's Users here for a
//! Device with a bearer token (ADR 0105), here signed by an in-process fake Hub.

use std::time::Duration;

use crate::support::{fake_hub, log::Log, refused, server};

use connectrpc::{ConnectError, ErrorCode, client::HttpClient};
use fake_hub::{FakeHub, SERVER_ID, in_5_minutes};
use nama_proto::nama::v1::{
    GetServerLinkRequest, ListDevicesRequest, RefusalReason, RevokeDeviceRequest,
    ServerServiceClient, SetSwitchPinRequest, SignInServiceClient, SignInWithHubTokenRequest,
    SignInWithHubTokenResponse, StartServerLinkRequest as Start, UserServiceClient, WhoAmIRequest,
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
    // Signed `ago` seconds after signing in, both by the Hub's clock, which reads `at` seconds
    // from the Server's now: ahead for a Hub clock running fast, behind for a token kept a while.
    let signed_in = |ago: u64, at: i64| {
        let iat = (in_5_minutes() - 300).saturating_add_signed(at);
        hub.signed(&json!({
            "sub": grace.to_string(), "aud": SERVER_ID, "exp": in_5_minutes(),
            "jti": format!("{ago}s ago, at {at}s"), "iat": iat, "auth_time": iat - ago,
        }))
    };
    let just_now = trade(&server, &signed_in(5, 0)).await.unwrap().token;
    let eleven_minutes_ago = trade(&server, &signed_in(660, 0)).await.unwrap().token;
    let by_a_clock_an_hour_ahead = trade(&server, &signed_in(660, 3600)).await.unwrap().token;
    let kept_four_minutes_after_six = trade(&server, &signed_in(400, -240)).await.unwrap().token;
    let unsaid = hub.signed(&json!({
        "sub": grace.to_string(), "aud": SERVER_ID, "exp": in_5_minutes(), "jti": "unsaid",
    }));
    let unsaid = trade(&server, &unsaid).await.unwrap().token;

    set_switch_pin(&server, &just_now).await.unwrap();
    for lacking in [
        eleven_minutes_ago,
        by_a_clock_an_hour_ahead,
        kept_four_minutes_after_six,
        unsaid,
    ] {
        assert_refused(
            set_switch_pin(&server, &lacking).await,
            ErrorCode::FailedPrecondition,
            RefusalReason::REFUSAL_REASON_RECENT_AUTHENTICATION_REQUIRED,
        );
    }
}

/// Links `server` as [`FakeHub::link`] does, but with a token it can't check, so it keeps a Link
/// that doesn't know the Server's id, as the v0.2.0 Hub Plugin made them.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn link_without_server_id(hub: &FakeHub, server: &TestServer) {
    hub.expire_link_tokens();
    let settings = ServerServiceClient::new(HttpClient::plaintext(), server.client_config());
    settings.start_server_link(Start::default()).await.unwrap();
    hub.enter_code();
    let unchecked = settings.get_server_link(GetServerLinkRequest::default());
    unchecked.await.unwrap_err();
}

/// A token the Hub signed for `user` on [`SERVER_ID`], to trade once as `jti`.
fn for_this_server(hub: &FakeHub, user: i64, jti: &str) -> String {
    hub.signed(&json!({
        "sub": user.to_string(), "aud": SERVER_ID, "exp": in_5_minutes(), "jti": jti,
    }))
}

#[tokio::test]
async fn a_server_linked_without_its_id_learns_it_from_the_hub_once_started_and_takes_hub_tokens() {
    let hub = FakeHub::start().await;
    let server = TestServer::builder().config(hub.config()).start().await;
    link_without_server_id(&hub, &server).await;
    let grace = server.user_with_default_permissions("Grace").await.id;
    assert_refused(
        trade(&server, &for_this_server(&hub, grace, "before")).await,
        ErrorCode::Unauthenticated,
        RefusalReason::REFUSAL_REASON_HUB_TOKEN_REFUSED,
    );

    // Starting reports the Server Addresses, which the Hub answers with the Server's id.
    let upgraded = TestServer::builder()
        .config(hub.config())
        .restart(server)
        .await;

    let traded = async {
        let mut attempts = 0;
        let mut token = || {
            attempts += 1;
            for_this_server(&hub, grace, &format!("after {attempts}"))
        };
        while trade(&upgraded, &token()).await.is_err() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    };
    tokio::time::timeout(server::WAIT, traded).await.unwrap();
}

#[tokio::test]
async fn a_server_that_knows_its_id_keeps_it_when_the_hub_names_another() {
    let log = Log::global();
    let (server, hub) = FakeHub::linked_server().await;
    let grace = server.user_with_default_permissions("Grace").await.id;
    hub.sign_in_for("another server");

    let restarted = TestServer::builder()
        .config(hub.config())
        .restart(server)
        .await;

    log.wait_for(
        &["the Hub named the Server by an id its Link doesn't keep"],
        1,
    )
    .await;
    trade(&restarted, &for_this_server(&hub, grace, "kept"))
        .await
        .unwrap();
}
