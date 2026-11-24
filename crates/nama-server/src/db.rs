//! The Server's SQLite database (ADR 0005): one file in the data directory, in WAL mode, with
//! migrations embedded in the binary and applied on open.
//!
//! Reads go through a pool of read-only connections; writes queue on one writer task (see the
//! crate's `AGENTS.md`), so a long scan never locks readers out and writes never fight over
//! SQLite's write lock.

use std::{
    collections::HashSet,
    panic::AssertUnwindSafe,
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
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

/// The copy of the database an upgrade leaves beside it, to go back to if the upgrade goes wrong.
const BEFORE_UPGRADE_FILE_NAME: &str = "nama.db.before-upgrade";

const MIB: u64 = 1024 * 1024;

/// The least free space startup asks the data directory for, however small the database.
const LOW_SPACE_FLOOR_BYTES: u64 = 1024 * MIB;

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
    /// A newer version of Nama upgraded the database; holds the version of the migration this
    /// one doesn't have.
    #[error(
        "the database was upgraded by a newer version of Nama (migration {0} is unknown to this \
         one): run that version, or go back to the copy in {BEFORE_UPGRADE_FILE_NAME} as the \
         README says"
    )]
    NewerVersion(i64),
    /// A migration failed.
    #[error("could not migrate the database: {0}")]
    Migrate(#[source] MigrateError),
    /// The upgrade left a row whose foreign key points at nothing, so none of it was applied.
    #[error(
        "upgrading the database would leave row {} of {table} pointing at a missing row of \
         {parent}, so it was left as it was",
        rowid.map_or_else(|| "?".to_owned(), |id| id.to_string())
    )]
    BrokenForeignKey {
        /// The table holding the row.
        table: String,
        /// The row's rowid, or `None` in a `WITHOUT ROWID` table.
        rowid: Option<i64>,
        /// The table its key points into.
        parent: String,
    },
    /// The copy to go back to couldn't be written, so the database was left as it was.
    #[error(
        "could not copy the database to {BEFORE_UPGRADE_FILE_NAME} before upgrading it, so it was \
         left as it was; the copy needs about {needs_mb} MB free in {}: {source}",
        data_dir.display()
    )]
    BeforeUpgradeCopy {
        /// Where the copy goes.
        data_dir: PathBuf,
        /// About how much free space the copy needs: the database's size.
        needs_mb: u64,
        /// Why the copy failed.
        source: sqlx::Error,
    },
}

