use std::{future, sync::Arc, time::Instant};

use sqlx::PgPool;
use tokio::sync::mpsc;

use super::{WatchStateChange, WatchStates, merge};
use crate::{
    accounts::{AccountId, AccountSession, Accounts, Caller},
    db,
    links::{LinkedServer, Links, Polled},
    mail::{Email, Mail, Mailer, Sending},
    signing::Keys,
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
