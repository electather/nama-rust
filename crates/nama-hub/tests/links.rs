//! Registering a Server through the device flow, listing an Account's Servers with their Server
//! Addresses, Unlinking one, Linking a Linked Server's Users to Accounts, and pushing and pulling
//! their Watch State, against a real PostgreSQL.

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

use connectrpc::ErrorCode;
use hub::TestHub;
use linking::*;
use nama_hub::titles;
use nama_hub_proto::nama::hub::v1::{
    ExternalId, ListLinkedServersRequest, ListSimilarTitlesRequest, PullWatchStatesRequest,
    PushWatchStatesRequest, RefusalReason, SetServerAddressesRequest, SetServerAddressesResponse,
    TitleKind, UnlinkServerRequest, WatchStateChange,
};
use refused::assert_refused;
use sqlx::PgPool;

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

/// The User `user_id`'s Watch State of Alien at `position_ms` of 7000, changed at `changed_at_ms`.
fn alien_at(user_id: i64, position_ms: u64, changed_at_ms: i64) -> WatchStateChange {
    WatchStateChange {
        user_id,
        ids: vec!["tmdb-348".to_owned(), "imdb-tt0078748".to_owned()],
        position_ms,
        duration_ms: 7000,
        changed_at_ms,
        ..Default::default()
    }
}

/// Pushes `changes` from the Server presenting `credential`.
async fn push(
    hub: &TestHub,
    credential: &str,
    changes: Vec<WatchStateChange>,
) -> Result<(), connectrpc::ConnectError> {
    linked_server(hub, credential)
        .push_watch_states(PushWatchStatesRequest {
            changes,
            ..Default::default()
        })
        .await?;
    Ok(())
}

#[sqlx::test(migrations = false)]
async fn a_linked_server_gets_the_titles_like_one_it_names(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db.clone()).await;
    titles::import(&db, &include_bytes!("fixtures/wikidata-dump.json")[..])
        .await
        .unwrap();
    nama_hub::train_vectors(&db).await.unwrap();
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
    set_addresses_with_port(hub, credential, url, lan_addresses, 0).await?;
    Ok(())
}

/// Reports `url`, `lan_addresses` and `public_port` as the Server Addresses of the Server
/// presenting `credential`, and answers what the Hub found at the port.
async fn set_addresses_with_port(
    hub: &TestHub,
    credential: &str,
    url: &str,
    lan_addresses: &[&str],
    public_port: u32,
) -> Result<SetServerAddressesResponse, connectrpc::ConnectError> {
    let set = linked_server(hub, credential)
        .set_server_addresses(SetServerAddressesRequest {
            url: url.to_owned(),
            lan_addresses: lan_addresses
                .iter()
                .map(|&address| address.to_owned())
                .collect(),
            public_port,
            ..Default::default()
        })
        .await?;
    Ok(set.into_owned())
}

// Which Account a push reaches, and which change wins, is checked where it merges, in
// `src/sync/sync_tests.rs`, since only `sync` reads its table.
#[sqlx::test(migrations = false)]
async fn a_linked_server_pushes_its_users_watch_state_linked_or_not(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let server = linked(&hub, &ada, "Ada's").await;
    let code = start_user(&hub, &server, 7).await.unwrap();
    approve_user(&hub, &ada, &code).await.unwrap();

    push(
        &hub,
        &server,
        vec![alien_at(7, 3000, 2_000), alien_at(8, 1000, 1_000)],
    )
    .await
    .unwrap();
}

