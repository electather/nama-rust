//! The Server's SQLite database (ADR 0005): one file in the data directory, in WAL mode, with
//! migrations embedded in the binary and applied on open.
//!
//! Reads go through a pool of read-only connections; writes queue on one writer task (see the
//! crate's `AGENTS.md`), so a long scan never locks readers out and writes never fight over
//! SQLite's write lock.

use std::{
    collections::HashSet,
    panic::AssertUnwindSafe,
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[cfg(feature = "testing")]
use std::sync::atomic::{AtomicI64, Ordering};

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
use tracing::Instrument as _;

/// The database file's name inside the data directory.
pub const FILE_NAME: &str = "nama.db";

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

const MAX_READERS: u32 = 8;

/// How many writes can wait for the writer before [`Database::write`] waits for room.
const WRITE_QUEUE_CAPACITY: usize = 64;

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
        warn_about_filesystem(data_dir);
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
        // Every reader opens here and is never replaced or pinged on acquire: one opened or pinged
        // in a cancelled caller's future outlives `SqlitePool::close`, and the writer must close
        // last to remove the WAL.
        let readers = SqlitePoolOptions::new()
            .max_connections(MAX_READERS)
            .min_connections(MAX_READERS)
            .idle_timeout(None)
            .max_lifetime(None)
            .test_before_acquire(false)
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
        self.write_then(write, || {}).await
    }

    /// Like [`Database::write`], then runs `after_commit` on the writer only once the commit has
    /// succeeded, before this returns and before the next write begins, even if the caller stopped
    /// waiting. Use it to change state in memory that must match the database, in the order the
    /// writes commit.
    ///
    /// `after_commit` is synchronous and must be short: every other write waits for it.
    ///
    /// # Errors
    ///
    /// As [`Database::write`], and then `after_commit` doesn't run. If `after_commit` panics, the
    /// write stays committed but this returns [`sqlx::Error::WorkerCrashed`].
    pub async fn write_then<T, E, F>(
        &self,
        write: F,
        after_commit: impl FnOnce() + Send + 'static,
    ) -> Result<T, E>
    where
        T: Send + 'static,
        E: From<sqlx::Error> + Send + 'static,
        F: for<'t> FnOnce(&'t mut Transaction<'_, Sqlite>) -> BoxFuture<'t, Result<T, E>>
            + Send
            + 'static,
    {
        let (done, result) = oneshot::channel();
        // The writer's task runs it, so its queries would otherwise belong to no caller's trace.
        let caller = tracing::Span::current();
        let job: Job = Box::new(move |conn| {
            Box::pin(
                async move {
                    // The caller may have stopped waiting; the write still happened.
                    let _: Result<(), _> =
                        done.send(in_transaction(conn, write, after_commit).await);
                }
                .instrument(caller),
            )
        });
        // The writer runs until the last handle is dropped, so it's there to take the job.
        if self.jobs.send(job).await.is_err() {
            return Err(sqlx::Error::PoolClosed.into());
        }
        // The job only drops its reply unsent if `write` or `after_commit` panicked.
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

    /// How many writes wait for the writer, not counting the one it runs.
    #[cfg(test)]
    pub(crate) fn queued_writes(&self) -> usize {
        self.jobs.max_capacity() - self.jobs.capacity()
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

/// Runs `work` in a task of its own, so what must follow a committed write, such as publishing
/// its events, finishes even if the caller stops waiting (ADR 0029).
///
/// # Errors
///
/// Whatever `work` returns, or [`sqlx::Error::WorkerCrashed`] if it panicked.
pub(crate) async fn detached<T, E>(
    work: impl Future<Output = Result<T, E>> + Send + 'static,
) -> Result<T, E>
where
    T: Send + 'static,
    E: From<sqlx::Error> + Send + 'static,
{
    // The task is never aborted, so it only fails if it panicked.
    tokio::spawn(work.in_current_span())
        .await
        .unwrap_or_else(|_| Err(sqlx::Error::WorkerCrashed.into()))
}

/// Runs queued writes one at a time until every [`Database`] handle is gone.
async fn run_writer(mut conn: SqliteConnection, mut queue: mpsc::Receiver<Job>) {
    while let Some(job) = queue.recv().await {
        // A write that panics fails alone: its caller gets an error, and a transaction it left
        // open is rolled back before the next write begins. An `after_commit` that panics comes
        // after its commit, so that write stays.
        if AssertUnwindSafe(job(&mut conn))
            .catch_unwind()
            .await
            .is_err()
        {
            tracing::error!("a database write panicked");
        }
    }
    if let Err(err) = conn.close().await {
        tracing::warn!("could not close the database writer cleanly: {err}");
    }
}

async fn in_transaction<T, E, F>(
    conn: &mut SqliteConnection,
    write: F,
    after_commit: impl FnOnce(),
) -> Result<T, E>
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
            after_commit();
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

/// How far [`advance_clock`] has moved the Server's clock, in milliseconds.
#[cfg(feature = "testing")]
static OFFSET_MS: AtomicI64 = AtomicI64::new(0);

/// Moves the Server's clock, which [`now`], [`now_ms`] and so a Plugin's `nama.now` read, ahead by
/// `by`. Testing only. Process-wide, so `TestServer` tests sharing a process take turns, each
/// starting on the real clock: see "One clock" in `crates/nama-server/AGENTS.md`.
#[cfg(feature = "testing")]
pub fn advance_clock(by: Duration) {
    let by = i64::try_from(by.as_millis()).unwrap_or(i64::MAX);
    OFFSET_MS.fetch_add(by, Ordering::Relaxed);
}

/// Puts the Server's clock back on the real clock, for a test that moves it. Testing only. Moves
/// every clock in the process back, so only a test holding the clock alone may call it: see "One
/// clock" in `crates/nama-server/AGENTS.md`.
#[cfg(feature = "testing")]
pub fn reset_clock() {
    OFFSET_MS.store(0, Ordering::Relaxed);
}

#[cfg(feature = "testing")]
fn offset_ms() -> i64 {
    OFFSET_MS.load(Ordering::Relaxed)
}

#[cfg(not(feature = "testing"))]
const fn offset_ms() -> i64 {
    0
}

/// Now, in seconds since the Unix epoch, as the database stores times.
pub(crate) fn now() -> i64 {
    now_ms().div_euclid(1000)
}

/// Now, in milliseconds since the Unix epoch.
pub(crate) fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| {
            i64::try_from(since.as_millis()).unwrap_or(i64::MAX)
        })
        .saturating_add(offset_ms())
}

