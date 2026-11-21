//! A title's Score, stored once per title in `title_scores` rather than on each of its Matches
//! (ADR 0081). [`Contents::movies`](super::Contents::movies) and
//! [`Contents::shows`](super::Contents::shows) read it back by the same key.

use sqlx::SqliteConnection;

use super::Metadata;

/// Stores the Score that `matched` gives its title, a Movie or an Episode's Show, in place of what
/// the same title source gave it before.
pub(super) async fn store(
    conn: &mut SqliteConnection,
    matched: &Metadata,
) -> Result<(), sqlx::Error> {
    let (kind, ids) = if matched.show_title.is_some() {
        ("show", &matched.show_ids)
    } else {
        ("movie", &matched.ids)
    };
    // The first id in namespace order, which `movies` and `shows` look it up by.
    let Some(id) = ids.iter().next() else {
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
    .execute(conn)
    .await?;
    Ok(())
}

#[cfg(test)]
#[path = "scores_tests.rs"]
mod tests;
