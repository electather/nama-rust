use std::{future, sync::Arc, time::Instant};

use sqlx::PgPool;
use tokio::sync::mpsc;

use super::{WatchStateChange, WatchStates, WatchlistEntry, Watchlists, merge, merge_watchlist};
use crate::{
    accounts::{AccountId, AccountSession, Accounts, Caller},
    db,
    links::{LinkedServer, Links, Polled},
    mail::{Email, Mail, Mailer, Sending},
    signing::Keys,
    viewing::Viewing,
};

/// A signing key made with `openssl genpkey -algorithm ed25519`, which Linking a Server needs.
const SIGNING_KEY: &str = "-----BEGIN PRIVATE KEY-----
MC4CAQAwBQYDK2VwBCIEIJzJDsQn3SpQXPoFGl74lUI4L+huggYWuNcSt9vaCwTS
-----END PRIVATE KEY-----
";

/// Mail that hands each email to the test.
struct Inbox(mpsc::UnboundedSender<Email>);

impl Mailer for Inbox {
    fn send(&self, email: Email) -> Sending<'_> {
        let _unread = self.0.send(email);
        Box::pin(future::ready(Ok(())))
    }
}

/// The Hub's areas on a migrated `db`, and the emails it sends.
struct Hub {
    accounts: Accounts,
    links: Links,
    watch_states: WatchStates,
    watchlists: Watchlists,
    sent: mpsc::UnboundedReceiver<Email>,
}

impl Hub {
    async fn on(db: &PgPool) -> Self {
        db::migrate(db).await.unwrap();
        let (inbox, sent) = mpsc::unbounded_channel();
        let mail = Mail {
            mailer: Arc::new(Inbox(inbox)),
            public_url: "https://hub.example.com".to_owned(),
        };
        let accounts = Accounts::new(db.clone(), Some(mail));
        let keys = Arc::new(Keys::from_pems(SIGNING_KEY).unwrap());
        let links = Links::new(db.clone(), accounts.clone(), keys);
        Self {
            watch_states: WatchStates::new(db.clone(), links.clone()),
            watchlists: Watchlists::new(
                db.clone(),
                links.clone(),
                Viewing::new(db.clone(), links.clone()),
            ),
            accounts,
            links,
            sent,
        }
    }

    /// A session of a new Account with `email`, proven by a Sign-in Link, as Linking needs.
    async fn proven(&mut self, email: &str, ip: &str) -> AccountSession {
        let caller = || Caller {
            ip: ip.parse().unwrap(),
            now: Instant::now(),
        };
        let password = "correct horse battery";
        self.accounts
            .create(email, password, caller())
            .await
            .unwrap();
        (self.accounts)
            .send_sign_in_link(email, "/", caller())
            .unwrap();
        let sent = self.sent.recv().await.unwrap();
        let (_, token) = sent.text.split_once("/sign-in-link#").unwrap();
        let token = token.split_whitespace().next().unwrap();
        let signed_in = self.accounts.sign_in_with_link(token, None).await.unwrap();
        let session = self.accounts.session(&signed_in.signed_in.token).await;
        session.unwrap().unwrap()
    }

    /// A Server Linked to `session`'s Account.
    async fn linked_server(&self, session: &AccountSession) -> LinkedServer {
        let started = self.links.start("Living room").await.unwrap();
        self.links
            .approve(session, &started.user_code)
            .await
            .unwrap();
        let Polled::Linked { credential, .. } =
            self.links.poll(&started.device_code).await.unwrap()
        else {
            panic!("the Server should be Linked");
        };
        self.links.server(&credential).await.unwrap().unwrap()
    }

    /// Links `server`'s User `user_id` to `session`'s Account.
    async fn link_user(&self, server: &LinkedServer, user_id: i64, session: &AccountSession) {
        let code = self
            .links
            .start_user(server, user_id, "Grace")
            .await
            .unwrap();
        self.links.approve_user(session, &code).await.unwrap();
    }
}

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

