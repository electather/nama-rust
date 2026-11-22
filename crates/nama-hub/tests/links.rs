//! Registering a Server through the device flow, listing an Account's Servers with their Server
//! Addresses, Unlinking one, and Linking a Linked Server's Users to
//! Accounts, against a real PostgreSQL.

#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/hub.rs"]
mod hub;
#[path = "support/linking.rs"]
mod linking;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/mail.rs"]
mod mail;
#[path = "support/refused.rs"]
mod refused;

use axum::body::{Body, Bytes};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use connectrpc::{
    ErrorCode,
    client::{ClientTransport as _, HttpClient, full_body},
};
use hub::TestHub;
use linking::{
    approve, create, create_proven, hub_with_mail, linked, linked_server, links, poll, start,
};
use nama_hub::titles;
use nama_hub_proto::nama::hub::v1::{
    ApproveUserLinkRequest, ExternalId, GetUserLinkRequest, ListLinkedServersRequest,
    ListLinkedUsersRequest, ListSimilarTitlesRequest, RefusalReason, SetServerAddressesRequest,
    StartUserLinkRequest, TitleKind, UnlinkLinkedUserRequest, UnlinkServerRequest,
    UnlinkUserRequest,
};
use refused::assert_refused;
use ring::signature::{ED25519, UnparsedPublicKey};
use serde_json::Value;
use sqlx::PgPool;

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

#[sqlx::test(migrations = false)]
async fn a_linked_server_gets_the_titles_like_one_it_names(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db.clone()).await;
    titles::import(&db, &include_bytes!("fixtures/wikidata-dump.json")[..])
        .await
        .unwrap();
    titles::train(&db).await.unwrap();
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let credential = linked(&hub, &ada, "Living room").await;
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

/// Reports `url` and `lan_addresses` as the Server Addresses of the Server presenting `credential`.
async fn set_addresses(
    hub: &TestHub,
    credential: &str,
    url: &str,
    lan_addresses: &[&str],
) -> Result<(), connectrpc::ConnectError> {
    linked_server(hub, credential)
        .set_server_addresses(SetServerAddressesRequest {
            url: url.to_owned(),
            lan_addresses: lan_addresses
                .iter()
                .map(|&address| address.to_owned())
                .collect(),
            ..Default::default()
        })
        .await?;
    Ok(())
}

/// Each Server the Account signed in as `token` lists: its name, its Server URL and its Server
/// Addresses.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn server_addresses(hub: &TestHub, token: &str) -> Vec<(String, String, Vec<String>)> {
    let listed = links(hub, token)
        .list_linked_servers(ListLinkedServersRequest::default())
        .await
        .unwrap()
        .into_owned();
    listed
        .servers
        .into_iter()
        .map(|server| (server.name, server.url, server.addresses))
        .collect()
}

#[sqlx::test(migrations = false)]
async fn an_account_lists_the_server_addresses_its_server_last_reported(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let living_room = linked(&hub, &ada, "Living room").await;
    assert_eq!(
        server_addresses(&hub, &ada).await,
        [("Living room".to_owned(), String::new(), vec![])]
    );
    set_addresses(&hub, &living_room, "", &["http://192.168.1.10:7980"])
        .await
        .unwrap();

    set_addresses(
        &hub,
        &living_room,
        "https://nama.example.com",
        &["http://192.168.1.11:7980"],
    )
    .await
    .unwrap();

    assert_eq!(
        server_addresses(&hub, &ada).await,
        [(
            "Living room".to_owned(),
            "https://nama.example.com".to_owned(),
            vec![
                "https://nama.example.com".to_owned(),
                "http://192.168.1.11:7980".to_owned()
            ]
        )]
    );
}

#[sqlx::test(migrations = false)]
async fn an_account_lists_the_servers_its_users_are_on_and_no_others(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let bea = create_proven(&hub, &mut inbox, "bea@example.com").await;
    let cy = create_proven(&hub, &mut inbox, "cy@example.com").await;
    let living_room = linked(&hub, &ada, "Living room").await;
    linked(&hub, &ada, "Cabin").await;
    set_addresses(&hub, &living_room, "https://nama.example.com", &[])
        .await
        .unwrap();
    let code = start_user(&hub, &living_room, 7).await.unwrap();
    approve_user(&hub, &bea, &code).await.unwrap();

    assert_eq!(
        server_addresses(&hub, &bea).await,
        [(
            "Living room".to_owned(),
            "https://nama.example.com".to_owned(),
            vec!["https://nama.example.com".to_owned()]
        )]
    );
    assert_eq!(server_names(&hub, &ada).await, ["Living room", "Cabin"]);
    assert_eq!(server_names(&hub, &cy).await, Vec::<String>::new());
}

#[sqlx::test(migrations = false)]
async fn a_server_sets_only_its_own_addresses(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let bea = create_proven(&hub, &mut inbox, "bea@example.com").await;
    linked(&hub, &ada, "Ada's").await;
    let beas = linked(&hub, &bea, "Bea's").await;

    set_addresses(&hub, &beas, "https://bea.example.com", &[])
        .await
        .unwrap();

    assert_eq!(
        server_addresses(&hub, &ada).await,
        [("Ada's".to_owned(), String::new(), vec![])]
    );
}

#[sqlx::test(migrations = false)]
async fn a_server_address_must_be_an_http_url(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let living_room = linked(&hub, &ada, "Living room").await;
    let too_many = ["http://192.168.1.10:7980"; 17];
    let too_long = format!("https://{}.example.com", "a".repeat(2000));

    for (url, lan_addresses) in [
        ("javascript:alert(1)", &[][..]),
        ("", &["192.168.1.10:7980"][..]),
        (too_long.as_str(), &[][..]),
        ("", &too_many[..]),
    ] {
        assert_refused(
            set_addresses(&hub, &living_room, url, lan_addresses).await,
            ErrorCode::InvalidArgument,
            RefusalReason::REFUSAL_REASON_SERVER_ADDRESS_INVALID,
        );
    }
    assert_refused(
        set_addresses(&hub, "not a credential", "", &[]).await,
        ErrorCode::Unauthenticated,
        RefusalReason::REFUSAL_REASON_SERVER_NOT_LINKED,
    );
    assert_eq!(
        server_addresses(&hub, &ada).await,
        [("Living room".to_owned(), String::new(), vec![])]
    );
}
