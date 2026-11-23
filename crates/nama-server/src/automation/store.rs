//! The Store: what each Plugin keeps, Plugin-wide or for one User, by key.

use nama_plugin_api::PluginId;
use sqlx::SqliteConnection;

use crate::db::Database;

/// The longest key a Plugin may keep, in bytes.
const MAX_KEY: usize = 256;
/// The largest value a Plugin may keep, in bytes.
const MAX_VALUE: usize = 64 * 1024;
/// The most entries a Plugin may keep, Plugin-wide and per-User together.
const MAX_ENTRIES: i64 = 10_000;

/// Why the Store refused or failed.
#[derive(Debug, thiserror::Error)]
pub(super) enum Error {
    #[error("a Store key can't be over {MAX_KEY} bytes, and this one is {0}")]
    KeyTooLong(usize),
    #[error("a Store value can't be over 64 KiB, and this one is {0} bytes")]
    ValueTooLarge(usize),
    #[error("a Plugin can't keep over {0} entries in its Store; delete one to keep a new key")]
    Full(i64),
    #[error("there is no User {0}")]
    NoUser(i64),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

fn check_key(key: &str) -> Result<(), Error> {
    if key.len() > MAX_KEY {
        return Err(Error::KeyTooLong(key.len()));
    }
    Ok(())
}

/// The value `plugin` keeps under `key`, for `user` or Plugin-wide.
pub(super) async fn get(
    db: &Database,
    plugin: &PluginId,
    user: Option<i64>,
    key: &str,
) -> Result<Option<Vec<u8>>, Error> {
    check_key(key)?;
    let plugin = plugin.as_str();
    Ok(sqlx::query_scalar!(
        "SELECT value FROM plugin_store WHERE plugin_id = ? AND user_id IS ? AND key = ?",
        plugin,
        user,
        key
    )
    .fetch_optional(db.reader())
    .await?)
}

/// Keeps `value` under `key` for `plugin`, for `user` or Plugin-wide, replacing what was there.
/// Refuses a new key once the Plugin keeps [`MAX_ENTRIES`].
pub(super) async fn put(
    db: &Database,
    plugin: &PluginId,
    user: Option<i64>,
    key: String,
    value: Vec<u8>,
) -> Result<(), Error> {
    put_within(db, plugin, user, key, value, MAX_ENTRIES).await
}

/// [`put`], with the Plugin's cap on entries at `cap`, which a test lowers.
#[expect(
    clippy::too_many_arguments,
    reason = "the cap is an argument only so a test can lower it"
)]
async fn put_within(
    db: &Database,
    plugin: &PluginId,
    user: Option<i64>,
    key: String,
    value: Vec<u8>,
    cap: i64,
) -> Result<(), Error> {
    check_key(&key)?;
    if value.len() > MAX_VALUE {
        return Err(Error::ValueTooLarge(value.len()));
    }
    let plugin = plugin.as_str().to_owned();
    db.write(move |tx| {
        Box::pin(async move {
            let replaced = sqlx::query!(
                "DELETE FROM plugin_store WHERE plugin_id = ? AND user_id IS ? AND key = ?",
                plugin,
                user,
                key
            )
            .execute(&mut **tx)
            .await?
            .rows_affected();
            if replaced == 0 {
                // Counted in the write, so two puts can't both take the last place.
                check_room(tx, &plugin, cap).await?;
            }
            sqlx::query!(
                "INSERT INTO plugin_store (plugin_id, user_id, key, value) VALUES (?, ?, ?, ?)",
                plugin,
                user,
                key,
                value
            )
            .execute(&mut **tx)
            .await
            .map_err(|err| match (&err, user) {
                (sqlx::Error::Database(db_err), Some(user))
                    if db_err.is_foreign_key_violation() =>
                {
                    Error::NoUser(user)
                }
                _ => Error::Database(err),
            })?;
            Ok(())
        })
    })
    .await
}

/// Refuses with [`Error::Full`] once `plugin` keeps `cap` entries.
async fn check_room(conn: &mut SqliteConnection, plugin: &str, cap: i64) -> Result<(), Error> {
    let kept = sqlx::query_scalar!(
        r#"SELECT count(*) AS "kept!: i64" FROM plugin_store WHERE plugin_id = ?"#,
        plugin
    )
    .fetch_one(conn)
    .await?;
    if kept >= cap {
        return Err(Error::Full(cap));
    }
    Ok(())
}

/// Removes what `plugin` keeps under `key`, for `user` or Plugin-wide.
pub(super) async fn delete(
    db: &Database,
    plugin: &PluginId,
    user: Option<i64>,
    key: String,
) -> Result<(), Error> {
    check_key(&key)?;
    let plugin = plugin.as_str().to_owned();
    db.write(move |tx| {
        Box::pin(async move {
            sqlx::query!(
                "DELETE FROM plugin_store WHERE plugin_id = ? AND user_id IS ? AND key = ?",
                plugin,
                user,
                key
            )
            .execute(&mut **tx)
            .await?;
            Ok(())
        })
    })
    .await
}

/// Removes every entry `plugin` keeps, Plugin-wide and per-User.
pub(crate) async fn delete_plugin(
    conn: &mut SqliteConnection,
    plugin: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query!("DELETE FROM plugin_store WHERE plugin_id = ?", plugin)
        .execute(conn)
        .await?;
    Ok(())
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
