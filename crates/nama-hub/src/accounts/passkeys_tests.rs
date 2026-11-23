use std::time::Instant;

use sqlx::PgPool;
use url::Url;
use uuid::Uuid;

use super::can_sign_in;
use crate::accounts::{Account, AccountId, AccountSession, Accounts, Error, NewPasskey};

/// Accounts whose database is never reached: each call here fails before it would be.
fn accounts(public_url: Option<&str>) -> Accounts {
    let db = PgPool::connect_lazy("postgres://nobody@localhost/never").unwrap();
    Accounts::new(
        db,
        None,
        public_url.map(|url| Url::parse(url).unwrap()).as_ref(),
    )
}

fn session(recent: bool) -> AccountSession {
    AccountSession {
        account: Account {
            id: AccountId(Uuid::nil()),
            email: "ada@example.com".to_owned(),
        },
        email_proven: false,
        authenticated_recently: recent,
        id: Uuid::nil(),
    }
}

#[test]
fn an_account_keeps_a_way_to_sign_in_with_a_password_a_passkey_or_sign_in_links() {
    assert!(can_sign_in(true, 0, false));
    assert!(can_sign_in(false, 1, false));
    assert!(
        can_sign_in(false, 0, true),
        "a Sign-in Link, while mail is set up"
    );
    assert!(!can_sign_in(false, 0, false));
}

#[tokio::test]
async fn adding_or_removing_a_passkey_needs_a_recent_sign_in_before_anything_else() {
    let accounts = accounts(Some("https://hub.example"));
    let stale = session(false);
    let passkey = NewPasskey {
        name: "",
        credential_id: b"",
        client_data_json: b"",
        public_key: b"",
        algorithm: 0,
    };

    for refused in [
        accounts
            .start_adding_passkey(&stale, Instant::now())
            .await
            .map(drop),
        accounts
            .add_passkey(&stale, &passkey, Instant::now())
            .await
            .map(drop),
        accounts.remove_passkey(&stale, Uuid::nil()).await,
    ] {
        assert!(
            matches!(refused, Err(Error::RecentAuthenticationRequired)),
            "{refused:?}"
        );
    }
}

#[tokio::test]
async fn without_a_public_url_there_are_no_passkeys() {
    let accounts = accounts(None);

    assert!(matches!(
        accounts.start_passkey_sign_in(Instant::now()),
        Err(Error::PasskeysOff)
    ));
    assert!(matches!(
        accounts
            .start_adding_passkey(&session(true), Instant::now())
            .await,
        Err(Error::PasskeysOff)
    ));
}