/// Merges each change, in order, into `account`'s Watch State.
async fn merge_each(db: &PgPool, account: AccountId, changes: &[WatchStateChange]) {
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

/// How many Watch State rows every Account has together.
async fn rows(db: &PgPool) -> i64 {
    sqlx::query_scalar!(r#"SELECT count(*) AS "rows!" FROM watch_states"#)
        .fetch_one(db)
        .await
        .unwrap()
}

#[sqlx::test(migrations = false)]
async fn two_servers_conflicting_changes_end_with_the_latest_whichever_arrives_last(db: PgPool) {
    let mut hub = Hub::on(&db).await;
    let ada = hub.proven("ada@example.com", "203.0.113.1").await;
    let (first, second) = (hub.linked_server(&ada).await, hub.linked_server(&ada).await);
    hub.link_user(&first, 7, &ada).await;
    hub.link_user(&second, 3, &ada).await;
    let watch_states = &hub.watch_states;

    let made_later = alien_at(7, 3000, 2_000);
    watch_states.push(&first, vec![made_later]).await.unwrap();
    let made_earlier = alien_at(3, 1000, 1_000);
    watch_states
        .push(&second, vec![made_earlier])
        .await
        .unwrap();

    let account = ada.account.id;
    assert_eq!(
        kept(&db, account, "tmdb-348").await,
        Some((false, 3000, 7000))
    );
    assert_eq!(
        kept(&db, account, "imdb-tt0078748").await,
        Some((false, 3000, 7000))
    );

    let latest = alien_at(3, 5000, 3_000);
    watch_states.push(&second, vec![latest]).await.unwrap();

    assert_eq!(
        kept(&db, account, "tmdb-348").await,
        Some((false, 5000, 7000))
    );
}

#[sqlx::test(migrations = false)]
async fn a_push_reaches_only_the_account_each_user_is_linked_to(db: PgPool) {
    let mut hub = Hub::on(&db).await;
    let ada = hub.proven("ada@example.com", "203.0.113.1").await;
    let bea = hub.proven("bea@example.com", "203.0.113.2").await;
    let server = hub.linked_server(&ada).await;
    hub.link_user(&server, 7, &ada).await;
    hub.link_user(&server, 8, &bea).await;

    let changes = vec![
        alien_at(7, 1000, 1_000),
        alien_at(8, 2000, 1_000),
        alien_at(9, 3000, 1_000),
    ];
    hub.watch_states.push(&server, changes).await.unwrap();

    let (ada, bea) = (ada.account.id, bea.account.id);
    assert_eq!(kept(&db, ada, "tmdb-348").await, Some((false, 1000, 7000)));
    assert_eq!(kept(&db, bea, "tmdb-348").await, Some((false, 2000, 7000)));
    assert_eq!(
        rows(&db).await,
        4,
        "two ids each for Ada and Bea, none for User 9"
    );
}

#[sqlx::test(migrations = false)]
async fn marking_unplayed_later_wins_over_played(db: PgPool) {
    let account = account(&db).await;
    let played = WatchStateChange {
        played: true,
        ..alien_at(7, 7000, 1_000)
    };
    let unplayed = WatchStateChange {
        duration_ms: 0,
        ..alien_at(7, 0, 2_000)
    };

    merge_each(&db, account, &[played, unplayed]).await;

    assert_eq!(kept(&db, account, "tmdb-348").await, Some((false, 0, 0)));
}

#[sqlx::test(migrations = false)]
async fn a_change_naming_an_id_twice_keeps_it_once(db: PgPool) {
    let account = account(&db).await;
    let twice = WatchStateChange {
        ids: vec!["tmdb-348".to_owned(), "tmdb-348".to_owned()],
        ..alien_at(7, 3000, 1_000)
    };

    merge_each(&db, account, &[twice]).await;

    assert_eq!(
        kept(&db, account, "tmdb-348").await,
        Some((false, 3000, 7000))
    );
}

/// What `server`'s User `user_id` pulls: each change's User, id, position and when it was made.
async fn pulled(
    watch_states: &WatchStates,
    server: &LinkedServer,
    user_id: i64,
) -> Vec<(i64, Vec<String>, i64, i64)> {
    let pulled = watch_states.pull(server, user_id).await.unwrap();
    let mut pulled: Vec<_> = (pulled.into_iter())
        .map(|change| {
            (
                change.user_id,
                change.ids,
                change.position_ms,
                change.changed_at_ms,
            )
        })
        .collect();
    pulled.sort();
    pulled
}

#[sqlx::test(migrations = false)]
async fn a_server_pulls_what_another_server_of_the_account_pushed_by_each_id(db: PgPool) {
    let mut hub = Hub::on(&db).await;
    let ada = hub.proven("ada@example.com", "203.0.113.1").await;
    let (first, second) = (hub.linked_server(&ada).await, hub.linked_server(&ada).await);
    hub.link_user(&first, 7, &ada).await;
    hub.link_user(&second, 3, &ada).await;

    let made_at = 1_700_000_000_123;
    let pushed = alien_at(7, 3000, made_at);
    hub.watch_states.push(&first, vec![pushed]).await.unwrap();

    assert_eq!(
        pulled(&hub.watch_states, &second, 3).await,
        [
            (3, vec!["imdb-tt0078748".to_owned()], 3000, made_at),
            (3, vec!["tmdb-348".to_owned()], 3000, made_at),
        ]
    );
}

#[sqlx::test(migrations = false)]
async fn a_pull_holds_only_the_account_the_user_is_linked_to(db: PgPool) {
    let mut hub = Hub::on(&db).await;
    let ada = hub.proven("ada@example.com", "203.0.113.1").await;
    let bea = hub.proven("bea@example.com", "203.0.113.2").await;
    let server = hub.linked_server(&ada).await;
    hub.link_user(&server, 7, &ada).await;
    hub.link_user(&server, 8, &bea).await;
    let changes = vec![alien_at(7, 1000, 1_000), alien_at(8, 2000, 1_000)];
    hub.watch_states.push(&server, changes).await.unwrap();

    let ada_pulled = pulled(&hub.watch_states, &server, 7).await;
    let positions: Vec<_> = ada_pulled
        .iter()
        .map(|&(_, _, position, _)| position)
        .collect();
    assert_eq!(positions, [1000, 1000]);
    assert_eq!(
        pulled(&hub.watch_states, &server, 9).await,
        [],
        "User 9 isn't Linked"
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

/// What `server`'s User `user_id` gets back syncing `entries`, in order of id.
async fn synced(
    watchlists: &Watchlists,
    server: &LinkedServer,
    user_id: i64,
    entries: Vec<WatchlistEntry>,
) -> Vec<WatchlistEntry> {
    let mut merged = watchlists.sync(server, user_id, entries).await.unwrap();
    merged.sort_by(|a, b| a.id.cmp(&b.id));
    merged
}

#[sqlx::test(migrations = false)]
async fn a_watchlist_change_made_last_wins_removals_included_whichever_server_syncs_last(
    db: PgPool,
) {
    let mut hub = Hub::on(&db).await;
    let ada = hub.proven("ada@example.com", "203.0.113.1").await;
    let (first, second) = (hub.linked_server(&ada).await, hub.linked_server(&ada).await);
    hub.link_user(&first, 7, &ada).await;
    hub.link_user(&second, 3, &ada).await;
    let watchlists = &hub.watchlists;

    synced(watchlists, &first, 7, vec![the_matrix(true, 2_000)]).await;
    let added_earlier = vec![the_matrix(false, 1_000)];

    assert_eq!(
        synced(watchlists, &second, 3, added_earlier).await,
        [the_matrix(true, 2_000)]
    );
    let added_later = vec![the_matrix(false, 3_000)];
    assert_eq!(
        synced(watchlists, &second, 3, added_later).await,
        [the_matrix(false, 3_000)]
    );
    assert_eq!(
        synced(watchlists, &first, 7, vec![the_matrix(true, 2_000)]).await,
        [the_matrix(false, 3_000)]
    );
}

#[sqlx::test(migrations = false)]
async fn a_watchlist_keeps_any_id_a_server_sends_and_of_one_named_twice_the_latest(db: PgPool) {
    let mut hub = Hub::on(&db).await;
    let ada = hub.proven("ada@example.com", "203.0.113.1").await;
    let server = hub.linked_server(&ada).await;
    hub.link_user(&server, 7, &ada).await;
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
        synced(&hub.watchlists, &server, 7, twice).await,
        [the_matrix(true, 2_000), dark]
    );
}

#[sqlx::test(migrations = false)]
async fn of_two_watchlist_changes_made_at_the_same_moment_the_removal_wins(db: PgPool) {
    let mut hub = Hub::on(&db).await;
    let ada = hub.proven("ada@example.com", "203.0.113.1").await;
    let (first, second) = (hub.linked_server(&ada).await, hub.linked_server(&ada).await);
    hub.link_user(&first, 7, &ada).await;
    hub.link_user(&second, 3, &ada).await;
    let watchlists = &hub.watchlists;

    synced(watchlists, &first, 7, vec![the_matrix(false, 1_000)]).await;
    let removed_at_once = vec![the_matrix(true, 1_000)];

    assert_eq!(
        synced(watchlists, &second, 3, removed_at_once).await,
        [the_matrix(true, 1_000)]
    );
    assert_eq!(
        synced(watchlists, &first, 7, vec![the_matrix(false, 1_000)]).await,
        [the_matrix(true, 1_000)]
    );
    let both = vec![the_matrix(false, 2_000), the_matrix(true, 2_000)];
    assert_eq!(
        synced(watchlists, &first, 7, both).await,
        [the_matrix(true, 2_000)]
    );
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
    let keys = Arc::new(Keys::from_pems(SIGNING_KEY).unwrap());
    let links = Links::new(db.clone(), Accounts::new(db.clone(), None), keys);
    let viewing = Viewing::new(db.clone(), links.clone());
    let watchlists = Watchlists::new(db.clone(), links, viewing.clone());
    (watchlists, viewing, created)
}

/// Puts `entries` on `account`'s Watchlist.
async fn watchlist(db: &PgPool, account: AccountId, entries: Vec<WatchlistEntry>) {
    let mut conn = db.acquire().await.unwrap();
    merge_watchlist(&mut conn, account, entries).await.unwrap();
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
        watchlist(&db, account, entries).await;
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
        watchlist(&db, account, vec![the_matrix(false, 1_000)]).await;
    }
    assert_eq!(
        most_watchlisted_ids(&watchlists).await,
        [(false, "tmdb-603".to_owned())]
    );

    viewing.set_pooling(accounts[0], false).await.unwrap();

    assert_eq!(most_watchlisted_ids(&watchlists).await, []);
}
