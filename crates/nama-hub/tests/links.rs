//! Registering a Server through the device flow, listing an Account's Servers, Unlinking one,
//! what a Linked Server shares, and Linking a Linked Server's Users to Accounts, against a real
//! PostgreSQL.

#[path = "support/hub.rs"]
mod hub;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/mail.rs"]
mod mail;
#[path = "support/refused.rs"]
mod refused;

use axum::{
    body::{Body, Bytes},
    http::{HeaderMap, StatusCode},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use connectrpc::{
    ErrorCode,
    client::{ClientTransport as _, HttpClient, full_body},
};
use hub::TestHub;
use mail::{Inbox, fake_mail};
use nama_hub::diagnostics::Signal;
use nama_hub_proto::nama::hub::v1::{
    AccountServiceClient, ApproveServerLinkRequest, ApproveUserLinkRequest, CreateAccountRequest,
    GetUserLinkRequest, LinkServiceClient, LinkedServerServiceClient, ListLinkedServersRequest,
    ListLinkedUsersRequest, PollServerLinkRequest, PollServerLinkResponse, RefusalReason,
    SendSignInLinkRequest, ServerLinkServiceClient, SignInWithLinkRequest, StartServerLinkRequest,
    StartServerLinkResponse, StartUserLinkRequest, UnlinkLinkedUserRequest, UnlinkServerRequest,
    UnlinkUserRequest,
};
use refused::assert_refused;
use ring::signature::{ED25519, UnparsedPublicKey};
use serde_json::Value;
use sqlx::PgPool;

/// A Hub that emails Sign-in Links through the fake, and the fake's inbox.
async fn hub_with_mail(db: PgPool) -> (TestHub, Inbox) {
    let (mail, inbox) = fake_mail();
    (TestHub::start(db, Some(mail)).await, inbox)
}

fn accounts(hub: &TestHub) -> AccountServiceClient<HttpClient> {
    AccountServiceClient::new(HttpClient::plaintext(), hub.client_config())
}

/// The session token a response's `Set-Cookie` carries.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn session_token(headers: &HeaderMap) -> String {
    let cookie = headers.get("set-cookie").unwrap().to_str().unwrap();
    let token = cookie.split(';').next().unwrap();
    token.strip_prefix("nama_hub_session=").unwrap().to_owned()
}

