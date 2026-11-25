//! A title's ids as Contents stores and reads them: a Match's in `item_match_ids`, a Pin's as its
//! text form in `match_pins.pinned_id`.

use std::collections::HashMap;

use nama_plugin_api::metadata::{ExternalId, ExternalIds, IMDB, TMDB, TVDB};
use sqlx::SqliteConnection;

use super::pins::Pin;
use crate::{library::title_source::TitleKind, playback::title_ids_changed};

impl Pin {
    /// The pin stored in a `match_pins` row, if there is one, holding the id `id` in its text
    /// form. An id that doesn't parse is no pin, as a bad stored ID Hint is no hint.
    pub(super) fn from_row(pinned: bool, id: Option<&str>) -> Option<Self> {
        if !pinned {
            return None;
        }
        match id {
            None => Some(Self::Unmatched),
            Some(id) => parse_hint(id).map(Self::Title),
        }
    }
}

/// The namespaces a title's id is preferred in where no Library says: TMDB's, then IMDb's,
/// then TVDB's. A Request, which belongs to no Library, keeps a title under it.
pub(crate) const DEFAULT_ORDER: [&str; 3] = [TMDB, IMDB, TVDB];

/// The one of `ids` that identifies a title: the first in a namespace of `order`, a Library's
/// title sources or [`DEFAULT_ORDER`], or else the first in namespace order. `None` if it has
/// none.
pub(crate) fn primary_id(ids: &ExternalIds, order: &[impl AsRef<str>]) -> Option<ExternalId> {
    (order.iter())
        .find_map(|namespace| ids.get(namespace.as_ref()))
        .or_else(|| ids.iter().next())
}

/// [`primary_id`] in the order a Library stores as `libraries.title_sources`, comma-separated.
pub(super) fn primary_id_in(ids: &ExternalIds, title_sources: &str) -> Option<ExternalId> {
    primary_id(ids, &title_sources.split(',').collect::<Vec<_>>())
}

/// Hands `$query!` the start of a subquery giving an Item's ids in one scope of `item_match_ids`
/// as a JSON object of namespace to value; the caller closes it with the scope, as in `'title')`.
/// [`ids_from_json`] reads it. A macro because sqlx reads a query from literals only; `$arg`s go
/// to `$query!` first, so another such macro, such as `matched_items`, can hand on its own too.
macro_rules! ids_sql {
    ($query:ident $(, $arg:tt)*) => {
        $query!(
            $($arg,)*
            "(SELECT json_group_object(namespace, value) FROM item_match_ids
              WHERE item_match_ids.item_id = item_matches.item_id AND scope = "
        )
    };
}
pub(super) use ids_sql;

/// The ids an [`ids_sql`] subquery returned. A pair that isn't a valid id is skipped.
pub(super) fn ids_from_json(json: Option<&str>) -> ExternalIds {
    let pairs: HashMap<String, String> = json
        .and_then(|json| serde_json::from_str(json).ok())
        .unwrap_or_default();
    (pairs.into_iter())
        .filter_map(|(namespace, value)| ExternalId::new(&namespace, value).ok())
        .collect()
}

/// `ids` as a JSON array of their text forms, in order.
pub(super) fn ids_json(ids: &[ExternalId]) -> String {
    serde_json::Value::from_iter(ids.iter().map(ToString::to_string)).to_string()
}

