//! Registering a Server through the device flow, listing an Account's Servers, Unlinking one,
//! Linking a Linked Server's Users to Accounts, pushing and pulling their Watch State, and syncing
//! their Watchlists and reading the most watchlisted, against a real PostgreSQL.

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
    ExternalId, ListMostWatchlistedTitlesRequest, ListSimilarTitlesRequest, PullWatchStatesRequest,
    PushWatchStatesRequest, RefusalReason, SyncWatchlistRequest, TitleKind, UnlinkServerRequest,
    WatchStateChange, WatchlistEntry,
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
    let (hub, _inbox) = hub_with_mail(db).await;
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
async fn on_a_hub_without_mail_an_unproven_account_cant_link_a_server(db: PgPool) {
    let hub = TestHub::start(db, None).await;
    let ada = create(&hub, "ada@example.com").await;
    let started = start(&hub, "Living room").await;

    assert_refused(
        approve(&hub, &ada, &started.user_code).await,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_SIGN_IN_LINKS_OFF,
    );
    assert!(!poll(&hub, &started.device_code).await.unwrap().linked);
}

#[sqlx::test(migrations = false)]
async fn an_email_proven_while_the_hub_had_mail_still_links_without_it(db: PgPool) {
    let (with_mail, mut inbox) = hub_with_mail(db.clone()).await;
    let without_mail = TestHub::start(db, None).await;
    let ada = create_proven(&with_mail, &mut inbox, "ada@example.com").await;
    let started = start(&without_mail, "Living room").await;

    approve(&without_mail, &ada, &started.user_code)
        .await
        .unwrap();

    assert_eq!(server_names(&without_mail, &ada).await, ["Living room"]);
}

