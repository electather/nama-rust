use std::time::Instant;

use sqlx::PgPool;

use super::{WatchStateChange, merge};
use crate::{
    accounts::{AccountId, Accounts, Caller},
    db,
};

/// An Account, on a migrated `db`.
async fn account(db: &PgPool) -> AccountId {
    db::migrate(db).await.unwrap();
    let caller = Caller {
        ip: "203.0.113.1".parse().unwrap(),
        now: Instant::now(),
    };
    let created = Accounts::new(db.clone(), None)
        .create("ada@example.com", "correct horse battery", caller)
        .await
        .unwrap();
    created.account.id
}

/// Alien's Watch State at `position_ms` of 7000, changed at `changed_at_ms`.
fn alien_at(position_ms: i64, changed_at_ms: i64) -> WatchStateChange {
    WatchStateChange {
        user_id: 1,
        episode: false,
        ids: vec!["tmdb-348".to_owned(), "imdb-tt0078748".to_owned()],
        played: false,
        position_ms,
        duration_ms: 7000,
        changed_at_ms,
    }
}

/// Merges each change, in order, into `account`'s Watch State.
async fn push(db: &PgPool, account: AccountId, changes: &[WatchStateChange]) {
    let mut conn = db.acquire().await.unwrap();
    for change in changes {
        merge(&mut conn, account, change).await.unwrap();
    }
}

/// `account`'s Watch State of the Movie known as `id`: played, position and duration.
async fn kept(db: &PgPool, account: AccountId, id: &str) -> Option<(bool, i64, i64)> {
    sqlx::query!(
        "SELECT played, position_ms, duration_ms FROM watch_states
         WHERE account_id = $1 AND NOT episode AND title_id = $2",
        account.0,
        id,
    )
    .fetch_optional(db)
    .await
    .unwrap()
    .map(|row| (row.played, row.position_ms, row.duration_ms))
}

#[sqlx::test(migrations = false)]
async fn two_servers_conflicting_changes_end_with_the_latest_whichever_arrives_last(db: PgPool) {
    let account = account(&db).await;
    let server_a = alien_at(3000, 2_000);
    let server_b = alien_at(1000, 1_000);

    push(&db, account, &[server_a, server_b]).await;

    assert_eq!(
        kept(&db, account, "tmdb-348").await,
        Some((false, 3000, 7000))
    );
    assert_eq!(
        kept(&db, account, "imdb-tt0078748").await,
        Some((false, 3000, 7000))
    );

    let server_b_later = alien_at(5000, 3_000);
    push(&db, account, &[server_b_later]).await;

    assert_eq!(
        kept(&db, account, "tmdb-348").await,
        Some((false, 5000, 7000))
    );
}

#[sqlx::test(migrations = false)]
async fn marking_unplayed_later_wins_over_played(db: PgPool) {
    let account = account(&db).await;
    let played = WatchStateChange {
        played: true,
        ..alien_at(7000, 1_000)
    };
    let unplayed = WatchStateChange {
        duration_ms: 0,
        ..alien_at(0, 2_000)
    };

    push(&db, account, &[played, unplayed]).await;

    assert_eq!(kept(&db, account, "tmdb-348").await, Some((false, 0, 0)));
}

#[sqlx::test(migrations = false)]
async fn a_change_naming_an_id_twice_keeps_it_once(db: PgPool) {
    let account = account(&db).await;
    let twice = WatchStateChange {
        ids: vec!["tmdb-348".to_owned(), "tmdb-348".to_owned()],
        ..alien_at(3000, 1_000)
    };

    push(&db, account, &[twice]).await;

    assert_eq!(
        kept(&db, account, "tmdb-348").await,
        Some((false, 3000, 7000))
    );
}
