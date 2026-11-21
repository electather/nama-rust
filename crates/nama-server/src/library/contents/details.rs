//! A title's Score and Trailers, stored once per title in `title_details` rather than on each of
//! its Matches (ADR 0081). [`Contents::movies`](super::Contents::movies) and
//! [`Contents::shows`](super::Contents::shows) read them back by the same key.

use sqlx::SqliteConnection;

use super::Metadata;
use crate::library::title_source::Trailer;

/// Stores the Score and Trailers that `matched` gives its title, a Movie or an Episode's Show, in
/// place of what the same title source gave it before.
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
    let trailers =
        serde_json::to_string(&matched.trailers).map_err(|err| sqlx::Error::Encode(err.into()))?;
    sqlx::query!(
        "INSERT OR REPLACE INTO title_details (plugin, kind, namespace, value, score, trailers)
         VALUES (?, ?, ?, ?, ?, ?)",
        plugin,
        kind,
        namespace,
        value,
        matched.score,
        trailers,
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// The Trailers in the `trailers` column's JSON, `json`: none where there's no row, as for a
/// title Matched through another source.
pub(super) fn trailers(json: Option<&str>) -> Result<Vec<Trailer>, sqlx::Error> {
    json.map_or(Ok(Vec::new()), |json| {
        serde_json::from_str(json).map_err(|err| sqlx::Error::Decode(err.into()))
    })
}

#[cfg(test)]
#[path = "details_tests.rs"]
mod tests;
