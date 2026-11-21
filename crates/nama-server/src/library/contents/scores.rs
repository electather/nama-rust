//! A title's Score, stored once per title in `title_scores` rather than on each of its Matches
//! (ADR 0081). [`Contents::movies`](super::Contents::movies) and
//! [`Contents::shows`](super::Contents::shows) read it back by the same key.

use nama_plugin_api::metadata::ExternalId;
use sqlx::SqliteConnection;

use super::Metadata;

/// Stores the Score that `matched` gives its title, a Movie or an Episode's Show, in place of what
/// the same title source gave it before.
pub(super) async fn store(
    conn: &mut SqliteConnection,
    matched: &Metadata,
) -> Result<(), sqlx::Error> {
    let Some((plugin, kind, id)) = title_key(matched) else {
        return Ok(());
    };
    let (namespace, value) = (id.namespace(), id.value());
    sqlx::query!(
        "INSERT OR REPLACE INTO title_scores (plugin, kind, namespace, value, score)
         VALUES (?, ?, ?, ?, ?)",
        plugin,
        kind,
        namespace,
        value,
        matched.score,
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// What a per-title store keeps the title `matched` gives under: the Plugin that Matched it, Movie
/// or Show, and its first id in namespace order, which reads look it up by. `None` without ids.
pub(super) fn title_key(matched: &Metadata) -> Option<(&str, &'static str, ExternalId)> {
    let (kind, ids) = if matched.show_title.is_some() {
        ("show", &matched.show_ids)
    } else {
        ("movie", &matched.ids)
    };
    Some((matched.plugin.as_str(), kind, ids.iter().next()?))
}

#[cfg(test)]
#[path = "scores_tests.rs"]
mod tests;
