use std::time::Instant;

use super::*;
use crate::{accounts::Caller, probe::Tcp};

/// A signing key, made with `openssl genpkey -algorithm ed25519`.
const SIGNING_KEY: &str = "-----BEGIN PRIVATE KEY-----
MC4CAQAwBQYDK2VwBCIEIJzJDsQn3SpQXPoFGl74lUI4L+huggYWuNcSt9vaCwTS
-----END PRIVATE KEY-----
";

#[test]
fn a_code_is_eight_letters_from_the_alphabet() {
    let code = random_code().unwrap();

    assert_eq!(code.len(), CODE_LENGTH);
    assert!(
        code.bytes().all(|letter| CODE_LETTERS.contains(&letter)),
        "{code}"
    );
}

#[test]
fn a_code_typed_any_way_is_stored_as_shown_without_its_hyphen() {
    assert_eq!(typed_code("bdfh-jklm "), "BDFHJKLM");
    assert_eq!(shown_code("BDFHJKLM"), "BDFH-JKLM");
}

#[sqlx::test(migrations = false)]
async fn a_server_linked_late_in_a_second_says_that_second(db: PgPool) {
    crate::db::migrate(&db).await.unwrap();
    let accounts = Accounts::new(db.clone(), None, None);
    let created = accounts
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
    sqlx::query!(
        "INSERT INTO linked_servers (account_id, name, created_at)
         VALUES ($1, 'Den', to_timestamp(1700000000.7))",
        created.account.id.0,
    )
    .execute(&db)
    .await
    .unwrap();
    let links = Links::new(
        db,
        accounts,
        Arc::new(Keys::from_pems(SIGNING_KEY).unwrap()),
        None,
        Arc::new(Tcp),
    );

    let servers = links.list(created.account.id).await.unwrap();

    assert_eq!(servers[0].linked_at, 1_700_000_000);
}