/// Why the data directory's filesystem is a poor home for SQLite's WAL, which needs working
/// locks and shared memory.
#[derive(Debug, PartialEq, Eq)]
enum FilesystemRisk {
    /// Locks across the network fail, so the database can lock up or corrupt.
    Network,
    /// Rewriting pages in place fragments the file, and some setups, such as ZFS inside an LXC
    /// container, lock it up.
    CopyOnWrite,
}

/// `filesystem` is spelled as macOS's `statfs` names it, or as `linux_filesystem_name` does.
fn filesystem_risk(filesystem: &str) -> Option<FilesystemRisk> {
    match filesystem {
        "nfs" | "cifs" | "smb2" | "smbfs" | "9p" | "ceph" | "afs" | "afpfs" | "webdav" => {
            Some(FilesystemRisk::Network)
        }
        "zfs" | "btrfs" => Some(FilesystemRisk::CopyOnWrite),
        _ => None,
    }
}

/// Warns when `data_dir` sits where SQLite does badly; the Server starts anyway.
fn warn_about_filesystem(data_dir: &Path) {
    let Some(filesystem) = filesystem_name(data_dir) else {
        return;
    };
    match filesystem_risk(&filesystem) {
        Some(FilesystemRisk::Network) => tracing::warn!(
            "the data directory is on a network share ({filesystem}), where the database can \
             lock up or corrupt; keep it on a local disk"
        ),
        Some(FilesystemRisk::CopyOnWrite) => tracing::warn!(
            "the data directory is on {filesystem}, a copy-on-write filesystem, where the \
             database can slow down or lock up; see \"Docker\" in the README"
        ),
        None => {}
    }
}

#[cfg(target_os = "linux")]
fn filesystem_name(dir: &Path) -> Option<String> {
    let stat = rustix::fs::statfs(dir).ok()?;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "magic numbers fit in 32 bits, and a 32-bit `long` holds some as negative"
    )]
    let magic = stat.f_type as u32;
    linux_filesystem_name(magic).map(str::to_owned)
}

/// Linux reports a magic number, not a name, so only those `filesystem_risk` knows are named.
#[cfg(target_os = "linux")]
fn linux_filesystem_name(magic: u32) -> Option<&'static str> {
    match magic {
        0x6969 => Some("nfs"),
        0xFF53_4D42 => Some("cifs"),
        0xFE53_4D42 => Some("smb2"),
        0x0102_1997 => Some("9p"),
        0x00C3_6400 => Some("ceph"),
        0x5346_414F => Some("afs"),
        0x2FC1_2FC1 => Some("zfs"),
        0x9123_683E => Some("btrfs"),
        _ => None,
    }
}

#[cfg(target_vendor = "apple")]
fn filesystem_name(dir: &Path) -> Option<String> {
    let stat = rustix::fs::statfs(dir).ok()?;
    // A name such as "apfs", NUL-terminated and ASCII, so no `c_char` is negative.
    Some(
        stat.f_fstypename
            .iter()
            .map_while(|&c| u8::try_from(c).ok().filter(|&byte| byte != 0))
            .map(char::from)
            .collect(),
    )
}

// ponytail: other systems, Windows among them, aren't checked; ask theirs once one runs a Server.
#[cfg(not(any(target_os = "linux", target_vendor = "apple")))]
const fn filesystem_name(_dir: &Path) -> Option<String> {
    None
}

#[cfg(test)]
#[path = "db_tests.rs"]
mod tests;