/// `id` and every id a Matched title of `kind`, in any Library, shares with it, through any
/// chain of Matches each sharing an id with the last, since each Library's Match of a title may
/// hold a different set of its ids. Read on `conn` so a write can act on it; a stored pair that
/// isn't a valid id is skipped.
pub(crate) async fn matched_ids(
    conn: &mut SqliteConnection,
    kind: TitleKind,
    id: &ExternalId,
) -> Result<Vec<ExternalId>, sqlx::Error> {
    // A Show's ids are the `show` scope of its Episodes' Matches; an Episode's `title` ids are
    // its own, so a Movie's are read only off Movies.
    let (scope, item_kind) = match kind {
        TitleKind::Movie => ("title", "movie"),
        TitleKind::Show => ("show", "episode"),
    };
    let (namespace, value) = (id.namespace(), id.value());
    // UNION, not UNION ALL, drops ids already found, so the chain ends.
    let rows = sqlx::query!(
        r#"WITH RECURSIVE found (namespace, value) AS (
               SELECT ?1, ?2
               UNION
               SELECT other.namespace, other.value
               FROM found
               JOIN item_match_ids AS named
                   ON named.scope = ?3 AND named.namespace = found.namespace
                  AND named.value = found.value
               JOIN item_matches ON item_matches.item_id = named.item_id
               JOIN library_items ON library_items.id = named.item_id
               JOIN item_match_ids AS other
                   ON other.item_id = named.item_id AND other.scope = named.scope
               WHERE item_matches.state = 'matched' AND library_items.kind = ?4
           )
           SELECT namespace AS "namespace!: String", value AS "value!: String" FROM found"#,
        namespace,
        value,
        scope,
        item_kind
    )
    .fetch_all(conn)
    .await?;
    Ok((rows.into_iter())
        .filter_map(|row| ExternalId::new(&row.namespace, row.value).ok())
        .collect())
}

/// Stores `ids` as the Item `item`'s title's and `show_ids` as its Show's, in place of those it
/// had.
pub(super) async fn insert_match_ids(
    conn: &mut SqliteConnection,
    item: i64,
    ids: &ExternalIds,
    show_ids: &ExternalIds,
) -> Result<(), sqlx::Error> {
    // Deleted explicitly rather than trusting SQLite to cascade the delete REPLACE makes.
    sqlx::query!("DELETE FROM item_match_ids WHERE item_id = ?", item)
        .execute(&mut *conn)
        .await?;
    let ids = (ids.iter().map(|id| ("title", id))).chain(show_ids.iter().map(|id| ("show", id)));
    for (scope, id) in ids {
        let (namespace, value) = (id.namespace(), id.value());
        sqlx::query!(
            "INSERT INTO item_match_ids (item_id, scope, namespace, value) VALUES (?, ?, ?, ?)",
            item,
            scope,
            namespace,
            value
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

/// The ids the Item `item`'s Version is searched by, as [`FirstItem`](super::items::FirstItem)
/// reads them: a Movie's own, an Episode's Show's. None unless it's Matched.
pub(super) async fn searched_ids(
    conn: &mut SqliteConnection,
    item: i64,
) -> Result<ExternalIds, sqlx::Error> {
    let rows = sqlx::query!(
        "SELECT ids.namespace, ids.value FROM item_match_ids AS ids
         JOIN library_items ON library_items.id = ids.item_id
         WHERE ids.item_id = ?
           AND ids.scope = IIF(library_items.kind = 'movie', 'title', 'show')",
        item
    )
    .fetch_all(conn)
    .await?;
    Ok((rows.into_iter())
        .filter_map(|row| ExternalId::new(&row.namespace, row.value).ok())
        .collect())
}

/// Tells Segments, inside the write that changed the Item `item`'s Match, that its
/// [`searched_ids`] were `old` before it (ADR 0075).
pub(super) async fn searched_ids_changed(
    conn: &mut SqliteConnection,
    item: i64,
    old: &ExternalIds,
) -> Result<(), sqlx::Error> {
    let new = searched_ids(&mut *conn, item).await?;
    title_ids_changed(conn, item, old, &new).await
}

/// The ID Hint stored as `text`, which is how [`ExternalId`] displays: any well-formed id.
pub(crate) fn parse_hint(text: &str) -> Option<ExternalId> {
    text.parse().ok()
}

#[cfg(test)]
#[path = "ids_tests.rs"]
mod tests;
