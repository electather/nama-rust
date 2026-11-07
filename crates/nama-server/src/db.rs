//! The Server's SQLite database (ADR 0005): one file in the data directory, in WAL mode, with
//! migrations embedded in the binary and applied on open.
//!
//! Reads go through a pool of read-only connections. Every write goes through one writer task
//! that owns the only writable connection and runs each write as its own transaction, so a long
//! scan's writes queue behind each other instead of locking readers out, and the Server's writes
//! never fight each other for SQLite's write lock.
//!
//! Queries use `sqlx::query!` and friends, which check them at compile time against the query
//! data checked in under `.sqlx/` at the workspace root. The crate's README says how to
//! regenerate it.

use std::{
    collections::HashSet,
    panic::AssertUnwindSafe,
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use futures_util::FutureExt;
use sqlx::{
    Connection, Sqlite, SqliteConnection, SqlitePool, Transaction,
    migrate::{Migrate, MigrateError, Migrator},
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
};
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
};

/// The database file's name inside the data directory.
pub const FILE_NAME: &str = "nama.db";

/// How long a connection waits on a lock held by another before giving up.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// The most read connections open at once.
const MAX_READERS: u32 = 8;

/// How many writes can wait for the writer before [`Database::write`] waits for room.
const WRITE_QUEUE_CAPACITY: usize = 64;

/// The schema migrations in `migrations/`, compiled into the binary.
static MIGRATOR: Migrator = sqlx::migrate!();

