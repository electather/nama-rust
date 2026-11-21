//! When the Server last had each Plugin, enabled or not, in `plugins_had`, so what a Plugin it
//! stopped having left behind can be deleted once it has been gone long enough (#1452).

use std::time::Duration;

use sqlx::SqliteConnection;
use tracing::warn;

use super::PluginSettings;
use crate::db;

impl PluginSettings {
    /// Records that the Server has each Plugin the registry knows now. Failing is logged, not
    /// answered: a stale time only brings its Plugin's forgetting closer.
    // A Plugin joins or leaves the registry only as the Server starts, lists or shuts down, which
    // call this, so a removed Plugin's time is when the Server last had it.
    pub(super) async fn note_had(&self) {
        let had = self.had();
        let noted = self
            .db
            .write(move |tx| Box::pin(async move { note(tx, &had).await }))
            .await;
        if let Err(err) = noted {
            warn!("could not record which plugins the server has: {err}");
        }
    }

    /// The Plugins the registry knows now.
    fn had(&self) -> Had {
        let (loaded, refused): (Vec<_>, Vec<_>) =
            (self.registry.plugins().into_iter()).partition(|plugin| plugin.manifest.is_ok());
        Had {
            ids: loaded
                .into_iter()
                .map(|plugin| plugin.id.to_string())
                .collect(),
            refused_any: !refused.is_empty(),
        }
    }

    /// Deletes, in `conn`'s write, the settings of each Plugin the Server hasn't had for at least
    /// `gone_for`, and returns their ids, sorted, for the caller to delete the rest of what they
    /// left in the same write. A Plugin the registry knows as the write starts is never one of
    /// them; one a listing brings back during the write is forgotten all the same.
    pub(crate) async fn take_gone(
        &self,
        conn: &mut SqliteConnection,
        gone_for: Duration,
    ) -> Result<Vec<String>, sqlx::Error> {
        note(conn, &self.had()).await?;
        // ponytail: after a crash, a Plugin removed while the Server was down keeps its last note
        // (a start, a listing or a call of this), so it may be forgotten that much early; note on
        // a timer if that matters.
        let gone_for = i64::try_from(gone_for.as_secs()).unwrap_or(i64::MAX);
        let before = db::now().saturating_sub(gone_for);
        let mut gone = sqlx::query_scalar!(
            "DELETE FROM plugins_had WHERE had_at <= ? RETURNING plugin_id",
            before
        )
        .fetch_all(&mut *conn)
        .await?;
        for plugin in &gone {
            sqlx::query!("DELETE FROM plugin_settings WHERE plugin_id = ?", plugin)
                .execute(&mut *conn)
                .await?;
        }
        gone.sort_unstable();
        Ok(gone)
    }
}

/// The Plugins the registry knows.
struct Had {
    /// The id of each it loaded, enabled or not.
    ids: Vec<String>,
    /// Whether it holds a Plugin it refused, which it lists under its file's name since its id
    /// can't be read.
    refused_any: bool,
}

/// Records that the Server has each Plugin `had` names now.
async fn note(conn: &mut SqliteConnection, had: &Had) -> Result<(), sqlx::Error> {
    let now = db::now();
    if had.refused_any {
        // A refused file may be any Plugin noted before, such as a new version the Server can't
        // load, so each counts as had until the Admin fixes or removes it.
        sqlx::query!("UPDATE plugins_had SET had_at = ?", now)
            .execute(&mut *conn)
            .await?;
    }
    for plugin in &had.ids {
        sqlx::query!(
            "INSERT INTO plugins_had (plugin_id, had_at) VALUES (?, ?)
             ON CONFLICT (plugin_id) DO UPDATE SET had_at = excluded.had_at",
            plugin,
            now
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "had_tests.rs"]
mod tests;
