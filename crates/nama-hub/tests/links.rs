//! Registering a Server through the device flow, listing an Account's Servers and Unlinking one,
//! and Linking a Linked Server's Users to Accounts, against a real PostgreSQL.

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
use nama_hub_proto::nama::hub::v1::{RefusalReason, UnlinkServerRequest};
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
