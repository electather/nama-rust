use sqlx::{Connection, SqliteConnection, sqlite::SqliteConnectOptions};

use crate::db::{self, Database};

/// The version of the migration that takes Manage Users from Users on a chosen list.
const MANAGE_USERS_SEES_EVERY_LIBRARY: i64 = 20_261_127_000_219;

#[tokio::test]
async fn upgrading_takes_manage_users_and_its_invites_only_from_a_user_hidden_some_libraries() {
    let data = tempfile::tempdir().unwrap();
    let options = SqliteConnectOptions::new()
        .filename(data.path().join(db::FILE_NAME))
        .create_if_missing(true);
    let mut conn = SqliteConnection::connect_with(&options).await.unwrap();
    let migrations = sqlx::migrate!();
    let before = (migrations.iter().map(|migration| migration.version))
        .filter(|&version| version < MANAGE_USERS_SEES_EVERY_LIBRARY)
        .max();
    migrations.run_to(before.unwrap(), &mut conn).await.unwrap();
    // Bits: 2 Admin, 4 Manage Users, 8 Manage Libraries, 64 Request.
    sqlx::raw_sql(
        "INSERT INTO users (id, username, display_username, permissions, all_libraries,
                            created_at, updated_at)
         VALUES (1, 'hudson', 'Hudson', 68, 0, 0, 0),
                (2, 'ripley', 'Ripley', 12, 0, 0, 0),
                (3, 'gorman', 'Gorman', 6, 0, 0, 0),
                (4, 'hicks', 'Hicks', 4, 1, 0, 0);
         INSERT INTO invites (token_hash, note, permissions, created_by, created_at, expires_at)
         VALUES (x'01', '', 64, 1, 0, 1), (x'04', '', 64, 4, 0, 1);",
    )
    .execute(&mut conn)
    .await
    .unwrap();
    conn.close().await.unwrap();

    let db = Database::open(data.path()).await.unwrap();

    let permissions: Vec<(i64, i64)> =
        sqlx::query_as("SELECT id, permissions FROM users ORDER BY id")
            .fetch_all(db.reader())
            .await
            .unwrap();
    assert_eq!(permissions, [(1, 64), (2, 12), (3, 6), (4, 4)]);
    let invites_by: Vec<i64> = sqlx::query_scalar("SELECT created_by FROM invites")
        .fetch_all(db.reader())
        .await
        .unwrap();
    assert_eq!(invites_by, [4]);
}
