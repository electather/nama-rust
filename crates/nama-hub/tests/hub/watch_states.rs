//! Linked Servers pushing and pulling their Users' Watch States, against a real PostgreSQL.

use crate::support::hub::TestHub;
use crate::support::linking::*;
use crate::support::refused::assert_refused;
use connectrpc::ErrorCode;
use nama_hub_proto::nama::hub::v1::{
    PullWatchStatesRequest, PushWatchStatesRequest, RefusalReason, WatchStateChange,
};
use sqlx::PgPool;

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
    let without_a_namespace = WatchStateChange {
        ids: vec!["tmdb-348".to_owned(), "603".to_owned()],
        ..alien_at(7, 3000, 1_000)
    };
    let refused = assert_refused(
        push(
            &hub,
            &server,
            vec![alien_at(7, 3000, 1_000), without_a_namespace],
        )
        .await,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_WATCH_STATE_INVALID,
    );
    assert_eq!(refused.field, "changes[1].ids");
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
