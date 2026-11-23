use std::time::Instant;

use sqlx::PgPool;

use super::{RECENT_AUTHENTICATION_MINS, browser_name};
use crate::{
    accounts::{Accounts, Caller, Error},
    db,
};

#[sqlx::test(migrations = false)]
async fn a_session_signed_in_over_10_minutes_ago_cant_set_the_password(db: PgPool) {
    db::migrate(&db).await.unwrap();
    let accounts = Accounts::new(db.clone(), None, None);
    let ip = "203.0.113.1".parse().unwrap();
    let created = accounts
        .create(
            "ada@example.com",
            "correct horse battery",
            Caller {
                ip,
                now: Instant::now(),
            },
        )
        .await
        .unwrap();
    let fresh = accounts.session(&created.token).await.unwrap().unwrap();
    accounts
        .set_password(&fresh, "staple battery horse")
        .await
        .unwrap();

    sqlx::query!(
        "UPDATE account_sessions SET created_at = now() - make_interval(mins => $1)",
        RECENT_AUTHENTICATION_MINS,
    )
    .execute(&db)
    .await
    .unwrap();
    let stale = accounts.session(&created.token).await.unwrap().unwrap();

    // Refused before the password is looked at, so even one too short gets this refusal.
    assert!(matches!(
        accounts.set_password(&stale, "short").await,
        Err(Error::RecentAuthenticationRequired(_))
    ));
    accounts
        .sign_in(
            "ada@example.com",
            "staple battery horse",
            None,
            Caller {
                ip,
                now: Instant::now(),
            },
        )
        .await
        .unwrap();
}

#[sqlx::test(migrations = false)]
async fn a_session_ended_before_its_password_is_saved_cant_set_it(db: PgPool) {
    db::migrate(&db).await.unwrap();
    let accounts = Accounts::new(db, None, None);
    let ip = "203.0.113.1".parse().unwrap();
    let created = accounts
        .create(
            "ada@example.com",
            "correct horse battery",
            Caller {
                ip,
                now: Instant::now(),
            },
        )
        .await
        .unwrap();
    let session = accounts.session(&created.token).await.unwrap().unwrap();

    // As a Sign-in Link's takeover would, while the password hashes.
    accounts.sign_out(&session).await.unwrap();

    assert!(matches!(
        accounts
            .set_password(&session, "staple battery horse")
            .await,
        Err(Error::RecentAuthenticationRequired(_))
    ));
    accounts
        .sign_in(
            "ada@example.com",
            "correct horse battery",
            None,
            Caller {
                ip,
                now: Instant::now(),
            },
        )
        .await
        .unwrap();
}

#[sqlx::test(migrations = false)]
async fn only_a_session_signed_in_within_10_minutes_deletes_the_account(db: PgPool) {
    db::migrate(&db).await.unwrap();
    let accounts = Accounts::new(db.clone(), None, None);
    let caller = || Caller {
        ip: "203.0.113.1".parse().unwrap(),
        now: Instant::now(),
    };
    let password = "correct horse battery";
    let created = accounts
        .create("ada@example.com", password, caller())
        .await
        .unwrap();
    sqlx::query!(
        "UPDATE account_sessions SET created_at = now() - make_interval(mins => $1)",
        RECENT_AUTHENTICATION_MINS,
    )
    .execute(&db)
    .await
    .unwrap();
    let stale = accounts.session(&created.token).await.unwrap().unwrap();

    assert!(matches!(
        accounts.delete(&stale).await,
        Err(Error::RecentAuthenticationRequired(_))
    ));
    let signed_in = accounts
        .sign_in("ada@example.com", password, None, caller())
        .await
        .unwrap();
    let fresh = accounts.session(&signed_in.token).await.unwrap().unwrap();
    accounts.delete(&fresh).await.unwrap();

    assert!(matches!(
        accounts
            .sign_in("ada@example.com", password, None, caller())
            .await,
        Err(Error::WrongEmailOrPassword)
    ));
    assert!(accounts.session(&signed_in.token).await.unwrap().is_none());
}

#[sqlx::test(migrations = false)]
async fn only_a_session_signed_in_within_10_minutes_signs_out_the_others(db: PgPool) {
    db::migrate(&db).await.unwrap();
    let accounts = Accounts::new(db.clone(), None, None);
    let caller = || Caller {
        ip: "203.0.113.1".parse().unwrap(),
        now: Instant::now(),
    };
    let password = "correct horse battery";
    let created = accounts
        .create("ada@example.com", password, caller())
        .await
        .unwrap();
    let other = accounts
        .sign_in("ada@example.com", password, None, caller())
        .await
        .unwrap();
    sqlx::query!(
        "UPDATE account_sessions SET created_at = now() - make_interval(mins => $1)",
        RECENT_AUTHENTICATION_MINS,
    )
    .execute(&db)
    .await
    .unwrap();
    let stale = accounts.session(&created.token).await.unwrap().unwrap();
    let listed = accounts.sessions(&stale).await.unwrap();
    let other_id = listed.iter().find(|listed| !listed.is_current).unwrap().id;

    assert!(matches!(
        accounts.sign_out_session(&stale, other_id).await,
        Err(Error::RecentAuthenticationRequired(_))
    ));
    assert!(matches!(
        accounts.sign_out_other_sessions(&stale).await,
        Err(Error::RecentAuthenticationRequired(_))
    ));
    assert!(accounts.session(&other.token).await.unwrap().is_some());
}

#[test]
fn a_session_is_named_after_its_browser_and_os() {
    let cases = [
        (
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) \
             Chrome/131.0.0.0 Safari/537.36",
            "Chrome on macOS",
        ),
        (
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) \
             Chrome/131.0.0.0 Safari/537.36 Edg/131.0.0.0",
            "Edge on Windows",
        ),
        (
            "Mozilla/5.0 (X11; Linux x86_64; rv:133.0) Gecko/20100101 Firefox/133.0",
            "Firefox on Linux",
        ),
        (
            "Mozilla/5.0 (iPhone; CPU iPhone OS 18_1 like Mac OS X) AppleWebKit/605.1.15 \
             (KHTML, like Gecko) Version/18.1 Mobile/15E148 Safari/604.1",
            "Safari on iPhone",
        ),
        (
            "Mozilla/5.0 (Linux; Android 15; Pixel 9) AppleWebKit/537.36 (KHTML, like Gecko) \
             Chrome/131.0.0.0 Mobile Safari/537.36",
            "Chrome on Android",
        ),
        ("curl/8.11.0", "Unknown browser"),
    ];
    for (user_agent, name) in cases {
        assert_eq!(browser_name(Some(user_agent)), name, "{user_agent}");
    }
    assert_eq!(browser_name(None), "Unknown browser");
}
