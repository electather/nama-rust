//! The Server's SQLite database: opening, migrating, and reading while the single writer works.

use std::sync::Arc;

use nama_server::db::Database;

#[tokio::test]
async fn a_fresh_start_creates_the_database_and_restarting_applies_no_migrations() {
    let data_dir = tempfile::tempdir().unwrap();
    let data_dir = data_dir.path().join("not-yet-created");

    let first_start = Database::open(&data_dir).await.unwrap();
    assert!(data_dir.join("nama.db").is_file());
    assert_eq!(first_start.migrations_applied(), [1]);
    first_start.close().await;

    let restart = Database::open(&data_dir).await.unwrap();
    assert_eq!(restart.migrations_applied(), Vec::<i64>::new());
    restart.close().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn reads_succeed_while_a_long_series_of_writes_is_running() {
    const WRITES: i64 = 500;
    const ROWS_PER_WRITE: i64 = 20;
    let data_dir = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data_dir.path()).await.unwrap());

    let writes = tokio::spawn(write_series(Arc::clone(&db), WRITES, ROWS_PER_WRITE));

    // Every read while the writes run must succeed, and some must land between two writes.
    let mut reads_mid_series = 0;
    while !writes.is_finished() {
        let rows = sqlx::query_scalar!("SELECT COUNT(*) FROM schema_meta")
            .fetch_one(db.reader())
            .await
            .unwrap();
        assert_eq!(rows % ROWS_PER_WRITE, 0, "a read saw half a write");
        if rows > 0 && rows < WRITES * ROWS_PER_WRITE {
            reads_mid_series += 1;
        }
    }
    writes.await.unwrap().unwrap();
    assert!(reads_mid_series > 0, "no read ran while the writes did");

    let rows = sqlx::query_scalar!("SELECT COUNT(*) FROM schema_meta")
        .fetch_one(db.reader())
        .await
        .unwrap();
    assert_eq!(rows, WRITES * ROWS_PER_WRITE);
}

/// Makes `writes` writes one after another, each inserting `rows` rows into `schema_meta`.
async fn write_series(db: Arc<Database>, writes: i64, rows: i64) -> Result<(), sqlx::Error> {
    for write in 0..writes {
        insert_rows(&db, write, rows).await?;
    }
    Ok(())
}

/// Inserts `rows` rows into `schema_meta` in one write, keyed by `write` so writes don't collide.
async fn insert_rows(db: &Database, write: i64, rows: i64) -> Result<(), sqlx::Error> {
    db.write(move |tx| {
        Box::pin(async move {
            for row in 0..rows {
                let key = format!("write-{write}-row-{row}");
                sqlx::query!("INSERT INTO schema_meta (key, value) VALUES (?, 'x')", key)
                    .execute(&mut **tx)
                    .await?;
            }
            Ok(())
        })
    })
    .await
}

#[tokio::test]
async fn a_failed_write_commits_nothing() {
    let data_dir = tempfile::tempdir().unwrap();
    let db = Database::open(data_dir.path()).await.unwrap();

    let result = db
        .write(|tx| {
            Box::pin(async move {
                sqlx::query!("INSERT INTO schema_meta (key, value) VALUES ('kept?', 'no')")
                    .execute(&mut **tx)
                    .await?;
                // The same key again breaks the primary key, failing the write.
                sqlx::query!("INSERT INTO schema_meta (key, value) VALUES ('kept?', 'no')")
                    .execute(&mut **tx)
                    .await?;
                Ok(())
            })
        })
        .await;

    assert!(result.is_err());
    let rows = sqlx::query_scalar!("SELECT COUNT(*) FROM schema_meta")
        .fetch_one(db.reader())
        .await
        .unwrap();
    assert_eq!(rows, 0);
}

#[tokio::test]
async fn reader_connections_have_wal_and_foreign_keys_on_and_cannot_write() {
    let data_dir = tempfile::tempdir().unwrap();
    let db = Database::open(data_dir.path()).await.unwrap();

    let journal_mode: String = sqlx::query_scalar("PRAGMA journal_mode")
        .fetch_one(db.reader())
        .await
        .unwrap();
    assert_eq!(journal_mode, "wal");
    let foreign_keys: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
        .fetch_one(db.reader())
        .await
        .unwrap();
    assert_eq!(foreign_keys, 1);

    let write = sqlx::query!("INSERT INTO schema_meta (key, value) VALUES ('sneaky', 'no')")
        .execute(db.reader())
        .await;
    assert!(
        write.is_err(),
        "readers must not write; writes go through Database::write"
    );
}
