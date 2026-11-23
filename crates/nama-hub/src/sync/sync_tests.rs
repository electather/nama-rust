use std::time::Instant;

use sqlx::PgPool;

use super::{WatchStateChange, WatchStates, WatchlistEntry, Watchlists};
use crate::{
    accounts::{AccountId, AccountSession, Accounts, Caller},
    db,
    viewing::Viewing,
};

/// Watch State and Watchlists on a migrated `db`, and the Accounts they keep.
struct Hub {
    accounts: Accounts,
    watch_states: WatchStates,
    watchlists: Watchlists,
}

impl Hub {
    async fn on(db: &PgPool) -> Self {
        db::migrate(db).await.unwrap();
        Self {
            accounts: Accounts::new(db.clone(), None),
            watch_states: WatchStates::new(db.clone()),
            watchlists: Watchlists::new(db.clone(), Viewing::new(db.clone())),
        }
    }

    /// A session of a new Account with `email`.
    async fn account(&self, email: &str) -> AccountSession {
        let caller = Caller {
            ip: "203.0.113.1".parse().unwrap(),
            now: Instant::now(),
        };
        let created = (self.accounts)
            .create(email, "correct horse battery", caller)
            .await
            .unwrap();
        let session = self.accounts.session(&created.token).await;
        session.unwrap().unwrap()
    }

    /// Merges `changes` into `account`'s Watch State, in one push.
    async fn merge(&self, account: AccountId, changes: Vec<WatchStateChange>) {
        let changes = changes
            .into_iter()
            .map(|change| (account, change))
            .collect();
        self.watch_states.merge(changes).await.unwrap();
    }
}

