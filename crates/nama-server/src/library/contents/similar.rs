//! A title's Similar titles, stored once per title in `similar_titles` rather than on each of its
//! Matches (ADR 0081), one row per Similar source that answered (ADR 0084). [`Contents::similar`]
//! reads them back.

use nama_plugin_api::{
    PluginId,
    metadata::{ExternalId, ExternalIds},
};
use serde::{Deserialize, Serialize};
use sqlx::SqliteConnection;

use super::{Contents, Metadata};
use crate::library::{SimilarAnswer, TitleKind};

/// One title of a stored answer, as the `similar` column's JSON holds it.
#[derive(Serialize, Deserialize)]
struct StoredTitle {
    id: String,
    weight: f32,
}

/// Stores each Similar source's answer in `matched` about its title, a Movie or an Episode's
/// Show, in place of what that source answered before. A source that didn't answer keeps its row.
pub(super) async fn store(
    conn: &mut SqliteConnection,
    matched: &Metadata,
) -> Result<(), sqlx::Error> {
    let (kind, ids) = if matched.show_title.is_some() {
        (TitleKind::Show, &matched.show_ids)
    } else {
        (TitleKind::Movie, &matched.ids)
    };
    let Some((kind, id)) = key(kind, ids) else {
        return Ok(());
    };
    let (namespace, value) = (id.namespace(), id.value());
    for answer in &matched.similar {
        let plugin = answer.plugin.as_str();
        let titles: Vec<StoredTitle> = (answer.titles.iter())
            .map(|(similar, weight)| StoredTitle {
                id: similar.to_string(),
                weight: *weight,
            })
            .collect();
        let titles =
            serde_json::to_string(&titles).map_err(|err| sqlx::Error::Encode(err.into()))?;
        sqlx::query!(
            "INSERT OR REPLACE INTO similar_titles (plugin, kind, namespace, value, similar)
             VALUES (?, ?, ?, ?, ?)",
            plugin,
            kind,
            namespace,
            value,
            titles,
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

impl Contents {
    /// What each Similar source last answered about the Movie or Show of `kind` known by `ids`,
    /// by Plugin id. Empty if none has, or its Match predates them.
    pub(crate) async fn similar(
        &self,
        kind: TitleKind,
        ids: &ExternalIds,
    ) -> Result<Vec<SimilarAnswer>, sqlx::Error> {
        let Some((kind, id)) = key(kind, ids) else {
            return Ok(Vec::new());
        };
        let (namespace, value) = (id.namespace(), id.value());
        let rows = sqlx::query!(
            "SELECT plugin, similar FROM similar_titles
             WHERE kind = ? AND namespace = ? AND value = ?
             ORDER BY plugin",
            kind,
            namespace,
            value,
        )
        .fetch_all(self.db.reader())
        .await?;
        rows.into_iter()
            .map(|row| {
                let stored: Vec<StoredTitle> = serde_json::from_str(&row.similar)
                    .map_err(|err| sqlx::Error::Decode(err.into()))?;
                let titles = (stored.into_iter())
                    .filter_map(|title| Some((title.id.parse().ok()?, title.weight)))
                    .collect();
                Ok(SimilarAnswer {
                    plugin: PluginId::from(row.plugin),
                    titles,
                })
            })
            .collect()
    }

    /// Deletes every answer the Plugin `plugin`'s Similar source gave, as disabling it does.
    pub(crate) async fn forget_similar_from(&self, plugin: &PluginId) -> Result<(), sqlx::Error> {
        let plugin = plugin.as_str().to_owned();
        self.db
            .write(move |tx| {
                Box::pin(async move {
                    sqlx::query!("DELETE FROM similar_titles WHERE plugin = ?", plugin)
                        .execute(&mut **tx)
                        .await?;
                    Ok(())
                })
            })
            .await
    }
}

/// The kind and the first of `ids` in namespace order, which a title's rows are kept under and
/// `similar` looks them up by. `None` for a title with no id.
fn key(kind: TitleKind, ids: &ExternalIds) -> Option<(&'static str, ExternalId)> {
    let kind = match kind {
        TitleKind::Movie => "movie",
        TitleKind::Show => "show",
    };
    Some((kind, ids.iter().next()?))
}
