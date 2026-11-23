use std::{
    sync::Arc,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

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
        show_external_id: None,
    }
}

/// [`played`]'s play, of an Episode of the Show `tmdb-1399`.
fn played_episode() -> ViewingEvent {
    ViewingEvent {
        kind: Kind::Episode,
        external_id: "tmdb-63056".to_owned(),
        show_external_id: Some("tmdb-1399".to_owned()),
        ..played()
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
fn an_episode_naming_its_show_is_valid() {
    assert!(played_episode().is_valid());
}

#[test]
fn an_episode_without_its_show_is_invalid() {
    let event = ViewingEvent {
        show_external_id: None,
        ..played_episode()
    };

    assert!(!event.is_valid());
}

#[test]
fn an_episode_whose_show_is_over_its_length_is_invalid() {
    let event = ViewingEvent {
        show_external_id: Some("1".repeat(EXTERNAL_ID_LENGTH + 1)),
        ..played_episode()
    };

    assert!(!event.is_valid());
}

#[test]
fn a_movie_naming_a_show_is_invalid() {
    let event = ViewingEvent {
        show_external_id: Some("tmdb-1399".to_owned()),
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
    finish_at(db, account, key, movie, played().started_at).await;
}

/// Records that `account` finished `movie`, as the play named `key` that started at `started_at`.
async fn finish_at(db: &PgPool, account: AccountId, key: &str, movie: &str, started_at: i64) {
    let event = ViewingEvent {
        external_id: movie.to_owned(),
        started_at,
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
async fn pooled_counts_hold_only_counts_of_twenty_accounts(db: PgPool) {
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

/// Now, in seconds since the Unix epoch.
fn now() -> i64 {
    let since_epoch = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    i64::try_from(since_epoch.as_secs()).unwrap()
}

/// A week, in seconds.
const WEEK: i64 = 7 * 24 * 60 * 60;

/// The External IDs trending, most watched first.
async fn trending_ids(viewing: &Viewing) -> Vec<String> {
    let trending = viewing.trending().await.unwrap();
    trending
        .into_iter()
        .map(|title| title.external_id)
        .collect()
}

#[sqlx::test(migrations = false)]
async fn movies_twenty_accounts_finished_this_week_trend_most_watched_first(db: PgPool) {
    let (viewing, accounts) = pool_of(&db, 21).await;
    let at = now();
    for (n, &account) in accounts.iter().enumerate() {
        finish_at(&db, account, "a", "tmdb-78", at).await;
        if n < 20 {
            finish_at(&db, account, "b", "tmdb-603", at).await;
        }
        if n < 19 {
            finish_at(&db, account, "c", "tmdb-1", at).await;
        }
    }

    let trending = viewing.trending().await.unwrap();

    let ids: Vec<&str> = (trending.iter())
        .map(|title| title.external_id.as_str())
        .collect();
    assert_eq!(ids, ["tmdb-78", "tmdb-603"]);
    assert!((trending[0].weight - 1.0).abs() < 1e-6);
    assert!((trending[1].weight - 20.0 / 21.0).abs() < 1e-6);
}

#[sqlx::test(migrations = false)]
async fn a_movie_finished_again_the_same_week_counts_its_account_once(db: PgPool) {
    let (viewing, accounts) = pool_of(&db, 19).await;
    let at = now();
    for &account in &accounts {
        finish_at(&db, account, "a", "tmdb-603", at).await;
    }

    finish_at(&db, accounts[0], "b", "tmdb-603", at).await;

    assert_eq!(trending_ids(&viewing).await, Vec::<String>::new());
}

#[sqlx::test(migrations = false)]
async fn last_weeks_count_still_trends_and_the_week_before_doesnt(db: PgPool) {
    let (viewing, accounts) = pool_of(&db, 20).await;
    let at = now();
    for &account in &accounts {
        finish_at(&db, account, "a", "tmdb-603", at - WEEK).await;
        finish_at(&db, account, "b", "tmdb-78", at - 2 * WEEK).await;
    }

    assert_eq!(trending_ids(&viewing).await, ["tmdb-603"]);
}

#[sqlx::test(migrations = false)]
async fn an_account_with_pooling_off_adds_nothing_to_whats_trending(db: PgPool) {
    let (viewing, accounts) = pool_of(&db, 20).await;
    let (opted_out, first_nineteen) = accounts.split_last().unwrap();
    let at = now();
    for &account in first_nineteen {
        finish_at(&db, account, "a", "tmdb-603", at).await;
    }

    viewing.set_pooling(*opted_out, false).await.unwrap();
    finish_at(&db, *opted_out, "a", "tmdb-603", at).await;

    assert_eq!(trending_ids(&viewing).await, Vec::<String>::new());
}

#[sqlx::test(migrations = false)]
async fn finishing_a_movie_again_after_deleting_the_counted_play_counts_its_account_once(
    db: PgPool,
) {
    let (viewing, accounts) = pool_of(&db, 19).await;
    let at = now();
    for &account in &accounts {
        finish_at(&db, account, "a", "tmdb-603", at).await;
    }

    for key in ["b", "c"] {
        let [counted] = viewing.list(accounts[0]).await.unwrap().try_into().unwrap();
        viewing.delete(accounts[0], counted.id).await.unwrap();
        finish_at(&db, accounts[0], key, "tmdb-603", at).await;
    }

    assert_eq!(trending_ids(&viewing).await, Vec::<String>::new());
}

#[sqlx::test(migrations = false)]
async fn deleting_the_counted_play_takes_its_account_out_of_the_weeks_count(db: PgPool) {
    let (viewing, accounts) = pool_of(&db, 20).await;
    let at = now();
    for &account in &accounts {
        finish_at(&db, account, "a", "tmdb-603", at).await;
    }
    assert_eq!(trending_ids(&viewing).await, ["tmdb-603"]);

    let [counted] = viewing.list(accounts[0]).await.unwrap().try_into().unwrap();
    viewing.delete(accounts[0], counted.id).await.unwrap();

    assert_eq!(trending_ids(&viewing).await, Vec::<String>::new());
}

#[sqlx::test(migrations = false)]
async fn deleting_an_account_deletes_its_plays_but_leaves_its_pooled_counts(db: PgPool) {
    let (viewing, mut accounts) = pool_of(&db, 19).await;
    let leaving = Accounts::new(db.clone(), None);
    let caller = Caller {
        ip: "203.0.113.2".parse().unwrap(),
        now: Instant::now(),
    };
    let created = leaving
        .create("leaving@example.com", "correct horse battery", caller)
        .await
        .unwrap();
    accounts.push(created.account.id);
    let at = now();
    for &account in &accounts {
        finish_at(&db, account, "a", "tmdb-603", at).await;
    }
    let session = leaving.session(&created.token).await.unwrap().unwrap();

    leaving.delete(&session).await.unwrap();

    assert!(viewing.list(created.account.id).await.unwrap().is_empty());
    assert_eq!(count_of(&viewing, "tmdb-603").await, Some(20));
    assert_eq!(trending_ids(&viewing).await, ["tmdb-603"]);
}