/// The User `user_id`'s Watch State of Alien at `position_ms` of 7000, changed at `changed_at_ms`.
fn alien_at(user_id: i64, position_ms: i64, changed_at_ms: i64) -> WatchStateChange {
    WatchStateChange {
        user_id,
        episode: false,
        ids: vec!["tmdb-348".to_owned(), "imdb-tt0078748".to_owned()],
        played: false,
        position_ms,
        duration_ms: 7000,
        changed_at_ms,
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

/// How many Watch State rows every Account has together.
async fn rows(db: &PgPool) -> i64 {
    sqlx::query_scalar!(r#"SELECT count(*) AS "rows!" FROM watch_states"#)
        .fetch_one(db)
        .await
        .unwrap()
}

/// How many Watchlist entries every Account has together.
async fn watchlist_rows(db: &PgPool) -> i64 {
    sqlx::query_scalar!(r#"SELECT count(*) AS "rows!" FROM watchlists"#)
        .fetch_one(db)
        .await
        .unwrap()
}

#[sqlx::test(migrations = false)]
async fn conflicting_changes_end_with_the_latest_whichever_arrives_last(db: PgPool) {
    let hub = Hub::on(&db).await;
    let account = hub.account("ada@example.com").await.account.id;

    hub.merge(account, vec![alien_at(7, 3000, 2_000)]).await;
    hub.merge(account, vec![alien_at(3, 1000, 1_000)]).await;

    assert_eq!(
        kept(&db, account, "tmdb-348").await,
        Some((false, 3000, 7000))
    );
    assert_eq!(
        kept(&db, account, "imdb-tt0078748").await,
        Some((false, 3000, 7000))
    );

    hub.merge(account, vec![alien_at(3, 5000, 3_000)]).await;

    assert_eq!(
        kept(&db, account, "tmdb-348").await,
        Some((false, 5000, 7000))
    );
}

#[sqlx::test(migrations = false)]
async fn marking_unplayed_later_wins_over_played(db: PgPool) {
    let hub = Hub::on(&db).await;
    let account = hub.account("ada@example.com").await.account.id;
    let played = WatchStateChange {
        played: true,
        ..alien_at(7, 7000, 1_000)
    };
    let unplayed = WatchStateChange {
        duration_ms: 0,
        ..alien_at(7, 0, 2_000)
    };

    hub.merge(account, vec![played, unplayed]).await;

    assert_eq!(kept(&db, account, "tmdb-348").await, Some((false, 0, 0)));
}

#[sqlx::test(migrations = false)]
async fn a_change_naming_an_id_twice_keeps_it_once(db: PgPool) {
    let hub = Hub::on(&db).await;
    let account = hub.account("ada@example.com").await.account.id;
    let twice = WatchStateChange {
        ids: vec!["tmdb-348".to_owned(), "tmdb-348".to_owned()],
        ..alien_at(7, 3000, 1_000)
    };

    hub.merge(account, vec![twice]).await;

    assert_eq!(
        kept(&db, account, "tmdb-348").await,
        Some((false, 3000, 7000))
    );
}

/// The Watchlist entry of the Movie `tmdb-603`, taken off or not, changed at `changed_at_ms`.
fn the_matrix(removed: bool, changed_at_ms: i64) -> WatchlistEntry {
    WatchlistEntry {
        show: false,
        id: "tmdb-603".to_owned(),
        removed,
        changed_at_ms,
    }
}

/// What `account` gets back syncing `entries`, in order of id.
async fn synced(
    watchlists: &Watchlists,
    account: AccountId,
    entries: Vec<WatchlistEntry>,
) -> Vec<WatchlistEntry> {
    let mut merged = watchlists.sync_account(account, entries).await.unwrap();
    merged.sort_by(|a, b| a.id.cmp(&b.id));
    merged
}

#[sqlx::test(migrations = false)]
async fn a_watchlist_change_made_last_wins_removals_included_whichever_syncs_last(db: PgPool) {
    let hub = Hub::on(&db).await;
    let account = hub.account("ada@example.com").await.account.id;
    let watchlists = &hub.watchlists;

    synced(watchlists, account, vec![the_matrix(true, 2_000)]).await;
    let added_earlier = vec![the_matrix(false, 1_000)];

    assert_eq!(
        synced(watchlists, account, added_earlier).await,
        [the_matrix(true, 2_000)]
    );
    let added_later = vec![the_matrix(false, 3_000)];
    assert_eq!(
        synced(watchlists, account, added_later).await,
        [the_matrix(false, 3_000)]
    );
    assert_eq!(
        synced(watchlists, account, vec![the_matrix(true, 2_000)]).await,
        [the_matrix(false, 3_000)]
    );
}

#[sqlx::test(migrations = false)]
async fn a_watchlist_keeps_any_id_a_server_sends_and_of_one_named_twice_the_latest(db: PgPool) {
    let hub = Hub::on(&db).await;
    let account = hub.account("ada@example.com").await.account.id;
    let dark = WatchlistEntry {
        show: true,
        id: "tvdb-81189".to_owned(),
        removed: false,
        changed_at_ms: 1_000,
    };
    let twice = vec![
        the_matrix(false, 1_000),
        the_matrix(true, 2_000),
        dark.clone(),
    ];

    assert_eq!(
        synced(&hub.watchlists, account, twice).await,
        [the_matrix(true, 2_000), dark]
    );
}

#[sqlx::test(migrations = false)]
async fn of_two_watchlist_changes_made_at_the_same_moment_the_removal_wins(db: PgPool) {
    let hub = Hub::on(&db).await;
    let account = hub.account("ada@example.com").await.account.id;
    let watchlists = &hub.watchlists;

    synced(watchlists, account, vec![the_matrix(false, 1_000)]).await;
    let removed_at_once = vec![the_matrix(true, 1_000)];

    assert_eq!(
        synced(watchlists, account, removed_at_once).await,
        [the_matrix(true, 1_000)]
    );
    assert_eq!(
        synced(watchlists, account, vec![the_matrix(false, 1_000)]).await,
        [the_matrix(true, 1_000)]
    );
    let both = vec![the_matrix(false, 2_000), the_matrix(true, 2_000)];
    assert_eq!(
        synced(watchlists, account, both).await,
        [the_matrix(true, 2_000)]
    );
}

#[sqlx::test(migrations = false)]
async fn deleting_an_account_deletes_its_watch_state_and_watchlist(db: PgPool) {
    let hub = Hub::on(&db).await;
    let ada = hub.account("ada@example.com").await;
    let bea = hub.account("bea@example.com").await;
    hub.merge(ada.account.id, vec![alien_at(7, 1000, 1_000)])
        .await;
    hub.merge(bea.account.id, vec![alien_at(8, 2000, 1_000)])
        .await;
    synced(
        &hub.watchlists,
        bea.account.id,
        vec![the_matrix(false, 1_000)],
    )
    .await;

    hub.accounts.delete(&bea).await.unwrap();

    assert_eq!(rows(&db).await, 2, "Ada's Watch State of Alien, by each id");
    assert_eq!(watchlist_rows(&db).await, 0);
    let ada_pulled = hub.watch_states.pull_account(ada.account.id, 7).await;
    assert_eq!(ada_pulled.unwrap().len(), 2);
}

/// Watchlists on a migrated `db`, the Viewing whose pooling setting they count by, and `count` new
/// Accounts.
async fn accounts_on(db: &PgPool, count: usize) -> (Watchlists, Viewing, Vec<AccountId>) {
    db::migrate(db).await.unwrap();
    let mut created = Vec::new();
    for n in 0..count {
        // A fresh `Accounts` each, so its per-IP limit on creations never refuses one.
        let caller = Caller {
            ip: "203.0.113.1".parse().unwrap(),
            now: Instant::now(),
        };
        let account = Accounts::new(db.clone(), None)
            .create(
                &format!("watcher{n}@example.com"),
                "correct horse battery",
                caller,
            )
            .await;
        created.push(account.unwrap().account.id);
    }
    let viewing = Viewing::new(db.clone());
    let watchlists = Watchlists::new(db.clone(), viewing.clone());
    (watchlists, viewing, created)
}

/// The most watchlisted titles, each as whether it is a Show and its External ID.
async fn most_watchlisted_ids(watchlists: &Watchlists) -> Vec<(bool, String)> {
    let watchlisted = watchlists.most_watchlisted().await.unwrap();
    (watchlisted.into_iter())
        .map(|title| (title.show, title.external_id))
        .collect()
}

#[sqlx::test(migrations = false)]
async fn titles_on_twenty_accounts_watchlists_are_most_watchlisted_first_and_removals_dont_count(
    db: PgPool,
) {
    let (watchlists, _viewing, accounts) = accounts_on(&db, 21).await;
    let breaking_bad = WatchlistEntry {
        show: true,
        id: "tvdb-81189".to_owned(),
        removed: false,
        changed_at_ms: 1_000,
    };
    let alien = WatchlistEntry {
        id: "tmdb-348".to_owned(),
        ..the_matrix(false, 1_000)
    };
    for (n, &account) in accounts.iter().enumerate() {
        let mut entries = vec![the_matrix(n == 0, 1_000), breaking_bad.clone()];
        if n < 19 {
            entries.push(alien.clone());
        }
        synced(&watchlists, account, entries).await;
    }

    let watchlisted = watchlists.most_watchlisted().await.unwrap();

    let ids: Vec<(bool, &str)> = (watchlisted.iter())
        .map(|title| (title.show, title.external_id.as_str()))
        .collect();
    assert_eq!(ids, [(true, "tvdb-81189"), (false, "tmdb-603")]);
    assert!((watchlisted[0].weight - 1.0).abs() < 1e-6);
    assert!((watchlisted[1].weight - 20.0 / 21.0).abs() < 1e-6);
}

#[sqlx::test(migrations = false)]
async fn an_account_with_pooling_off_isnt_counted_in_the_most_watchlisted(db: PgPool) {
    let (watchlists, viewing, accounts) = accounts_on(&db, 20).await;
    for &account in &accounts {
        synced(&watchlists, account, vec![the_matrix(false, 1_000)]).await;
    }
    assert_eq!(
        most_watchlisted_ids(&watchlists).await,
        [(false, "tmdb-603".to_owned())]
    );

    viewing.set_pooling(accounts[0], false).await.unwrap();

    assert_eq!(most_watchlisted_ids(&watchlists).await, []);
}
