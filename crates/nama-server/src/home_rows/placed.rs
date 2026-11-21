//! What a Home layout places each Home row by, and the rows an Admin can place in one.

use nama_plugin_api::{
    PluginId,
    home_row::{HomeRowMethod, HomeRowSource},
};

use super::{HomeRows, Row, store::Scope};
use crate::server_settings::HomeRowId;

impl HomeRows {
    /// Every row a Home layout places that each enabled Home row source gives now, each with its
    /// heading: in Plugin id order, its shared rows as last stored, each in the Plugin's order, then
    /// its entry for each User's own rows, with no heading, if it gives them.
    ///
    /// # Errors
    ///
    /// If reading the stored rows fails.
    pub(crate) async fn placeable(&self) -> Result<Vec<(HomeRowId, String)>, sqlx::Error> {
        let stored = sqlx::query!(
            "SELECT plugin_id, key, heading FROM home_rows WHERE user_id IS NULL
             ORDER BY plugin_id, position"
        )
        .fetch_all(self.db.reader())
        .await?;
        let shared: Vec<_> = (stored.into_iter())
            .map(|row| {
                let plugin = PluginId::from(row.plugin_id);
                let id = HomeRowId::Shared {
                    plugin,
                    key: row.key,
                };
                (id, row.heading)
            })
            .collect();
        let mut placeable = Vec::new();
        for plugin in self.points.providers::<dyn HomeRowSource>() {
            if self.gives(&plugin, HomeRowMethod::Shared).await {
                let its = shared.iter().filter(|(id, _)| id.plugin() == Some(&plugin));
                placeable.extend(its.cloned());
            }
            if self.gives(&plugin, HomeRowMethod::User).await {
                placeable.push((HomeRowId::UsersOwn { plugin }, String::new()));
            }
        }
        Ok(placeable)
    }
}

/// What a Home layout places a row of `scope` by.
pub(super) fn placed_by((plugin, user): Scope, row: &Row) -> HomeRowId {
    let key = row.key.clone();
    match user {
        None => HomeRowId::Shared { plugin, key },
        Some(_) => HomeRowId::Own { plugin, key },
    }
}