/// Creates an Account with `email`, and returns its session token.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn create(hub: &TestHub, email: &str) -> String {
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
async fn create_proven(hub: &TestHub, inbox: &mut Inbox, email: &str) -> String {
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
fn links(hub: &TestHub, token: &str) -> LinkServiceClient<HttpClient> {
    let config = hub
        .client_config()
        .with_default_header("cookie", format!("nama_hub_session={token}"));
    LinkServiceClient::new(HttpClient::plaintext(), config)
}

/// A Server not yet Linked.
fn server(hub: &TestHub) -> ServerLinkServiceClient<HttpClient> {
    ServerLinkServiceClient::new(HttpClient::plaintext(), hub.client_config())
}

/// A Server presenting `credential`.
fn linked_server(hub: &TestHub, credential: &str) -> LinkedServerServiceClient<HttpClient> {
    let config = hub
        .client_config()
        .with_default_header("authorization", format!("Bearer {credential}"));
    LinkedServerServiceClient::new(HttpClient::plaintext(), config)
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn start(hub: &TestHub, name: &str) -> StartServerLinkResponse {
    server(hub)
        .start_server_link(StartServerLinkRequest {
            name: name.to_owned(),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
}

async fn poll(
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

async fn approve(hub: &TestHub, token: &str, code: &str) -> Result<(), connectrpc::ConnectError> {
    links(hub, token)
        .approve_server_link(ApproveServerLinkRequest {
            code: code.to_owned(),
            ..Default::default()
        })
        .await?;
    Ok(())
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn server_names(hub: &TestHub, token: &str) -> Vec<String> {
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
async fn verified_claims(hub: &TestHub, token: &str) -> Value {
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

#[sqlx::test(migrations = false)]
async fn entering_the_code_links_the_server_which_collects_its_credential_once(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "Ada@example.com").await;
    let started = start(&hub, "Living room").await;
    assert_eq!(started.expires_in_seconds, 600);
    assert_eq!(started.interval_seconds, 5);
    assert!(!poll(&hub, &started.device_code).await.unwrap().linked);

    // Typed in lower case, without its hyphen.
    approve(
        &hub,
        &ada,
        &started.user_code.replace('-', "").to_lowercase(),
    )
    .await
    .unwrap();

    assert_eq!(server_names(&hub, &ada).await, ["Living room"]);
    let polled = poll(&hub, &started.device_code).await.unwrap();
    assert!(polled.linked);
    let claims = verified_claims(&hub, &polled.token).await;
    assert_eq!(claims["email"], "Ada@example.com");
    assert_eq!(
        claims["exp"].as_u64().unwrap() - claims["iat"].as_u64().unwrap(),
        300
    );
    assert_refused(
        poll(&hub, &started.device_code).await,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_LINK_EXPIRED,
    );
    assert_refused(
        approve(&hub, &ada, &started.user_code).await,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_LINK_CODE_NOT_FOUND,
    );
}

#[sqlx::test(migrations = false)]
async fn unlinking_revokes_the_servers_credential(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let started = start(&hub, "Living room").await;
    approve(&hub, &ada, &started.user_code).await.unwrap();
    let credential = poll(&hub, &started.device_code).await.unwrap().credential;

    linked_server(&hub, &credential)
        .unlink_server(UnlinkServerRequest::default())
        .await
        .unwrap();

    assert_eq!(server_names(&hub, &ada).await, Vec::<String>::new());
    assert_refused(
        linked_server(&hub, &credential)
            .unlink_server(UnlinkServerRequest::default())
            .await,
        ErrorCode::Unauthenticated,
        RefusalReason::REFUSAL_REASON_SERVER_NOT_LINKED,
    );
}

#[sqlx::test(migrations = false)]
async fn an_account_lists_only_its_own_servers(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let bea = create_proven(&hub, &mut inbox, "bea@example.com").await;
    for (token, name) in [(&ada, "Ada's"), (&bea, "Bea's")] {
        let started = start(&hub, name).await;
        approve(&hub, token, &started.user_code).await.unwrap();
    }

    assert_eq!(server_names(&hub, &ada).await, ["Ada's"]);
    assert_eq!(server_names(&hub, &bea).await, ["Bea's"]);
}

#[sqlx::test(migrations = false)]
async fn a_server_unlinking_leaves_another_accounts_servers_linked(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let mut credentials = Vec::new();
    for (email, name) in [("ada@example.com", "Ada's"), ("bea@example.com", "Bea's")] {
        let token = create_proven(&hub, &mut inbox, email).await;
        let started = start(&hub, name).await;
        approve(&hub, &token, &started.user_code).await.unwrap();
        credentials.push((
            token,
            poll(&hub, &started.device_code).await.unwrap().credential,
        ));
    }
    let [(ada, ada_server), (bea, bea_server)] = <[_; 2]>::try_from(credentials).unwrap();

    linked_server(&hub, &ada_server)
        .unlink_server(UnlinkServerRequest::default())
        .await
        .unwrap();

    assert_eq!(server_names(&hub, &ada).await, Vec::<String>::new());
    assert_eq!(server_names(&hub, &bea).await, ["Bea's"]);
    linked_server(&hub, &bea_server)
        .unlink_server(UnlinkServerRequest::default())
        .await
        .unwrap();
}

#[sqlx::test(migrations = false)]
async fn an_account_whose_email_isnt_proven_cant_link_a_server(db: PgPool) {
    let hub = TestHub::start(db, None).await;
    let ada = create(&hub, "ada@example.com").await;
    let started = start(&hub, "Living room").await;

    assert_refused(
        approve(&hub, &ada, &started.user_code).await,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_EMAIL_NOT_PROVEN,
    );
    assert!(!poll(&hub, &started.device_code).await.unwrap().linked);
}

#[sqlx::test(migrations = false)]
async fn linking_needs_a_session_and_a_code_a_server_shows(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;

    assert_refused(
        approve(&hub, "no such session", "BDFH-JKLM").await,
        ErrorCode::Unauthenticated,
        RefusalReason::REFUSAL_REASON_SIGN_IN_REQUIRED,
    );
    let refusal = assert_refused(
        approve(&hub, &ada, "BDFH-JKLM").await,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_LINK_CODE_NOT_FOUND,
    );
    assert_eq!(refusal.field, "code");
    assert_refused(
        linked_server(&hub, "no such credential")
            .unlink_server(UnlinkServerRequest::default())
            .await,
        ErrorCode::Unauthenticated,
        RefusalReason::REFUSAL_REASON_SERVER_NOT_LINKED,
    );
}

#[sqlx::test(migrations = false)]
async fn a_credential_the_hub_cant_check_isnt_called_unlinked(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let started = start(&hub, "Living room").await;
    approve(&hub, &ada, &started.user_code).await.unwrap();
    let credential = poll(&hub, &started.device_code).await.unwrap().credential;
    hub.lose_database().await;

    let failed = linked_server(&hub, &credential)
        .unlink_server(UnlinkServerRequest::default())
        .await
        .unwrap_err();

    // A Server that heard "not Linked" would forget its Link.
    assert_ne!(failed.code, ErrorCode::Unauthenticated, "{failed:?}");
}

/// Links a Server named `name` to the Account signed in with `token`, and returns its credential.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn linked(hub: &TestHub, token: &str, name: &str) -> String {
    let started = start(hub, name).await;
    approve(hub, token, &started.user_code).await.unwrap();
    poll(hub, &started.device_code).await.unwrap().credential
}

/// Starts Linking the User `user_id` of the Server presenting `credential`, and returns their code.
async fn start_user(
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

async fn approve_user(
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
async fn user_account(hub: &TestHub, credential: &str, user_id: i64) -> String {
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
async fn linked_users(hub: &TestHub, token: &str) -> Vec<String> {
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
async fn unlink_user(hub: &TestHub, credential: &str, user_id: i64) {
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
async fn unlink_linked_user(hub: &TestHub, token: &str, id: &str) {
    links(hub, token)
        .unlink_linked_user(UnlinkLinkedUserRequest {
            id: id.to_owned(),
            ..Default::default()
        })
        .await
        .unwrap();
}

#[sqlx::test(migrations = false)]
async fn a_user_who_enters_their_code_is_linked_on_both_sides(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let bea = create_proven(&hub, &mut inbox, "Bea@example.com").await;
    let server = linked(&hub, &ada, "Ada's").await;
    let code = start_user(&hub, &server, 7).await.unwrap();
    assert_eq!(user_account(&hub, &server, 7).await, "");

    approve_user(&hub, &bea, &code.to_lowercase())
        .await
        .unwrap();

    assert_eq!(user_account(&hub, &server, 7).await, "Bea@example.com");
    assert_eq!(linked_users(&hub, &bea).await, ["Grace on Ada's"]);
    assert_eq!(linked_users(&hub, &ada).await, Vec::<String>::new());
    assert_eq!(user_account(&hub, &server, 8).await, "");
    assert_refused(
        approve_user(&hub, &bea, &code).await,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_LINK_CODE_NOT_FOUND,
    );
}

#[sqlx::test(migrations = false)]
async fn an_account_links_users_on_many_servers_but_a_user_links_to_one(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let bea = create_proven(&hub, &mut inbox, "bea@example.com").await;
    let living_room = linked(&hub, &ada, "Living room").await;
    let cabin = linked(&hub, &ada, "Cabin").await;
    for server in [&living_room, &cabin] {
        let code = start_user(&hub, server, 7).await.unwrap();
        approve_user(&hub, &bea, &code).await.unwrap();
    }

    assert_eq!(
        linked_users(&hub, &bea).await,
        ["Grace on Living room", "Grace on Cabin"]
    );
    assert_refused(
        start_user(&hub, &living_room, 7).await,
        ErrorCode::AlreadyExists,
        RefusalReason::REFUSAL_REASON_USER_LINKED,
    );
}

#[sqlx::test(migrations = false)]
async fn the_server_unlinks_only_its_own_user(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let living_room = linked(&hub, &ada, "Living room").await;
    let cabin = linked(&hub, &ada, "Cabin").await;
    for server in [&living_room, &cabin] {
        let code = start_user(&hub, server, 7).await.unwrap();
        approve_user(&hub, &ada, &code).await.unwrap();
    }

    unlink_user(&hub, &living_room, 7).await;

    assert_eq!(user_account(&hub, &living_room, 7).await, "");
    assert_eq!(linked_users(&hub, &ada).await, ["Grace on Cabin"]);
    // Unlinked, they may Link again.
    start_user(&hub, &living_room, 7).await.unwrap();
}

#[sqlx::test(migrations = false)]
async fn an_account_unlinks_only_its_own_users(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let bea = create_proven(&hub, &mut inbox, "bea@example.com").await;
    let server = linked(&hub, &ada, "Ada's").await;
    let code = start_user(&hub, &server, 7).await.unwrap();
    let link = approve_user(&hub, &bea, &code).await.unwrap();

    unlink_linked_user(&hub, &ada, &link).await;
    unlink_linked_user(&hub, &bea, "not a uuid").await;
    assert_eq!(linked_users(&hub, &bea).await, ["Grace on Ada's"]);
    unlink_linked_user(&hub, &bea, &link).await;

    assert_eq!(linked_users(&hub, &bea).await, Vec::<String>::new());
    assert_eq!(user_account(&hub, &server, 7).await, "");
}

#[sqlx::test(migrations = false)]
async fn unlinking_a_server_unlinks_its_users(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let server = linked(&hub, &ada, "Ada's").await;
    let code = start_user(&hub, &server, 7).await.unwrap();
    approve_user(&hub, &ada, &code).await.unwrap();

    linked_server(&hub, &server)
        .unlink_server(UnlinkServerRequest::default())
        .await
        .unwrap();

    assert_eq!(linked_users(&hub, &ada).await, Vec::<String>::new());
}

#[sqlx::test(migrations = false)]
async fn linking_a_user_needs_a_proven_email_and_a_linked_server(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let bea = create(&hub, "bea@example.com").await;
    let server = linked(&hub, &ada, "Ada's").await;
    let code = start_user(&hub, &server, 7).await.unwrap();

    assert_refused(
        approve_user(&hub, &bea, &code).await,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_EMAIL_NOT_PROVEN,
    );
    assert_eq!(user_account(&hub, &server, 7).await, "");
    assert_refused(
        start_user(&hub, "no such credential", 7).await,
        ErrorCode::Unauthenticated,
        RefusalReason::REFUSAL_REASON_SERVER_NOT_LINKED,
    );
}

/// Posts `batch` to the Hub's OTLP/HTTP `/otlp/v1/{signal}`, as the Server presenting
/// `credential`, if any, and answers the Hub's status.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn share(hub: &TestHub, signal: &str, credential: Option<&str>, batch: &[u8]) -> StatusCode {
    let mut request = axum::http::Request::post(format!(
        "{}otlp/v1/{signal}",
        hub.client_config().base_uri()
    ))
    .header("content-type", "application/x-protobuf");
    if let Some(credential) = credential {
        request = request.header("authorization", format!("Bearer {credential}"));
    }
    let request = request
        .body(full_body(Bytes::copy_from_slice(batch)))
        .unwrap();
    HttpClient::plaintext()
        .send(request)
        .await
        .unwrap()
        .status()
}

#[sqlx::test(migrations = false)]
async fn a_linked_servers_shared_batches_reach_the_backend_unchanged_and_no_one_elses_do(
    db: PgPool,
) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let credential = linked(&hub, &ada, "Living room").await;

    let anyone = share(&hub, "traces", None, b"anyone's").await;
    let unknown = share(&hub, "traces", Some("no such credential"), b"unknown").await;
    let spans = share(&hub, "traces", Some(&credential), b"spans").await;
    let lines = share(&hub, "logs", Some(&credential), b"lines").await;

    assert_eq!(
        [anyone, unknown, spans, lines],
        [
            StatusCode::UNAUTHORIZED,
            StatusCode::UNAUTHORIZED,
            StatusCode::OK,
            StatusCode::OK
        ]
    );
    assert_eq!(
        hub.shared(),
        [
            (Signal::Traces, Bytes::from_static(b"spans")),
            (Signal::Logs, Bytes::from_static(b"lines"))
        ]
    );
}

#[sqlx::test(migrations = false)]
async fn a_linked_server_sharing_over_4_mib_a_minute_is_refused(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let credential = linked(&hub, &ada, "Living room").await;
    let batch = vec![0; 3 * 512 * 1024];

    let mut answers = Vec::new();
    for _ in 0..3 {
        answers.push(share(&hub, "traces", Some(&credential), &batch).await);
    }

    assert_eq!(
        answers,
        [
            StatusCode::OK,
            StatusCode::OK,
            StatusCode::TOO_MANY_REQUESTS
        ]
    );
    assert_eq!(hub.shared().len(), 2);
}
