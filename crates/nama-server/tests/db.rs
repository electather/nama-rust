//! The Server's SQLite database: opening, migrating, and reading while the single writer works.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use nama_server::db::{self, Database};
use tokio::sync::oneshot;

#[tokio::test]
async fn a_fresh_start_creates_the_database_and_restarting_applies_no_migrations() {
    let data_dir = tempfile::tempdir().unwrap();
    let data_dir = data_dir.path().join("not-yet-created");

    let first_start = Database::open(&data_dir).await.unwrap();
    assert!(data_dir.join(db::FILE_NAME).is_file());
    assert_eq!(
        first_start.migrations_applied(),
        [
            1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
            25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43
        ]
    );
    insert_rows(&first_start, 0, 1).await.unwrap();
    first_start.close().await;

    let restart = Database::open(&data_dir).await.unwrap();
    assert_eq!(restart.migrations_applied(), Vec::<i64>::new());
    assert_eq!(count_rows(&restart).await, 1, "the restart lost a write");
    restart.close().await;
}

#[tokio::test]
async fn the_database_is_in_wal_mode_and_closing_checkpoints_the_wal() {
    let data_dir = tempfile::tempdir().unwrap();
    let db = Database::open(data_dir.path()).await.unwrap();
    insert_rows(&db, 0, 1).await.unwrap();
    // A read holds a reader connection open alongside the writer.
    assert_eq!(count_rows(&db).await, 1);
    // WAL mode belongs to the database file, so every connection to it uses it.
    let wal = data_dir.path().join(format!("{}-wal", db::FILE_NAME));
    assert!(wal.is_file(), "the database is not in WAL mode");

    db.close().await;
    assert!(
        !wal.exists(),
        "the WAL outlived the database: {}",
        wal.display()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn closing_after_aborted_reads_still_removes_the_wal() {
    // Each round aborts reads at whatever point they've reached, as shutdown does, so a round
    // only sometimes catches one opening or acquiring a reader.
    const ROUNDS: usize = 20;
    const READS: usize = 16;
    for round in 0..ROUNDS {
        let data_dir = tempfile::tempdir().unwrap();
        let db = Arc::new(Database::open(data_dir.path()).await.unwrap());
        insert_rows(&db, 0, 1).await.unwrap();
        let reads: Vec<_> = (0..READS)
            .map(|_| tokio::spawn(read_forever(Arc::clone(&db))))
            .collect();
        tokio::time::sleep(Duration::from_millis(1)).await;
        for read in &reads {
            read.abort();
        }
        for read in reads {
            assert!(read.await.unwrap_err().is_cancelled(), "a read failed");
        }

        Arc::into_inner(db).unwrap().close().await;
        let wal = data_dir.path().join(format!("{}-wal", db::FILE_NAME));
        assert!(
            !wal.exists(),
            "round {round}: the WAL outlived the database"
        );
    }
}

#[expect(clippy::infinite_loop, reason = "the test aborts it")]
async fn read_forever(db: Arc<Database>) {
    loop {
        count_rows(&db).await;
    }
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
        let rows = count_rows(&db).await;
        assert_eq!(rows % ROWS_PER_WRITE, 0, "a read saw half a write");
        if rows > 0 && rows < WRITES * ROWS_PER_WRITE {
            reads_mid_series += 1;
        }
    }
    writes.await.unwrap().unwrap();
    assert!(reads_mid_series > 0, "no read ran while the writes did");

    assert_eq!(count_rows(&db).await, WRITES * ROWS_PER_WRITE);
}

/// How many rows `schema_meta` has, read through the reader pool.
#[expect(clippy::unwrap_used, reason = "a failed read fails the test")]
async fn count_rows(db: &Database) -> i64 {
    sqlx::query_scalar!("SELECT COUNT(*) FROM schema_meta")
        .fetch_one(db.reader())
        .await
        .unwrap()
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

    let result: Result<(), sqlx::Error> = db
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
    assert_eq!(count_rows(&db).await, 0);
}

#[tokio::test]
async fn a_failed_commit_returns_its_error_skips_after_commit_and_later_writes_work() {
    let data_dir = tempfile::tempdir().unwrap();
    let db = Database::open(data_dir.path()).await.unwrap();
    let wrote = Arc::new(AtomicBool::new(false));
    let write_finished = Arc::clone(&wrote);
    let ran = Arc::new(AtomicBool::new(false));
    let after_commit = Arc::clone(&ran);

    let result: Result<(), sqlx::Error> = db
        .write_then(
            |tx| {
                Box::pin(async move {
                    // Temporary tables, so no domain table's owner is bypassed. A deferred foreign
                    // key is checked only at COMMIT, so the write succeeds and the commit fails.
                    sqlx::raw_sql(
                        "CREATE TEMP TABLE parent (id INTEGER PRIMARY KEY);
                         CREATE TEMP TABLE child (parent_id INTEGER REFERENCES parent (id));
                         PRAGMA defer_foreign_keys = ON;
                         INSERT INTO child (parent_id) VALUES (1);",
                    )
                    .execute(&mut **tx)
                    .await?;
                    write_finished.store(true, Ordering::SeqCst);
                    Ok(())
                })
            },
            move || after_commit.store(true, Ordering::SeqCst),
        )
        .await;

    assert!(wrote.load(Ordering::SeqCst), "the write itself failed");
    assert!(result.is_err(), "the commit should have failed");
    assert!(
        !ran.load(Ordering::SeqCst),
        "the closure ran after a failed commit"
    );
    insert_rows(&db, 0, 1).await.unwrap();
    assert_eq!(count_rows(&db).await, 1, "a later write failed");
}

#[tokio::test]
async fn the_after_commit_closure_has_run_when_a_committed_write_returns() {
    let data_dir = tempfile::tempdir().unwrap();
    let db = Database::open(data_dir.path()).await.unwrap();
    let ran = Arc::new(AtomicBool::new(false));
    let after_commit = Arc::clone(&ran);

    db.write_then(
        |_| Box::pin(async { Ok::<_, sqlx::Error>(()) }),
        move || after_commit.store(true, Ordering::SeqCst),
    )
    .await
    .unwrap();

    assert!(ran.load(Ordering::SeqCst));
}

#[tokio::test]
async fn a_read_does_not_wait_for_a_write_transaction_still_open() {
    let data_dir = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data_dir.path()).await.unwrap());
    let (inserted, wait_for_insert) = oneshot::channel();
    let (finish, wait_for_finish) = oneshot::channel::<()>();

    let write = tokio::spawn(insert_and_hold_open(
        Arc::clone(&db),
        inserted,
        wait_for_finish,
    ));
    wait_for_insert.await.unwrap();

    // Well inside the busy timeout, so a read blocked by the write would time out here.
    let rows = tokio::time::timeout(Duration::from_secs(1), count_rows(&db))
        .await
        .expect("a read waited for the open write transaction");
    assert_eq!(rows, 0, "a read saw a write that hadn't committed");

    finish.send(()).unwrap();
    write.await.unwrap().unwrap();
    assert_eq!(count_rows(&db).await, 1);
}