/// Why the database couldn't be opened.
#[derive(Debug, thiserror::Error)]
pub enum OpenError {
    /// The data directory couldn't be created.
    #[error("could not create the data directory: {0}")]
    DataDir(#[source] std::io::Error),
    /// SQLite couldn't open or set up the database file.
    #[error("could not open the database: {0}")]
    Sqlx(#[from] sqlx::Error),
    /// A migration failed, or the database has migrations this binary doesn't know.
    #[error("could not migrate the database: {0}")]
    Migrate(#[from] MigrateError),
}

/// The future a write returns, which may borrow the transaction it was given.
pub use futures_util::future::BoxFuture;

/// A write waiting for the writer task, which hands it the writable connection.
type Job = Box<dyn for<'c> FnOnce(&'c mut SqliteConnection) -> BoxFuture<'c, ()> + Send>;

/// The Server's open database.
#[derive(Debug)]
pub struct Database {
    readers: SqlitePool,
    jobs: mpsc::Sender<Job>,
    writer: JoinHandle<()>,
    migrations_applied: Vec<i64>,
}

impl Database {
    /// Opens the database in `data_dir`, creating the directory and the file if they don't exist,
    /// and applies any migrations it hasn't had yet.
    ///
    /// # Errors
    ///
    /// If the directory or file can't be created or opened, or a migration fails.
    pub async fn open(data_dir: &Path) -> Result<Self, OpenError> {
        tokio::fs::create_dir_all(data_dir)
            .await
            .map_err(OpenError::DataDir)?;
        let options = SqliteConnectOptions::new()
            .filename(data_dir.join(FILE_NAME))
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .busy_timeout(BUSY_TIMEOUT)
            .foreign_keys(true);

        // The writer creates the file and switches it to WAL before any reader opens it.
        let mut writer =
            SqliteConnection::connect_with(&options.clone().create_if_missing(true)).await?;
        let migrations_applied = migrate(&mut writer).await?;
        let readers = SqlitePoolOptions::new()
            .max_connections(MAX_READERS)
            .connect_with(options.read_only(true))
            .await?;
        let (jobs, queue) = mpsc::channel(WRITE_QUEUE_CAPACITY);
        Ok(Self {
            readers,
            jobs,
            writer: tokio::spawn(run_writer(writer, queue)),
            migrations_applied,
        })
    }

    /// The pool of read-only connections. Use it for every query that doesn't change anything;
    /// a query that tries to fails.
    #[must_use]
    pub fn reader(&self) -> &SqlitePool {
        &self.readers
    }

    /// Runs `write` in a transaction on the single writable connection, after any writes queued
    /// before it, and commits if it returns `Ok` or rolls back if it returns `Err`.
    ///
    /// Keep each write short, such as one batch of a scan: readers never wait for it, but every
    /// other write does.
    ///
    /// ```no_run
    /// # async fn example(db: &nama_server::db::Database) -> Result<(), sqlx::Error> {
    /// let key = "example".to_owned();
    /// db.write(|tx| {
    ///     Box::pin(async move {
    ///         sqlx::query!("INSERT INTO schema_meta (key, value) VALUES (?, 'x')", key)
    ///             .execute(&mut **tx)
    ///             .await?;
    ///         Ok(())
    ///     })
    /// })
    /// .await
    /// # }
    /// ```
    ///
    /// `write` can fail with any error a [`sqlx::Error`] converts into, so it can roll back
    /// because of something it found, such as a row that's already taken, and say so in its own
    /// terms.
    ///
    /// # Errors
    ///
    /// Whatever `write` returns, or [`sqlx::Error::WorkerCrashed`] if `write` panicked. Either
    /// way nothing it did is committed, and later writes carry on.
    pub async fn write<T, E, F>(&self, write: F) -> Result<T, E>
    where
        T: Send + 'static,
        E: From<sqlx::Error> + Send + 'static,
        F: for<'t> FnOnce(&'t mut Transaction<'_, Sqlite>) -> BoxFuture<'t, Result<T, E>>
            + Send
            + 'static,
    {
        let (done, result) = oneshot::channel();
        let job: Job = Box::new(move |conn| {
            Box::pin(async move {
                // The caller may have stopped waiting; the write still happened.
                let _: Result<(), _> = done.send(in_transaction(conn, write).await);
            })
        });
        // The writer runs until the last handle is dropped, so it's there to take the job.
        if self.jobs.send(job).await.is_err() {
            return Err(sqlx::Error::PoolClosed.into());
        }
        // The job only drops its reply unsent if `write` panicked.
        result
            .await
            .unwrap_or_else(|_| Err(sqlx::Error::WorkerCrashed.into()))
    }

    /// Versions of the migrations this open applied, oldest first. Empty when the database was
    /// already up to date.
    #[must_use]
    pub fn migrations_applied(&self) -> &[i64] {
        &self.migrations_applied
    }

    /// Closes every connection, after finishing the writes already queued.
    ///
    /// The writer closes last: only a writable connection can checkpoint the WAL into the
    /// database file and remove it, which SQLite does when the last connection closes.
    pub async fn close(self) {
        self.readers.close().await;
        drop(self.jobs);
        if let Err(err) = self.writer.await {
            tracing::warn!("the database writer stopped abnormally: {err}");
        }
    }
}

/// Runs queued writes one at a time until every [`Database`] handle is gone.
async fn run_writer(mut conn: SqliteConnection, mut queue: mpsc::Receiver<Job>) {
    while let Some(job) = queue.recv().await {
        // A write that panics fails alone: its caller gets an error, and the transaction it
        // left open is rolled back before the next write begins.
        if AssertUnwindSafe(job(&mut conn))
            .catch_unwind()
            .await
            .is_err()
        {
            tracing::error!("a database write panicked; it was rolled back");
        }
    }
    if let Err(err) = conn.close().await {
        tracing::warn!("could not close the database writer cleanly: {err}");
    }
}

/// Runs `write` in a transaction on `conn`, committing on `Ok` and rolling back on `Err`.
async fn in_transaction<T, E, F>(conn: &mut SqliteConnection, write: F) -> Result<T, E>
where
    E: From<sqlx::Error>,
    F: for<'t> FnOnce(&'t mut Transaction<'_, Sqlite>) -> BoxFuture<'t, Result<T, E>>,
{
    // The one writer already runs the Server's writes one at a time. IMMEDIATE also takes SQLite's
    // write lock up front, before the write's first read, so nothing it reads can change under it
    // even if something outside the Server writes to the file.
    let mut tx = conn.begin_with("BEGIN IMMEDIATE").await?;
    match write(&mut tx).await {
        Ok(value) => {
            tx.commit().await?;
            Ok(value)
        }
        Err(err) => {
            // Report why the write failed, not why undoing it did; a transaction that can't be
            // rolled back here is rolled back when the connection is next used.
            if let Err(rollback) = tx.rollback().await {
                tracing::warn!("could not roll back a failed write: {rollback}");
            }
            Err(err)
        }
    }
}

/// Applies pending migrations and returns the versions it applied.
async fn migrate(conn: &mut SqliteConnection) -> Result<Vec<i64>, MigrateError> {
    conn.ensure_migrations_table(&MIGRATOR.table_name).await?;
    let before: HashSet<i64> = conn
        .list_applied_migrations(&MIGRATOR.table_name)
        .await?
        .into_iter()
        .map(|m| m.version)
        .collect();
    MIGRATOR.run(&mut *conn).await?;
    Ok(MIGRATOR
        .iter()
        .filter(|m| m.migration_type.is_up_migration())
        .map(|m| m.version)
        .filter(|v| !before.contains(v))
        .collect())
}

/// Now, in seconds since the Unix epoch, as the database stores times.
pub(crate) fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| {
            i64::try_from(since.as_secs()).unwrap_or(i64::MAX)
        })
}