impl From<MigrateError> for OpenError {
    fn from(err: MigrateError) -> Self {
        if let MigrateError::VersionMissing(version) = err {
            Self::NewerVersion(version)
        } else {
            Self::Migrate(err)
        }
    }
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
    /// and applies any migrations it hasn't had yet, after copying a database that had some.
    ///
    /// # Errors
    ///
    /// If the directory or file can't be created or opened, the copy can't be written, or a
    /// migration fails.
    pub async fn open(data_dir: &Path) -> Result<Self, OpenError> {
        tokio::fs::create_dir_all(data_dir)
            .await
            .map_err(OpenError::DataDir)?;
        warn_about_filesystem(data_dir);
        let database_bytes = tokio::fs::metadata(data_dir.join(FILE_NAME))
            .await
            .map_or(0, |file| file.len());
        warn_if_low_on_space(data_dir, free_bytes(data_dir), database_bytes);
        let options = SqliteConnectOptions::new()
            .filename(data_dir.join(FILE_NAME))
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .busy_timeout(BUSY_TIMEOUT)
            .foreign_keys(true);

        // The writer creates the file and switches it to WAL before any reader opens it.
        let mut writer =
            SqliteConnection::connect_with(&options.clone().create_if_missing(true)).await?;
        let migrations_applied = migrate(&mut writer, data_dir).await?;
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
        // Ends once the writer takes the job, so a trace shows how long the write waited behind
        // others (#1130).
        let queued = tracing::info_span!("write queued");
        let job: Job = Box::new(move |conn| {
            drop(queued);
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
            // Shared with Diagnostics (`SHARED_LINES` in `src/diagnostics.rs`).
            tracing::error!(name: "database write panicked", "a database write panicked");
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

/// Applies the migrations the database in `data_dir` hasn't had, after copying it to
/// [`BEFORE_UPGRADE_FILE_NAME`] if it already had some, so an upgrade that goes wrong has a way
/// back.
async fn migrate(conn: &mut SqliteConnection, data_dir: &Path) -> Result<Vec<i64>, OpenError> {
    conn.ensure_migrations_table(&MIGRATOR.table_name).await?;
    let before: HashSet<i64> = conn
        .list_applied_migrations(&MIGRATOR.table_name)
        .await?
        .into_iter()
        .map(|m| m.version)
        .collect();
    let pending: Vec<i64> = MIGRATOR
        .iter()
        .filter(|m| m.migration_type.is_up_migration())
        .map(|m| m.version)
        .filter(|v| !before.contains(v))
        .collect();
    // A new database has nothing to go back to.
    if !pending.is_empty() && !before.is_empty() {
        // Counts the pages still in the WAL too, which the file's size misses.
        let size = sqlx::query_scalar!(
            r#"SELECT page_count * page_size AS "size!: u64"
               FROM pragma_page_count(), pragma_page_size()"#
        )
        .fetch_one(&mut *conn)
        .await?;
        if let Err(source) = copy_before_upgrade(conn, data_dir).await {
            return Err(OpenError::BeforeUpgradeCopy {
                data_dir: data_dir.to_owned(),
                needs_mb: size.div_ceil(1_000_000),
                source,
            });
        }
    }
    if pending.is_empty() {
        // Still run: it's what fails on a database a newer Server upgraded. Nothing to check keys
        // for, and a row broken before keys were enforced mustn't stop every start.
        MIGRATOR.run(&mut *conn).await?;
        return Ok(pending);
    }
    // SQLite ignores this pragma inside a transaction, so it goes off before the upgrade's opens
    // (ADR 0060): with it on, a rebuild's `DROP TABLE` would cascade into the rows referencing it.
    sqlx::query!("PRAGMA foreign_keys = OFF")
        .execute(&mut *conn)
        .await?;
    let upgraded = upgrade(conn).await;
    // The upgrade's own error says more than one turning foreign keys back on.
    let restored = sqlx::query!("PRAGMA foreign_keys = ON")
        .execute(&mut *conn)
        .await;
    upgraded?;
    restored?;
    Ok(pending)
}

/// Runs the migrations in one transaction, so an upgrade that fails part way leaves the database as
/// it was (ADR 0060). Each migration's own transaction nests in it as a savepoint.
async fn upgrade(conn: &mut SqliteConnection) -> Result<(), OpenError> {
    // IMMEDIATE takes the write lock up front, so a second Server on the same data directory
    // fails busy before it changes anything.
    let mut tx = conn.begin_with("BEGIN IMMEDIATE").await?;
    let upgraded = async {
        MIGRATOR.run(&mut *tx).await?;
        // The casts are for sqlx, which can't look up the pragma's own columns' types.
        let broken = sqlx::query!(
            r#"SELECT CAST("table" AS TEXT) AS "table!: String",
                      CAST(rowid AS INTEGER) AS "rowid?: i64",
                      CAST(parent AS TEXT) AS "parent!: String"
               FROM pragma_foreign_key_check"#
        )
        .fetch_optional(&mut *tx)
        .await?;
        match broken {
            Some(row) => Err(OpenError::BrokenForeignKey {
                table: row.table,
                rowid: row.rowid,
                parent: row.parent,
            }),
            None => Ok(()),
        }
    }
    .await;
    match upgraded {
        Ok(()) => {
            tx.commit().await?;
            Ok(())
        }
        Err(err) => {
            // Rolled back here, not on drop: `PRAGMA foreign_keys = ON` comes next, and SQLite
            // ignores it inside a transaction.
            if let Err(rollback) = tx.rollback().await {
                tracing::warn!("could not roll back a failed upgrade: {rollback}");
            }
            Err(err)
        }
    }
}

/// Writes a copy of the database to [`BEFORE_UPGRADE_FILE_NAME`], replacing the last upgrade's.
async fn copy_before_upgrade(
    conn: &mut SqliteConnection,
    data_dir: &Path,
) -> Result<(), sqlx::Error> {
    tracing::info!("copying the database to {BEFORE_UPGRADE_FILE_NAME} before upgrading it");
    copy_to(conn, &data_dir.join(BEFORE_UPGRADE_FILE_NAME)).await
}

/// Writes a copy of the database `conn` is open on to `copy`, replacing any file there. It's
/// written under another name first, so a failed copy leaves the file at `copy` as it was.
pub(crate) async fn copy_to(conn: &mut SqliteConnection, copy: &Path) -> Result<(), sqlx::Error> {
    let mut partial = copy.as_os_str().to_owned();
    partial.push(".partial");
    let partial = PathBuf::from(partial);
    // VACUUM INTO refuses a file that isn't empty, such as one a crash left half-written.
    if let Err(err) = tokio::fs::remove_file(&partial).await
        && err.kind() != std::io::ErrorKind::NotFound
    {
        return Err(err.into());
    }
    let partial_path = partial.to_str().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "the copy's path isn't valid UTF-8",
        )
    })?;
    let written = async {
        sqlx::query!("VACUUM INTO ?", partial_path)
            .execute(&mut *conn)
            .await?;
        // VACUUM INTO doesn't sync what it writes, and the rename must not replace the last good
        // copy with one a power cut could still lose. Windows syncs only a file opened for writing.
        tokio::fs::OpenOptions::new()
            .write(true)
            .open(&partial)
            .await?
            .sync_all()
            .await?;
        tokio::fs::rename(&partial, copy).await?;
        Ok(())
    }
    .await;
    if written.is_err() {
        // A copy that ran out of room would otherwise keep holding the space the error asks for.
        // Removing it is best effort: the error that stopped the copy is the one to report.
        let _: std::io::Result<()> = tokio::fs::remove_file(&partial).await;
    }
    written
}

/// Opens the database file at `path` on a connection of its own, beside any a running Server
/// holds, without creating it or applying migrations.
///
/// # Errors
///
/// If there's no such file or SQLite can't open it.
pub(crate) async fn connect_existing(path: &Path) -> Result<SqliteConnection, sqlx::Error> {
    SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(path)
            .busy_timeout(BUSY_TIMEOUT),
    )
    .await
}

