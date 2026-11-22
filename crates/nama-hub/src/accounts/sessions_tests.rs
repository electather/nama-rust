use std::time::Instant;

use sqlx::PgPool;

use super::RECENT_MINUTES;
use crate::{
    accounts::{Accounts, Error},
    db,
};

#[sqlx::test(migrations = false)]
async fn a_session_signed_in_over_10_minutes_ago_cant_set_the_password(db: PgPool) {
    db::migrate(&db).await.unwrap();
    let accounts = Accounts::new(db.clone(), None);
    let ip = "203.0.113.1".parse().unwrap();
    let created = accounts
        .create(
            "ada@example.com",
            "correct horse battery",
            ip,
            Instant::now(),
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
        RECENT_MINUTES,
    )
    .execute(&db)
    .await
    .unwrap();
    let stale = accounts.session(&created.token).await.unwrap().unwrap();

    // Refused before the password is looked at, so even one too short gets this refusal.
    assert!(matches!(
        accounts.set_password(&stale, "short").await,
        Err(Error::RecentAuthenticationRequired)
    ));
    accounts
        .sign_in(
            "ada@example.com",
            "staple battery horse",
            ip,
            Instant::now(),
        )
        .await
        .unwrap();
}
