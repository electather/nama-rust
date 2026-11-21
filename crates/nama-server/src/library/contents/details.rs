//! What a title source says of a whole Movie or Show, stored once per title in `title_details`
//! rather than on each of its Matches (ADR 0081). [`Contents::movies`](super::Contents::movies)
//! and [`Contents::shows`](super::Contents::shows) read it back by the same key.

use sqlx::SqliteConnection;

use super::Metadata;

/// Stores the Score and Original Language that `matched` gives its title, a Movie or an
/// Episode's Show, in place of what the same title source said of it before.
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
        "INSERT OR REPLACE INTO title_details
             (plugin, kind, namespace, value, score, original_language)
         VALUES (?, ?, ?, ?, ?, ?)",
        plugin,
        kind,
        namespace,
        value,
        matched.score,
        matched.original_language,
    )
    .execute(conn)
    .await?;
    Ok(())
}

#[cfg(test)]
#[path = "details_tests.rs"]
mod tests;
