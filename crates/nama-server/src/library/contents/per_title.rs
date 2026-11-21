//! What is stored once per title rather than on each of its Matches (ADR 0081): a title's Score,
//! in `title_scores`, and its Similar titles, in `similar_titles`, both keyed alike.
//! [`Contents::movies`](super::Contents::movies) and [`Contents::shows`](super::Contents::shows)
//! read the Score back, and [`Contents::similar`] the Similar titles.

use nama_plugin_api::metadata::{ExternalId, ExternalIds};
use sqlx::SqliteConnection;

use super::{Contents, Metadata};
use crate::library::TitleKind;

/// Stores the Score and Similar titles that `matched` gives its title, a Movie or an Episode's
/// Show, in place of what the same title source gave it before.
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
    let (plugin, namespace, value) = (matched.plugin.as_str(), id.namespace(), id.value());
    sqlx::query!(
        "INSERT OR REPLACE INTO title_scores (plugin, kind, namespace, value, score)
         VALUES (?, ?, ?, ?, ?)",
        plugin,
        kind,
        namespace,
        value,
        matched.score,
    )
    .execute(&mut *conn)
    .await?;
    let similar: Vec<String> = matched.similar.iter().map(ToString::to_string).collect();
    let similar = serde_json::Value::from(similar).to_string();
    sqlx::query!(
        "INSERT OR REPLACE INTO similar_titles (plugin, kind, namespace, value, similar)
         VALUES (?, ?, ?, ?, ?)",
        plugin,
        kind,
        namespace,
        value,
        similar,
    )
    .execute(conn)
    .await?;
    Ok(())
}

impl Contents {
    /// The Similar titles of the Movie or Show of `kind` known by `ids`, as the Match of it in
    /// `library` stores them, from that Match's title source alone, most alike first. Empty if
    /// it gave none, or the Match predates them.
    pub(crate) async fn similar(
        &self,
        library: i64,
        kind: TitleKind,
        ids: &ExternalIds,
    ) -> Result<Vec<ExternalId>, sqlx::Error> {
        // The rows of a Movie's Match name it in the `title` scope, and an Episode's its Show in
        // the `show` scope.
        let scope = match kind {
            TitleKind::Movie => "title",
            TitleKind::Show => "show",
        };
        let Some((kind, id)) = key(kind, ids) else {
            return Ok(Vec::new());
        };
        let (namespace, value) = (id.namespace(), id.value());
        let similar = sqlx::query_scalar!(
            "SELECT similar_titles.similar FROM similar_titles
             WHERE similar_titles.kind = ?1
               AND similar_titles.namespace = ?2
               AND similar_titles.value = ?3
               AND similar_titles.plugin IN (
                   SELECT item_matches.plugin FROM item_match_ids
                   JOIN item_matches ON item_matches.item_id = item_match_ids.item_id
                   JOIN library_items ON library_items.id = item_matches.item_id
                   JOIN library_files ON library_files.id = library_items.file_id
                   WHERE library_files.library_id = ?4
                     AND item_match_ids.scope = ?5
                     AND item_match_ids.namespace = ?2
                     AND item_match_ids.value = ?3
                     AND item_matches.state = 'matched')
             LIMIT 1",
            kind,
            namespace,
            value,
            library,
            scope,
        )
        .fetch_optional(self.db.reader())
        .await?;
        let similar: Vec<String> = (similar.as_deref())
            .and_then(|json| serde_json::from_str(json).ok())
            .unwrap_or_default();
        Ok(similar
            .iter()
            .filter_map(|named| named.parse().ok())
            .collect())
    }
}

/// The kind and the first of `ids` in namespace order, which a title's rows are kept under and
/// `movies` and `shows` look its Score up by. `None` for a title with no id.
fn key(kind: TitleKind, ids: &ExternalIds) -> Option<(&'static str, ExternalId)> {
    let kind = match kind {
        TitleKind::Movie => "movie",
        TitleKind::Show => "show",
    };
    Some((kind, ids.iter().next()?))
}

#[cfg(test)]
#[path = "per_title_tests.rs"]
mod tests;