/// Inserts a row in a write, says so on `inserted`, then keeps the transaction open until `finish`.
#[expect(clippy::unwrap_used, reason = "a dropped channel end fails the test")]
async fn insert_and_hold_open(
    db: Arc<Database>,
    inserted: oneshot::Sender<()>,
    finish: oneshot::Receiver<()>,
) -> Result<(), sqlx::Error> {
    db.write(|tx| {
        Box::pin(async move {
            sqlx::query!("INSERT INTO schema_meta (key, value) VALUES ('open', 'yes')")
                .execute(&mut **tx)
                .await?;
            inserted.send(()).unwrap();
            finish.await.unwrap();
            Ok(())
        })
    })
    .await
}

#[tokio::test]
async fn a_write_that_panics_fails_alone_and_later_writes_still_work() {
    let data_dir = tempfile::tempdir().unwrap();
    let db = Database::open(data_dir.path()).await.unwrap();

    let panicked: Result<(), sqlx::Error> = db
        .write(|tx| {
            Box::pin(async move {
                sqlx::query!("INSERT INTO schema_meta (key, value) VALUES ('kept?', 'no')")
                    .execute(&mut **tx)
                    .await?;
                panic!("a bug in one write");
            })
        })
        .await;
    assert!(panicked.is_err());

    insert_rows(&db, 0, 1).await.unwrap();
    assert_eq!(count_rows(&db).await, 1, "the panicked write was committed");
}

#[tokio::test]
async fn reader_connections_have_a_busy_timeout_and_foreign_keys_on_and_cannot_write() {
    let data_dir = tempfile::tempdir().unwrap();
    let db = Database::open(data_dir.path()).await.unwrap();

    let busy_timeout_ms: Option<i64> = sqlx::query_scalar!("PRAGMA busy_timeout")
        .fetch_one(db.reader())
        .await
        .unwrap();
    assert!(busy_timeout_ms.is_some_and(|ms| ms > 0), "no busy timeout");
    let foreign_keys: Option<i64> = sqlx::query_scalar!("PRAGMA foreign_keys")
        .fetch_one(db.reader())
        .await
        .unwrap();
    assert_eq!(foreign_keys, Some(1));

    let write = sqlx::query!("INSERT INTO schema_meta (key, value) VALUES ('sneaky', 'no')")
        .execute(db.reader())
        .await;
    assert!(
        write.is_err(),
        "readers must not write; writes go through Database::write"
    );
}

// Two PRs that each add a migration with the same number don't conflict in git, since their files
// differ (#561). Listing every file in one place makes both PRs append a different last line, which does.
#[test]
fn migrations_txt_lists_every_migration_in_order() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files: Vec<String> = std::fs::read_dir(dir.join("migrations"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    files.sort();
    let listed = std::fs::read_to_string(dir.join("migrations.txt")).unwrap();
    let listed: Vec<&str> = listed.lines().collect();
    assert_eq!(
        listed, files,
        "add the new migration's file name as the last line of crates/nama-server/migrations.txt"
    );
}
