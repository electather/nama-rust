use std::{sync::Arc, time::Instant};

use super::*;
use crate::{
    accounts::{Accounts, Caller},
    db,
    signing::Keys,
};

/// A signing key made with `openssl genpkey -algorithm ed25519`, which `Links` needs.
const SIGNING_KEY: &str = "-----BEGIN PRIVATE KEY-----
MC4CAQAwBQYDK2VwBCIEIJzJDsQn3SpQXPoFGl74lUI4L+huggYWuNcSt9vaCwTS
-----END PRIVATE KEY-----
";

fn played() -> ViewingEvent {
    ViewingEvent {
        kind: Kind::Movie,
        external_id: "tmdb-603".to_owned(),
        started_at: 1_700_000_000,
        time_of_day_minutes: 1260,
        stop_position_ms: 5_000,
        duration_ms: 8_160_000,
        finished: false,
        device_kind: DeviceKind::Tv,
    }
}

#[test]
fn a_play_within_the_rules_is_valid() {
    assert!(played().is_valid());
}

#[test]
fn a_play_without_an_external_id_is_invalid() {
    let event = ViewingEvent {
        external_id: String::new(),
        ..played()
    };

    assert!(!event.is_valid());
}

#[test]
fn a_time_of_day_past_the_last_minute_is_invalid() {
    let event = ViewingEvent {
        time_of_day_minutes: MINUTES_PER_DAY,
        ..played()
    };

    assert!(!event.is_valid());
}

#[test]
fn a_stop_past_the_end_is_invalid() {
    let event = ViewingEvent {
        stop_position_ms: 8_160_001,
        ..played()
    };

    assert!(!event.is_valid());
}

#[test]
fn a_key_over_its_length_is_invalid() {
    let event = UserViewingEvent {
        user_id: 1,
        key: "k".repeat(KEY_LENGTH + 1),
        event: played(),
    };

    assert!(!event.is_valid());
}

#[test]
fn each_device_kind_reads_back_as_stored() {
    for kind in DeviceKind::ALL {
        assert_eq!(DeviceKind::stored(kind.as_str()), kind);
    }
}

/// Viewing on a migrated `db`, and `count` new Accounts.
async fn pool_of(db: &PgPool, count: usize) -> (Viewing, Vec<AccountId>) {
    db::migrate(db).await.unwrap();
    let mut created = Vec::new();
    for n in 0..count {
        // A fresh `Accounts` each, so its per-IP limit on creations never refuses one.
        let accounts = Accounts::new(db.clone(), None);
        let caller = Caller {
            ip: "203.0.113.1".parse().unwrap(),
            now: Instant::now(),
        };
        let email = format!("viewer{n}@example.com");
        let account = accounts
            .create(&email, "correct horse battery", caller)
            .await;
        created.push(account.unwrap().account.id);
    }
    let keys = Arc::new(Keys::from_pems(SIGNING_KEY).unwrap());
    let links = Links::new(db.clone(), Accounts::new(db.clone(), None), keys);
    (Viewing::new(db.clone(), links), created)
}

/// Records that `account` finished `movie`, as the play named `key`.
async fn finish(db: &PgPool, account: AccountId, key: &str, movie: &str) {
    let event = ViewingEvent {
        external_id: movie.to_owned(),
        stop_position_ms: 8_000_000,
        finished: true,
        ..played()
    };
    let mut tx = db.begin().await.unwrap();
    record_one(&mut tx, account, key, &event).await.unwrap();
    tx.commit().await.unwrap();
}

/// The Movie's own count, as recommendations would read it.
async fn count_of(viewing: &Viewing, movie: &str) -> Option<i64> {
    viewing.pooled_count(movie, movie).await.unwrap()
}

#[test]
fn only_a_finished_movie_is_poolable() {
    let finished = ViewingEvent {
        finished: true,
        ..played()
    };
    let episode = ViewingEvent {
        kind: Kind::Episode,
        ..finished.clone()
    };

    assert!(finished.is_poolable());
    assert!(!played().is_poolable());
    assert!(!episode.is_poolable());
}

#[sqlx::test(migrations = false)]
async fn a_count_under_twenty_accounts_cant_be_read(db: PgPool) {
    let (viewing, accounts) = pool_of(&db, 20).await;
    let (last, first_nineteen) = accounts.split_last().unwrap();
    for &account in first_nineteen {
        finish(&db, account, "a", "tmdb-603").await;
    }

    assert_eq!(count_of(&viewing, "tmdb-603").await, None);

    finish(&db, *last, "a", "tmdb-603").await;

    assert_eq!(count_of(&viewing, "tmdb-603").await, Some(20));
}

#[sqlx::test(migrations = false)]
async fn an_account_with_pooling_off_adds_nothing(db: PgPool) {
    let (viewing, accounts) = pool_of(&db, 20).await;
    let (opted_out, first_nineteen) = accounts.split_last().unwrap();
    for &account in first_nineteen {
        finish(&db, account, "a", "tmdb-603").await;
        finish(&db, account, "b", "tmdb-78").await;
    }

    viewing.set_pooling(*opted_out, false).await.unwrap();
    finish(&db, *opted_out, "a", "tmdb-603").await;
    finish(&db, *opted_out, "b", "tmdb-78").await;

    assert!(!viewing.pooling(*opted_out).await.unwrap());
    assert_eq!(count_of(&viewing, "tmdb-603").await, None);
    assert_eq!(count_of(&viewing, "tmdb-78").await, None);
    assert_eq!(
        viewing.pooled_count("tmdb-603", "tmdb-78").await.unwrap(),
        None
    );
}

#[sqlx::test(migrations = false)]
async fn a_movie_finished_again_counts_its_account_once(db: PgPool) {
    let (viewing, accounts) = pool_of(&db, 19).await;
    for &account in &accounts {
        finish(&db, account, "a", "tmdb-603").await;
    }

    finish(&db, accounts[0], "b", "tmdb-603").await;

    assert_eq!(count_of(&viewing, "tmdb-603").await, None);
}

#[sqlx::test(migrations = false)]
async fn twenty_accounts_finishing_two_movies_count_the_pair_either_way(db: PgPool) {
    let (viewing, accounts) = pool_of(&db, 20).await;
    for &account in &accounts {
        finish(&db, account, "a", "tmdb-603").await;
        finish(&db, account, "b", "tmdb-78").await;
    }

    let pair = |a, b| viewing.pooled_count(a, b);
    assert_eq!(pair("tmdb-603", "tmdb-78").await.unwrap(), Some(20));
    assert_eq!(pair("tmdb-78", "tmdb-603").await.unwrap(), Some(20));
    assert_eq!(count_of(&viewing, "tmdb-78").await, Some(20));
}

#[sqlx::test(migrations = false)]
async fn vectors_train_only_on_counts_of_twenty_accounts(db: PgPool) {
    let (_, accounts) = pool_of(&db, 20).await;
    let (last, first_nineteen) = accounts.split_last().unwrap();
    for &account in first_nineteen {
        finish(&db, account, "a", "tmdb-603").await;
        finish(&db, account, "b", "tmdb-78").await;
    }
    finish(&db, *last, "a", "tmdb-603").await;

    let pooled = pooled_counts(&db).await.unwrap();

    let only_the_matrix = PooledCount {
        a: "tmdb-603".to_owned(),
        b: "tmdb-603".to_owned(),
        accounts: 20,
    };
    assert_eq!(pooled, [only_the_matrix]);
}