/// The highest migration the database `conn` is open on has had, or `None` if it has had none.
///
/// # Errors
///
/// If reading fails, such as on a file no Server made.
pub(crate) async fn highest_migration_applied(
    conn: &mut SqliteConnection,
) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar!(r#"SELECT MAX(version) AS "version?: i64" FROM _sqlx_migrations"#)
        .fetch_one(conn)
        .await
}

/// The highest migration this Server knows: it can't open a database that has had a higher one.
#[must_use]
pub(crate) fn highest_migration_known() -> i64 {
    MIGRATOR.iter().map(|m| m.version).max().unwrap_or_default()
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

/// Now, for timing how long ago something happened: monotonic, unlike [`now_ms`], so a change to
/// the system's time can't stretch or shrink it, but moved by [`advance_clock`] too.
pub(crate) fn instant_now() -> Instant {
    Instant::now() + Duration::from_millis(u64::try_from(offset_ms()).unwrap_or_default())
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

/// Warns when `data_dir` has less `free` space than the larger of twice the database or
/// [`LOW_SPACE_FLOOR_BYTES`], the room to grow and to copy it before an upgrade; the Server starts
/// anyway.
fn warn_if_low_on_space(data_dir: &Path, free: std::io::Result<u64>, database_bytes: u64) {
    let free = match free {
        Ok(free) => free,
        Err(err) => {
            tracing::debug!("could not read the data directory's free space: {err}");
            return;
        }
    };
    let threshold = database_bytes.saturating_mul(2).max(LOW_SPACE_FLOOR_BYTES);
    if free < threshold {
        tracing::warn!(
            "the data directory, {}, has {} MiB free, under the {} MiB the database needs to grow \
             and to be copied before an upgrade: free some space",
            data_dir.display(),
            free / MIB,
            threshold.div_ceil(MIB)
        );
    }
}

/// How many bytes an unprivileged process may still write to the filesystem holding `dir`.
#[cfg(unix)]
pub(crate) fn free_bytes(dir: &Path) -> std::io::Result<u64> {
    let stat = rustix::fs::statvfs(dir)?;
    Ok(stat.f_bavail.saturating_mul(stat.f_frsize))
}

// ponytail: off unix the Server reads no free space; ask GetDiskFreeSpaceExW for Windows.
#[cfg(not(unix))]
pub(crate) fn free_bytes(_dir: &Path) -> std::io::Result<u64> {
    Err(std::io::ErrorKind::Unsupported.into())
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
