//! The Hub's PostgreSQL database (ADR 0005): connecting to it, migrating it, and checking it
//! answers.

use sqlx::{
    PgPool,
    migrate::{MigrateError, Migrator},
};

static MIGRATOR: Migrator = sqlx::migrate!();

/// Why the Hub couldn't reach its database.
#[derive(Debug, thiserror::Error)]
#[error("could not connect to Postgres: {0}")]
pub struct ConnectError(#[from] sqlx::Error);

/// Connects to the database at `url`, a `postgres://` URL.
///
/// # Errors
///
/// If `url` is malformed, or the database can't be reached or refuses the login.
pub async fn connect(url: &str) -> Result<PgPool, ConnectError> {
    Ok(PgPool::connect(url).await?)
}

/// Applies every migration `db` hasn't had, all in one transaction, so an upgrade that fails part
/// way leaves it as it was (ADR 0060). There is no copy before upgrading: the host's PostgreSQL
/// keeps backups.
///
/// # Errors
///
/// If a migration fails, or `db` had one this Hub doesn't know.
pub async fn migrate(db: &PgPool) -> Result<(), MigrateError> {
    let mut tx = db.begin().await?;
    // Each migration's own transaction nests in this one as a savepoint.
    MIGRATOR.run(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

/// Checks `db` answers a query.
pub(crate) async fn ping(db: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query_scalar!("SELECT 1").fetch_one(db).await?;
    Ok(())
}
