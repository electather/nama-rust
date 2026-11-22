//! Registering a Server through the device flow, listing an Account's Servers and Unlinking one,
//! against a real PostgreSQL.

#[path = "support/hub.rs"]
mod hub;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/mail.rs"]
mod mail;
#[path = "support/refused.rs"]
mod refused;

use axum::{
    body::{Body, Bytes},
    http::HeaderMap,
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use connectrpc::{
    ErrorCode,
    client::{ClientTransport as _, HttpClient, full_body},
};
use hub::TestHub;
use mail::{Inbox, fake_mail};
use nama_hub::titles;
use nama_hub_proto::nama::hub::v1::{
    AccountServiceClient, ApproveServerLinkRequest, CreateAccountRequest, ExternalId,
    LinkServiceClient, LinkedServerServiceClient, ListLinkedServersRequest,
    ListSimilarTitlesRequest, PollServerLinkRequest, PollServerLinkResponse, RefusalReason,
    SendSignInLinkRequest, ServerLinkServiceClient, SignInWithLinkRequest, StartServerLinkRequest,
    StartServerLinkResponse, TitleKind, UnlinkServerRequest,
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

#[sqlx::test(migrations = false)]
async fn a_linked_server_gets_the_titles_like_one_it_names(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db.clone()).await;
    titles::import(&db, &include_bytes!("fixtures/wikidata-dump.json")[..])
        .await
        .unwrap();
    titles::train(&db).await.unwrap();
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let started = start(&hub, "Living room").await;
    approve(&hub, &ada, &started.user_code).await.unwrap();
    let credential = poll(&hub, &started.device_code).await.unwrap().credential;
    let the_matrix = ListSimilarTitlesRequest {
        kind: TitleKind::TITLE_KIND_MOVIE.into(),
        ids: vec![ExternalId {
            namespace: "tmdb".to_owned(),
            value: "603".to_owned(),
            ..Default::default()
        }],
        ..Default::default()
    };

    let similar = linked_server(&hub, &credential)
        .list_similar_titles(the_matrix.clone())
        .await
        .unwrap()
        .into_owned();

    let ids: Vec<&str> = (similar.titles.iter())
        .map(|title| title.id.value.as_str())
        .collect();
    assert_eq!(ids, ["604", "348"]);
    assert_refused(
        linked_server(&hub, "not-a-credential")
            .list_similar_titles(the_matrix)
            .await,
        ErrorCode::Unauthenticated,
        RefusalReason::REFUSAL_REASON_SERVER_NOT_LINKED,
    );
}