#[sqlx::test(migrations = false)]
async fn a_push_breaking_a_limit_or_from_no_linked_server_is_refused(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let server = linked(&hub, &ada, "Ada's").await;
    let unmatched = WatchStateChange {
        ids: Vec::new(),
        ..alien_at(7, 3000, 1_000)
    };

    let past_the_end = assert_refused(
        push(
            &hub,
            &server,
            vec![alien_at(7, 3000, 1_000), alien_at(7, 8000, 1_000)],
        )
        .await,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_WATCH_STATE_INVALID,
    );
    assert_eq!(past_the_end.field, "changes[1].position_ms");
    let without_ids = assert_refused(
        push(&hub, &server, vec![unmatched]).await,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_WATCH_STATE_INVALID,
    );
    assert_eq!(without_ids.field, "changes[0].ids");
    assert_refused(
        push(&hub, "no such credential", vec![alien_at(7, 3000, 1_000)]).await,
        ErrorCode::Unauthenticated,
        RefusalReason::REFUSAL_REASON_SERVER_NOT_LINKED,
    );
}

/// The positions of the Watch State the Server presenting `credential` pulls for its User
/// `user_id`, one per title and id.
async fn pulled_positions(
    hub: &TestHub,
    credential: &str,
    user_id: i64,
) -> Result<Vec<u64>, connectrpc::ConnectError> {
    let pulled = linked_server(hub, credential)
        .pull_watch_states(PullWatchStatesRequest {
            user_id,
            ..Default::default()
        })
        .await?
        .into_owned();
    Ok((pulled.watch_states.iter())
        .map(|state| state.position_ms)
        .collect())
}

#[sqlx::test(migrations = false)]
async fn a_linked_server_pulls_only_the_account_its_user_is_linked_to(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let bea = create_proven(&hub, &mut inbox, "bea@example.com").await;
    let server = linked(&hub, &ada, "Ada's").await;
    let ada_code = start_user(&hub, &server, 7).await.unwrap();
    approve_user(&hub, &ada, &ada_code).await.unwrap();
    let bea_code = start_user(&hub, &server, 8).await.unwrap();
    approve_user(&hub, &bea, &bea_code).await.unwrap();
    let pushed = vec![alien_at(7, 3000, 2_000), alien_at(8, 1000, 1_000)];
    push(&hub, &server, pushed).await.unwrap();

    assert_eq!(
        pulled_positions(&hub, &server, 7).await.unwrap(),
        [3000, 3000]
    );
    assert_eq!(
        pulled_positions(&hub, &server, 9).await.unwrap(),
        Vec::<u64>::new()
    );
    assert_refused(
        pulled_positions(&hub, "no such credential", 7).await,
        ErrorCode::Unauthenticated,
        RefusalReason::REFUSAL_REASON_SERVER_NOT_LINKED,
    );
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
    let port_refusal = assert_refused(
        set_addresses_with_port(&hub, &living_room, "", &[], 65536).await,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_SERVER_ADDRESS_INVALID,
    );
    assert_eq!(port_refusal.params["maxPort"], "65535");
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

#[sqlx::test(migrations = false)]
async fn the_hub_lists_the_public_address_it_reached_between_the_server_url_and_the_lan(
    db: PgPool,
) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let living_room = linked(&hub, &ada, "Living room").await;
    // The test's calls come from 127.0.0.1, so the Hub tries the Server's port there.
    let open = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = open.local_addr().unwrap().port();
    let lan = ["http://192.168.1.10:7980"];

    let reached = set_addresses_with_port(
        &hub,
        &living_room,
        "https://nama.example.com",
        &lan,
        port.into(),
    )
    .await
    .unwrap();
    let listed = server_addresses(&hub, &ada).await;
    drop(open);
    let missed = set_addresses_with_port(
        &hub,
        &living_room,
        "https://nama.example.com",
        &lan,
        port.into(),
    )
    .await
    .unwrap();

    let public = format!("http://127.0.0.1:{port}");
    assert_eq!(
        (reached.public_address.as_str(), reached.reachable),
        (public.as_str(), true)
    );
    assert_eq!(
        listed[0].2,
        [
            "https://nama.example.com",
            public.as_str(),
            "http://192.168.1.10:7980"
        ]
    );
    assert_eq!(
        (missed.public_address.as_str(), missed.reachable),
        (public.as_str(), false)
    );
    assert_eq!(
        server_addresses(&hub, &ada).await[0].2,
        ["https://nama.example.com", "http://192.168.1.10:7980"]
    );
}
