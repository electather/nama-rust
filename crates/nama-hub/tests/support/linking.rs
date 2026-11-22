//! Linking a Server through the API, for tests that need a Linked Server and the Account that owns
//! it.

use axum::http::HeaderMap;
use connectrpc::client::HttpClient;
use nama_hub_proto::nama::hub::v1::{
    AccountServiceClient, ApproveServerLinkRequest, CreateAccountRequest, LinkServiceClient,
    LinkedServerServiceClient, PollServerLinkRequest, PollServerLinkResponse,
    SendSignInLinkRequest, ServerLinkServiceClient, SignInWithLinkRequest, StartServerLinkRequest,
    StartServerLinkResponse,
};
use sqlx::PgPool;

use crate::hub::TestHub;
use crate::mail::{Inbox, fake_mail};

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

pub(crate) async fn approve(
    hub: &TestHub,
    token: &str,
    code: &str,
) -> Result<(), connectrpc::ConnectError> {
    links(hub, token)
        .approve_server_link(ApproveServerLinkRequest {
            code: code.to_owned(),
            ..Default::default()
        })
        .await?;
    Ok(())
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
