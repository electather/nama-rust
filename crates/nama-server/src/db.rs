//! The Server's SQLite database (ADR 0005): one file in the data directory, in WAL mode, with
//! migrations embedded in the binary and applied on open.
//!
//! Reads go through a pool of read-only connections. Every write goes through one writer task
//! that owns the only writable connection and runs each write as its own transaction, so a long
//! scan's writes queue behind each other instead of locking readers out, and the Server's writes
//! never fight each other for SQLite's write lock.
//!
//! Queries use `sqlx::query!` and friends, which check them at compile time against the query
//! data checked in under `.sqlx/` at the workspace root. After adding or changing a query or a
//! migration, regenerate it with sqlx-cli against a scratch database, from the workspace root:
//!
//! ```sh
//! export DATABASE_URL=sqlite:///tmp/nama-dev.db
//! sqlx database create
//! sqlx migrate run --source crates/nama-server/migrations
//! cargo sqlx prepare --workspace -- --all-targets
//! ```

use std::{collections::HashSet, future::Future, path::Path, pin::Pin, time::Duration};

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
const WRITE_QUEUE: usize = 64;

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

/// A future that may borrow from the connection or transaction it was given.
type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

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
        std::fs::create_dir_all(data_dir).map_err(OpenError::DataDir)?;
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
        let (jobs, queue) = mpsc::channel(WRITE_QUEUE);
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
    /// db.write(|tx| {
    ///     Box::pin(async move {
    ///         sqlx::query("DELETE FROM schema_meta").execute(&mut **tx).await?;
    ///         Ok(())
    ///     })
    /// })
    /// .await
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Whatever `write` returns, or [`sqlx::Error::PoolClosed`] if the writer has stopped
    /// because an earlier write panicked.
    pub async fn write<T, F>(&self, write: F) -> Result<T, sqlx::Error>
    where
        T: Send + 'static,
        F: for<'t> FnOnce(&'t mut Transaction<'_, Sqlite>) -> BoxFuture<'t, Result<T, sqlx::Error>>
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
        if self.jobs.send(job).await.is_err() {
            return Err(sqlx::Error::PoolClosed);
        }
        result.await.unwrap_or(Err(sqlx::Error::PoolClosed))
    }

    /// Versions of the migrations this open applied, oldest first. Empty when the database was
    /// already up to date.
    #[must_use]
    pub fn migrations_applied(&self) -> &[i64] {
        &self.migrations_applied
    }

    /// Closes every connection, after finishing the writes already queued.
    pub async fn close(self) {
        drop(self.jobs);
        if let Err(err) = self.writer.await {
            tracing::warn!("the database writer stopped abnormally: {err}");
        }
        self.readers.close().await;
    }
}

/// Runs queued writes one at a time until every [`Database`] handle is gone.
async fn run_writer(mut conn: SqliteConnection, mut queue: mpsc::Receiver<Job>) {
    while let Some(job) = queue.recv().await {
        job(&mut conn).await;
    }
    if let Err(err) = conn.close().await {
        tracing::warn!("could not close the database writer cleanly: {err}");
    }
}

/// Runs `write` in a transaction on `conn`, committing on `Ok` and rolling back on `Err`.
async fn in_transaction<T, F>(conn: &mut SqliteConnection, write: F) -> Result<T, sqlx::Error>
where
    F: for<'t> FnOnce(&'t mut Transaction<'_, Sqlite>) -> BoxFuture<'t, Result<T, sqlx::Error>>,
{
    let mut tx = conn.begin().await?;
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
