//! When each Episode of a Show aired, stored once per Show from the Seasons its title source gave,
//! so [`Contents::aired_episodes`] counts those aired by today whenever a page asks.

use nama_plugin_api::{PluginId, metadata::ExternalIds};
use sqlx::SqliteConnection;

use super::{Contents, Show};
use crate::{db::now, library::title_source::Season};

/// Stores the air dates of `season`'s dated Episodes, as the Plugin `plugin`'s source gave them
/// for the Show with the ids `show`, in place of what it gave for that Season before.
pub(super) async fn store(
    conn: &mut SqliteConnection,
    plugin: &PluginId,
    show: &ExternalIds,
    season: &Season,
) -> Result<(), sqlx::Error> {
    // The first id in namespace order, which `aired_episodes` looks it up by, as `title_details`.
    let Some(id) = show.iter().next() else {
        return Ok(());
    };
    let (plugin, namespace, value) = (plugin.as_str(), id.namespace(), id.value());
    sqlx::query!(
        "DELETE FROM episode_air_dates
         WHERE plugin = ? AND namespace = ? AND value = ? AND season = ?",
        plugin,
        namespace,
        value,
        season.number
    )
    .execute(&mut *conn)
    .await?;
    for episode in &season.episodes {
        let Some(air_date) = &episode.air_date else {
            continue;
        };
        sqlx::query!(
            "INSERT OR REPLACE INTO episode_air_dates
                 (plugin, namespace, value, season, episode, air_date)
             VALUES (?, ?, ?, ?, ?, ?)",
            plugin,
            namespace,
            value,
            season.number,
            episode.number,
            air_date
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

/// Deletes the air dates `plugin`'s title source gave.
pub(super) async fn forget_plugin(
    conn: &mut SqliteConnection,
    plugin: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query!("DELETE FROM episode_air_dates WHERE plugin = ?", plugin)
        .execute(conn)
        .await?;
    Ok(())
}

impl Contents {
    /// How many of `show`'s Episodes, Specials aside, aired by today, going by the air dates its
    /// title source last gave. 0 when it gave none.
    pub(crate) async fn aired_episodes(&self, show: &Show) -> Result<u32, sqlx::Error> {
        let (Some(episode), Some(id)) = (show.episodes().next(), show.ids.iter().next()) else {
            return Ok(0);
        };
        let (namespace, value, now) = (id.namespace(), id.value(), now());
        let aired = sqlx::query_scalar!(
            r#"SELECT COUNT(*) AS "aired!: u32" FROM episode_air_dates
               WHERE plugin = ? AND namespace = ? AND value = ? AND season > 0
                 AND air_date <= date(?, 'unixepoch')"#,
            episode.plugin,
            namespace,
            value,
            now
        )
        .fetch_one(self.db.reader())
        .await?;
        Ok(aired)
    }
}

#[cfg(test)]
#[path = "air_dates_tests.rs"]
mod tests;