#[sqlx::test(migrations = false)]
async fn linking_needs_a_code_a_server_shows(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;

    let refusal = assert_refused(
        approve(&hub, &ada, "BDFH-JKLM").await,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_LINK_CODE_NOT_FOUND,
    );
    assert_eq!(refusal.field, "code");
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
async fn an_account_links_one_user_on_each_server(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let server = linked(&hub, &ada, "Ada's").await;
    let first = start_user(&hub, &server, 7).await.unwrap();
    approve_user(&hub, &ada, &first).await.unwrap();
    let second = start_user(&hub, &server, 8).await.unwrap();

    assert_refused(
        approve_user(&hub, &ada, &second).await,
        ErrorCode::AlreadyExists,
        RefusalReason::REFUSAL_REASON_ACCOUNT_HAS_USER_ON_SERVER,
    );
    assert_eq!(user_account(&hub, &server, 8).await, "");
    // The refused code still works once the first User is Unlinked.
    unlink_user(&hub, &server, 7).await;
    approve_user(&hub, &ada, &second).await.unwrap();
    assert_eq!(user_account(&hub, &server, 8).await, "ada@example.com");
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
async fn linking_a_user_needs_a_proven_email(db: PgPool) {
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
}

#[sqlx::test(migrations = false)]
async fn on_a_hub_without_mail_an_unproven_account_cant_link_a_user(db: PgPool) {
    let (with_mail, mut inbox) = hub_with_mail(db.clone()).await;
    let without_mail = TestHub::start(db, None).await;
    let ada = create_proven(&with_mail, &mut inbox, "ada@example.com").await;
    let bea = create(&without_mail, "bea@example.com").await;
    let server = linked(&with_mail, &ada, "Ada's").await;
    let code = start_user(&without_mail, &server, 7).await.unwrap();

    assert_refused(
        approve_user(&without_mail, &bea, &code).await,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_SIGN_IN_LINKS_OFF,
    );
    assert_eq!(user_account(&without_mail, &server, 7).await, "");
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
}

#[sqlx::test(migrations = false)]
async fn a_push_breaking_a_limit_is_refused(db: PgPool) {
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
}

/// The Watch State the Server presenting `credential` pulls for its User `user_id`, each title
/// and id's as its User, id, position and when it was made, in order.
async fn pulled(
    hub: &TestHub,
    credential: &str,
    user_id: i64,
) -> Result<Vec<(i64, Vec<String>, u64, i64)>, connectrpc::ConnectError> {
    let pulled = linked_server(hub, credential)
        .pull_watch_states(PullWatchStatesRequest {
            user_id,
            ..Default::default()
        })
        .await?
        .into_owned();
    let mut pulled: Vec<_> = (pulled.watch_states.into_iter())
        .map(|state| {
            (
                state.user_id,
                state.ids,
                state.position_ms,
                state.changed_at_ms,
            )
        })
        .collect();
    pulled.sort();
    Ok(pulled)
}

/// What a Server pulls for its User `user_id` of Alien at `position_ms`, changed at
/// `changed_at_ms`: a row for each of its ids.
fn alien_pulled(
    user_id: i64,
    position_ms: u64,
    changed_at_ms: i64,
) -> Vec<(i64, Vec<String>, u64, i64)> {
    ["imdb-tt0078748", "tmdb-348"]
        .map(|id| (user_id, vec![id.to_owned()], position_ms, changed_at_ms))
        .to_vec()
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
    let pushed = vec![
        alien_at(7, 3000, 2_000),
        alien_at(8, 1000, 4_000),
        alien_at(9, 5000, 3_000),
    ];
    push(&hub, &server, pushed).await.unwrap();

    assert_eq!(
        pulled(&hub, &server, 7).await.unwrap(),
        alien_pulled(7, 3000, 2_000),
        "neither Bea's nor User 9's, though both were made later"
    );
    assert_eq!(
        pulled(&hub, &server, 8).await.unwrap(),
        alien_pulled(8, 1000, 4_000)
    );
    assert_eq!(
        pulled(&hub, &server, 9).await.unwrap(),
        [],
        "User 9 isn't Linked"
    );
}

#[sqlx::test(migrations = false)]
async fn a_server_pulls_what_another_server_of_the_account_pushed_by_each_id(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let (first, second) = (
        linked(&hub, &ada, "Living room").await,
        linked(&hub, &ada, "Cabin").await,
    );
    let first_code = start_user(&hub, &first, 7).await.unwrap();
    approve_user(&hub, &ada, &first_code).await.unwrap();
    let second_code = start_user(&hub, &second, 3).await.unwrap();
    approve_user(&hub, &ada, &second_code).await.unwrap();
    let made_at = 1_700_000_000_123;

    push(&hub, &first, vec![alien_at(7, 3000, made_at)])
        .await
        .unwrap();

    assert_eq!(
        pulled(&hub, &second, 3).await.unwrap(),
        alien_pulled(3, 3000, made_at)
    );
}

#[sqlx::test(migrations = false)]
async fn of_two_changes_made_at_the_same_moment_in_one_push_the_one_pushed_last_wins(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let server = linked(&hub, &ada, "Ada's").await;
    let code = start_user(&hub, &server, 7).await.unwrap();
    approve_user(&hub, &ada, &code).await.unwrap();
    let at_once = vec![alien_at(7, 1000, 1_000), alien_at(7, 3000, 1_000)];

    push(&hub, &server, at_once).await.unwrap();

    assert_eq!(
        pulled(&hub, &server, 7).await.unwrap(),
        alien_pulled(7, 3000, 1_000)
    );
}

/// The Watchlist entry of the Movie `tmdb-603`, on the Watchlist since `changed_at_ms`.
fn the_matrix(changed_at_ms: i64) -> WatchlistEntry {
    WatchlistEntry {
        kind: TitleKind::TITLE_KIND_MOVIE.into(),
        id: "tmdb-603".to_owned(),
        changed_at_ms,
        ..Default::default()
    }
}

/// Syncs `entries` as the Watchlist of the User `user_id` of the Server presenting `credential`,
/// and answers when each entry of the Account's changed.
async fn sync_watchlist(
    hub: &TestHub,
    credential: &str,
    user_id: i64,
    entries: Vec<WatchlistEntry>,
) -> Result<Vec<i64>, connectrpc::ConnectError> {
    let synced = linked_server(hub, credential)
        .sync_watchlist(SyncWatchlistRequest {
            user_id,
            entries,
            ..Default::default()
        })
        .await?
        .into_owned();
    Ok((synced.entries.iter())
        .map(|entry| entry.changed_at_ms)
        .collect())
}

// Which change wins is checked where it merges, in `src/sync/sync_tests.rs`.
#[sqlx::test(migrations = false)]
async fn a_linked_server_syncs_a_watchlist_only_with_the_account_its_user_is_linked_to(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let bea = create_proven(&hub, &mut inbox, "bea@example.com").await;
    let server = linked(&hub, &ada, "Ada's").await;
    let ada_code = start_user(&hub, &server, 7).await.unwrap();
    approve_user(&hub, &ada, &ada_code).await.unwrap();
    let bea_code = start_user(&hub, &server, 8).await.unwrap();
    approve_user(&hub, &bea, &bea_code).await.unwrap();

    let synced = sync_watchlist(&hub, &server, 8, vec![the_matrix(1_000)]).await;
    assert_eq!(synced.unwrap(), [1_000]);
    let not_linked = sync_watchlist(&hub, &server, 9, vec![the_matrix(2_000)]).await;
    assert_eq!(not_linked.unwrap(), Vec::<i64>::new());
    assert_eq!(
        sync_watchlist(&hub, &server, 7, Vec::new()).await.unwrap(),
        Vec::<i64>::new(),
        "Ada gets none of Bea's Watchlist, nor User 9's"
    );
    let unspecified = WatchlistEntry {
        kind: TitleKind::TITLE_KIND_UNSPECIFIED.into(),
        ..the_matrix(1_000)
    };
    let refused = assert_refused(
        sync_watchlist(&hub, &server, 7, vec![the_matrix(1_000), unspecified]).await,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_WATCHLIST_INVALID,
    );
    assert_eq!(refused.field, "entries[1].kind");
}

// How many Accounts it takes is checked where they're counted, in `src/sync/sync_tests.rs`.
#[sqlx::test(migrations = false)]
async fn a_title_on_one_accounts_watchlist_isnt_most_watchlisted(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let server = linked(&hub, &ada, "Ada's").await;
    let code = start_user(&hub, &server, 7).await.unwrap();
    approve_user(&hub, &ada, &code).await.unwrap();
    sync_watchlist(&hub, &server, 7, vec![the_matrix(1_000)])
        .await
        .unwrap();

    let watchlisted = linked_server(&hub, &server)
        .list_most_watchlisted_titles(ListMostWatchlistedTitlesRequest::default())
        .await
        .unwrap();

    assert_eq!(watchlisted.into_owned().titles, []);
}
