use std::time::Instant;

use sqlx::PgPool;

use super::{CONTINUE_TO_LENGTH, LINK_MINUTES, find, hub_path, issue, use_up};
use crate::{
    accounts::{Accounts, Caller, Error},
    db,
};

#[test]
fn a_sign_in_continues_only_at_a_path_on_the_hub() {
    let longest = format!("/{}", "a".repeat(CONTINUE_TO_LENGTH - 1));
    for path in ["/", "/servers/1?accept=yes#top", longest.as_str()] {
        assert_eq!(hub_path(path), path);
    }
    let too_long = format!("{longest}a");
    for path in [
        "",
        "servers",
        "https://example.com/",
        "//example.com",
        "/\\example.com",
        "/\t/example.com",
        too_long.as_str(),
    ] {
        assert_eq!(hub_path(path), "/", "{path:?}");
    }
}

#[sqlx::test(migrations = false)]
async fn a_link_older_than_15_minutes_neither_names_its_account_nor_signs_in(db: PgPool) {
    db::migrate(&db).await.unwrap();
    Accounts::new(db.clone(), None)
        .create(
            "ada@example.com",
            "correct horse battery",
            Caller {
                ip: "203.0.113.1".parse().unwrap(),
                now: Instant::now(),
            },
        )
        .await
        .unwrap();
    let (_, token) = issue(&db, "ADA@example.com", "/").await.unwrap().unwrap();
    find(&db, &token).await.unwrap();

    sqlx::query!(
        "UPDATE sign_in_links SET created_at = now() - make_interval(mins => $1)",
        LINK_MINUTES,
    )
    .execute(&db)
    .await
    .unwrap();

    assert!(matches!(
        find(&db, &token).await,
        Err(Error::SignInLinkExpired)
    ));
    assert!(matches!(
        use_up(&db, &token, None).await,
        Err(Error::SignInLinkExpired)
    ));
}
