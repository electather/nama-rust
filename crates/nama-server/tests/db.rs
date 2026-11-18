//! The Server's SQLite database: opening, migrating, and reading while the single writer works.

use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use nama_server::db::{self, Database};
use sqlx::{Connection, SqliteConnection, sqlite::SqliteConnectOptions};
use tokio::sync::oneshot;

#[tokio::test]
async fn a_fresh_start_creates_the_database_and_restarting_applies_no_migrations() {
    let data_dir = tempfile::tempdir().unwrap();
    let data_dir = data_dir.path().join("not-yet-created");

    let first_start = Database::open(&data_dir).await.unwrap();
    assert!(data_dir.join(db::FILE_NAME).is_file());
    let every_version: Vec<i64> = sqlx::migrate!().iter().map(|m| m.version).collect();
    assert_eq!(first_start.migrations_applied(), every_version);
    insert_rows(&first_start, 0, 1).await.unwrap();
    first_start.close().await;

    let restart = Database::open(&data_dir).await.unwrap();
    assert_eq!(restart.migrations_applied(), Vec::<i64>::new());
    assert_eq!(count_rows(&restart).await, 1, "the restart lost a write");
    restart.close().await;
}

const BEFORE_UPGRADE: &str = "nama.db.before-upgrade";

#[tokio::test]
async fn starting_with_a_pending_migration_leaves_a_copy_from_before_it() {
    let data_dir = tempfile::tempdir().unwrap();
    let previous = create_database_one_migration_behind(data_dir.path()).await;

    let db = Database::open(data_dir.path()).await.unwrap();
    db.close().await;

    let copy = data_dir.path().join(BEFORE_UPGRADE);
    assert_eq!(latest_migration(&copy).await, previous);
}

#[tokio::test]
async fn a_later_upgrade_replaces_the_copy() {
    let data_dir = tempfile::tempdir().unwrap();
    let previous = create_database_one_migration_behind(data_dir.path()).await;
    let copy = data_dir.path().join(BEFORE_UPGRADE);
    std::fs::write(&copy, "an earlier upgrade's copy").unwrap();

    let db = Database::open(data_dir.path()).await.unwrap();
    db.close().await;

    assert_eq!(latest_migration(&copy).await, previous);
}

#[tokio::test]
async fn starting_with_nothing_pending_writes_no_copy() {
    let data_dir = tempfile::tempdir().unwrap();

    Database::open(data_dir.path()).await.unwrap().close().await;
    Database::open(data_dir.path()).await.unwrap().close().await;

    let copy = data_dir.path().join(BEFORE_UPGRADE);
    assert!(!copy.exists(), "{} was written", copy.display());
}

#[tokio::test]
async fn a_copy_that_cannot_be_written_stops_the_upgrade_says_where_and_how_much_space_and_leaves_no_partial()
 {
    let data_dir = tempfile::tempdir().unwrap();
    let previous = create_database_one_migration_behind(data_dir.path()).await;
    // A directory where the copy goes makes writing it fail.
    std::fs::create_dir(data_dir.path().join(BEFORE_UPGRADE)).unwrap();

    let err = Database::open(data_dir.path())
        .await
        .unwrap_err()
        .to_string();

    assert!(
        err.contains(&data_dir.path().display().to_string()) && err.contains(" MB free"),
        "{err}"
    );
    assert_eq!(
        latest_migration(&data_dir.path().join(db::FILE_NAME)).await,
        previous,
        "the database was upgraded without a copy"
    );
    let partial = data_dir.path().join(format!("{BEFORE_UPGRADE}.partial"));
    assert!(!partial.exists(), "{} was left behind", partial.display());
}

/// Creates the database in `data_dir` with every migration but the last, as the Server before
/// an upgrade left it, and returns the latest one it has.
#[expect(clippy::unwrap_used, reason = "a failed setup fails the test")]
async fn create_database_one_migration_behind(data_dir: &Path) -> i64 {
    let previous = sqlx::migrate!().iter().rev().nth(1).unwrap().version;
    let options = SqliteConnectOptions::new()
        .filename(data_dir.join(db::FILE_NAME))
        .create_if_missing(true);
    let mut conn = SqliteConnection::connect_with(&options).await.unwrap();
    sqlx::migrate!().run_to(previous, &mut conn).await.unwrap();
    conn.close().await.unwrap();
    previous
}

/// The latest migration the database file at `path` has had.
#[expect(clippy::unwrap_used, reason = "a failed read fails the test")]
async fn latest_migration(path: &Path) -> i64 {
    let options = SqliteConnectOptions::new().filename(path).read_only(true);
    let mut conn = SqliteConnection::connect_with(&options).await.unwrap();
    let latest = sqlx::query_scalar("SELECT max(version) FROM _sqlx_migrations")
        .fetch_one(&mut conn)
        .await
        .unwrap();
    conn.close().await.unwrap();
    latest
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

/// The version of the last table file ADR 0059's squash wrote; those up to it are frozen.
const LAST_TABLE_FILE: i64 = 36;

// The rule and its reason: crates/nama-server/AGENTS.md.
// ponytail: checks names only. Add a same-table check if a dev or upgrade database ever breaks on
// the order parallel migrations ran in.
#[test]
fn every_migration_after_the_table_files_is_named_by_utc_timestamp() {
    // 14 digits, as `YYYYMMDDHHMMSS` has.
    let timestamped = 10_000_000_000_000..100_000_000_000_000;
    let misnamed: Vec<String> = sqlx::migrate!()
        .iter()
        .filter(|m| m.version > LAST_TABLE_FILE && !timestamped.contains(&m.version))
        .map(|m| format!("{:04}_{}.sql", m.version, m.description.replace(' ', "_")))
        .collect();
    assert!(
        misnamed.is_empty(),
        "{misnamed:?} must be named by UTC creation time: see \
         crates/nama-server/README.md, \"Adding a migration\""
    );
}

/// The version of the migration that adds `request_followed_shows`.
const REQUEST_FOLLOWED_SHOWS: i64 = 20_261_117_234_836;

#[tokio::test]
async fn upgrading_to_request_followed_shows_links_each_request_to_the_followed_show_that_opened_it()
 {
    let data_dir = tempfile::tempdir().unwrap();
    let options = SqliteConnectOptions::new()
        .filename(data_dir.path().join(db::FILE_NAME))
        .create_if_missing(true);
    let mut conn = SqliteConnection::connect_with(&options).await.unwrap();
    let migrations = sqlx::migrate!();
    let before = migrations
        .iter()
        .map(|m| m.version)
        .filter(|&version| version < REQUEST_FOLLOWED_SHOWS)
        .max();
    migrations.run_to(before.unwrap(), &mut conn).await.unwrap();
    sqlx::raw_sql(
        "INSERT INTO followed_shows (id, show_id, title, state, created_at)
         VALUES (7, 'tmdb-70523', 'Dark', 'pending', 0);
         INSERT INTO requests (id, title_id, season, title, state, created_at, followed_show_id)
         VALUES (1, 'tmdb-70523', 1, 'Dark', 'pending', 0, 7),
                (2, 'tmdb-70523', 2, 'Dark', 'pending', 0, NULL);",
    )
    .execute(&mut conn)
    .await
    .unwrap();
    conn.close().await.unwrap();

    let db = Database::open(data_dir.path()).await.unwrap();
    let linked: Vec<(i64, i64)> =
        sqlx::query_as("SELECT request_id, followed_show_id FROM request_followed_shows")
            .fetch_all(db.reader())
            .await
            .unwrap();
    db.close().await;

    assert_eq!(linked, [(1, 7)]);
}
