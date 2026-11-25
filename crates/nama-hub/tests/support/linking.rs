//! Linking through the Hub's API, as the tests of Links and of Users created from Accounts do:
//! Accounts, their sessions, Linked Servers and Linked Users.

use axum::{
    body::{Body, Bytes},
    http::HeaderMap,
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use connectrpc::client::{ClientTransport as _, HttpClient, full_body};
use nama_hub_proto::nama::hub::v1::{
    AccountServiceClient, ApproveServerLinkRequest, ApproveUserLinkRequest, CreateAccountRequest,
    GetUserLinkRequest, LinkServiceClient, LinkedServerServiceClient, ListLinkedServersRequest,
    ListLinkedUsersRequest, PollServerLinkRequest, PollServerLinkResponse, SendSignInLinkRequest,
    ServerLinkServiceClient, SignInWithLinkRequest, StartServerLinkRequest,
    StartServerLinkResponse, StartUserLinkRequest, UnlinkLinkedUserRequest, UnlinkUserRequest,
};
use ring::signature::{ED25519, UnparsedPublicKey};
use serde_json::Value;
use sqlx::PgPool;

use crate::{
    hub::TestHub,
    mail::{Inbox, fake_mail},
};

/// A Hub that emails Sign-in Links through the fake, and the fake's inbox.
pub(crate) async fn hub_with_mail(db: PgPool) -> (TestHub, Inbox) {
    let (mail, inbox) = fake_mail();
    (TestHub::start(db, Some(mail)).await, inbox)
}

pub(crate) fn accounts(hub: &TestHub) -> AccountServiceClient<HttpClient> {
    AccountServiceClient::new(HttpClient::plaintext(), hub.client_config())
}

/// The session token a response's `Set-Cookie` carries.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
pub(crate) fn session_token(headers: &HeaderMap) -> String {
    let cookie = headers.get("set-cookie").unwrap().to_str().unwrap();
    let token = cookie.split(';').next().unwrap();
    token.strip_prefix("nama_hub_session=").unwrap().to_owned()
}

/// Creates an Account with `email`, and returns its session token.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
pub(crate) async fn create(hub: &TestHub, email: &str) -> String {
    let created = accounts(hub)
        .create_account(CreateAccountRequest {
            email: email.to_owned(),
            password: "correct horse battery".to_owned(),
            ..Default::default()
        })
        .await
        .unwrap();
    session_token(created.headers())
}

/// Creates an Account with `email` and proves its email with the Sign-in Link it gets, and returns
/// the token of the session the link signed in.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
pub(crate) async fn create_proven(hub: &TestHub, inbox: &mut Inbox, email: &str) -> String {
    create(hub, email).await;
    accounts(hub)
        .send_sign_in_link(SendSignInLinkRequest {
            email: email.to_owned(),
            continue_to: "/".to_owned(),
            ..Default::default()
        })
        .await
        .unwrap();
    let link = inbox.next_link(email).await;
    let used = accounts(hub)
        .sign_in_with_link(SignInWithLinkRequest {
            token: link,
            ..Default::default()
        })
        .await
        .unwrap();
    session_token(used.headers())
}

/// The Hub's pages, signed in with the session `token`.
pub(crate) fn links(hub: &TestHub, token: &str) -> LinkServiceClient<HttpClient> {
    let config = hub
        .client_config()
        .with_default_header("cookie", format!("nama_hub_session={token}"));
    LinkServiceClient::new(HttpClient::plaintext(), config)
}

/// A Server not yet Linked.
pub(crate) fn server(hub: &TestHub) -> ServerLinkServiceClient<HttpClient> {
    ServerLinkServiceClient::new(HttpClient::plaintext(), hub.client_config())
}

/// A Server presenting `credential`.
pub(crate) fn linked_server(
    hub: &TestHub,
    credential: &str,
) -> LinkedServerServiceClient<HttpClient> {
    let config = hub
        .client_config()
        .with_default_header("authorization", format!("Bearer {credential}"));
    LinkedServerServiceClient::new(HttpClient::plaintext(), config)
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
pub(crate) async fn start(hub: &TestHub, name: &str) -> StartServerLinkResponse {
    server(hub)
        .start_server_link(StartServerLinkRequest {
            name: name.to_owned(),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
}

pub(crate) async fn poll(
    hub: &TestHub,
    device_code: &str,
) -> Result<PollServerLinkResponse, connectrpc::ConnectError> {
    Ok(server(hub)
        .poll_server_link(PollServerLinkRequest {
            device_code: device_code.to_owned(),
            ..Default::default()
        })
        .await?
        .into_owned())
}

/// Approves `code` as the Account signed in with `token`, answering where to send them back to.
pub(crate) async fn approve(
    hub: &TestHub,
    token: &str,
    code: &str,
) -> Result<String, connectrpc::ConnectError> {
    Ok(links(hub, token)
        .approve_server_link(ApproveServerLinkRequest {
            code: code.to_owned(),
            ..Default::default()
        })
        .await?
        .into_owned()
        .return_url)
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
pub(crate) async fn server_names(hub: &TestHub, token: &str) -> Vec<String> {
    let listed = links(hub, token)
        .list_linked_servers(ListLinkedServersRequest::default())
        .await
        .unwrap()
        .into_owned();
    listed
        .servers
        .into_iter()
        .map(|server| server.name)
        .collect()
}

#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "a test helper fails its test by panicking"
)]
/// The claims of `token` once its signature checks out against the key its header names in the
/// Hub's published key set.
pub(crate) async fn verified_claims(hub: &TestHub, token: &str) -> Value {
    let request = axum::http::Request::get(format!(
        "{}.well-known/jwks.json",
        hub.client_config().base_uri()
    ))
    .body(full_body(Bytes::new()))
    .unwrap();
    let response = HttpClient::plaintext().send(request).await.unwrap();
    let jwks = axum::body::to_bytes(Body::new(response.into_body()), 1 << 16)
        .await
        .unwrap();
    let jwks: Value = serde_json::from_slice(&jwks).unwrap();
    let parts: Vec<&str> = token.split('.').collect();
    let decoded = |part: &str| -> Value {
        serde_json::from_slice(&URL_SAFE_NO_PAD.decode(part).unwrap()).unwrap()
    };
    let header = decoded(parts[0]);
    let key = jwks["keys"]
        .as_array()
        .unwrap()
        .iter()
        .find(|key| key["kid"] == header["kid"])
        .unwrap();
    let public = URL_SAFE_NO_PAD.decode(key["x"].as_str().unwrap()).unwrap();
    UnparsedPublicKey::new(&ED25519, public)
        .verify(
            format!("{}.{}", parts[0], parts[1]).as_bytes(),
            &URL_SAFE_NO_PAD.decode(parts[2]).unwrap(),
        )
        .unwrap();
    assert_eq!(header["alg"], "EdDSA");
    decoded(parts[1])
}

/// Links a Server named `name` to the Account signed in with `token`, and returns its credential.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
pub(crate) async fn linked(hub: &TestHub, token: &str, name: &str) -> String {
    let started = start(hub, name).await;
    approve(hub, token, &started.user_code).await.unwrap();
    poll(hub, &started.device_code).await.unwrap().credential
}

/// Starts Linking the User `user_id` of the Server presenting `credential`, and returns their code.
pub(crate) async fn start_user(
    hub: &TestHub,
    credential: &str,
    user_id: i64,
) -> Result<String, connectrpc::ConnectError> {
    Ok(linked_server(hub, credential)
        .start_user_link(StartUserLinkRequest {
            user_id,
            name: "Grace".to_owned(),
            ..Default::default()
        })
        .await?
        .into_owned()
        .user_code)
}

pub(crate) async fn approve_user(
    hub: &TestHub,
    token: &str,
    code: &str,
) -> Result<String, connectrpc::ConnectError> {
    let approved = links(hub, token)
        .approve_user_link(ApproveUserLinkRequest {
            code: code.to_owned(),
            ..Default::default()
        })
        .await?
        .into_owned();
    Ok(approved
        .user
        .into_option()
        .map_or_else(String::new, |user| user.id))
}

/// The email of the Account the Server presenting `credential` says its User `user_id` is Linked
/// to, empty while they aren't.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
pub(crate) async fn user_account(hub: &TestHub, credential: &str, user_id: i64) -> String {
    linked_server(hub, credential)
        .get_user_link(GetUserLinkRequest {
            user_id,
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .account
}

/// Each of the signed-in Account's Linked Users, as `<User> on <Server>`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
pub(crate) async fn linked_users(hub: &TestHub, token: &str) -> Vec<String> {
    let listed = links(hub, token)
        .list_linked_users(ListLinkedUsersRequest::default())
        .await
        .unwrap()
        .into_owned();
    listed
        .users
        .into_iter()
        .map(|user| format!("{} on {}", user.name, user.server_name))
        .collect()
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
pub(crate) async fn unlink_user(hub: &TestHub, credential: &str, user_id: i64) {
    linked_server(hub, credential)
        .unlink_user(UnlinkUserRequest {
            user_id,
            ..Default::default()
        })
        .await
        .unwrap();
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
pub(crate) async fn unlink_linked_user(hub: &TestHub, token: &str, id: &str) {
    links(hub, token)
        .unlink_linked_user(UnlinkLinkedUserRequest {
            id: id.to_owned(),
            ..Default::default()
        })
        .await
        .unwrap();
}
