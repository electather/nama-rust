//! Where a title is Matched in any Library, which Requests and Custom Images ask before they act.

use std::collections::HashSet;

use nama_plugin_api::metadata::ExternalId;
use sqlx::SqliteConnection;

/// The oldest Library with an Item Matched to the Movie `id`, by any of its ids, if any, read
/// on `conn` so a write can act on the answer before a scan changes it.
pub(crate) async fn library_with_movie(
    conn: &mut SqliteConnection,
    id: &ExternalId,
) -> Result<Option<i64>, sqlx::Error> {
    let (namespace, value) = (id.namespace(), id.value());
    sqlx::query_scalar!(
        r#"SELECT MIN(library_files.library_id) AS "library: i64"
           FROM item_match_ids
           JOIN item_matches ON item_matches.item_id = item_match_ids.item_id
           JOIN library_items ON library_items.id = item_matches.item_id
           JOIN library_files ON library_files.id = library_items.file_id
           WHERE item_match_ids.scope = 'title'
             AND item_match_ids.namespace = ?
             AND item_match_ids.value = ?
             AND item_matches.state = 'matched'
             AND library_items.kind = 'movie'"#,
        namespace,
        value
    )
    .fetch_one(&mut *conn)
    .await
}

/// The oldest Library with an Episode Matched to the Show `id`, by any of its ids, if any.
pub(crate) async fn library_with_show(
    conn: &mut SqliteConnection,
    id: &ExternalId,
) -> Result<Option<i64>, sqlx::Error> {
    let (namespace, value) = (id.namespace(), id.value());
    sqlx::query_scalar!(
        r#"SELECT MIN(library_files.library_id) AS "library: i64"
           FROM item_match_ids
           JOIN item_matches ON item_matches.item_id = item_match_ids.item_id
           JOIN library_items ON library_items.id = item_matches.item_id
           JOIN library_files ON library_files.id = library_items.file_id
           WHERE item_match_ids.scope = 'show'
             AND item_match_ids.namespace = ?
             AND item_match_ids.value = ?
             AND item_matches.state = 'matched'
             AND library_items.kind = 'episode'"#,
        namespace,
        value
    )
    .fetch_one(&mut *conn)
    .await
}

/// The numbers of the Episodes of Season `season` of the Show `show`, by any of its ids,
/// Matched in any Library.
pub(crate) async fn matched_episodes(
    conn: &mut SqliteConnection,
    show: &ExternalId,
    season: i64,
) -> Result<HashSet<i64>, sqlx::Error> {
    let (namespace, value) = (show.namespace(), show.value());
    // Matching finds an Episode by the numbers its file's name gives, so they are the Episode's.
    let numbers = sqlx::query_scalar!(
        r#"SELECT library_items.episode AS "episode!"
           FROM item_match_ids
           JOIN item_matches ON item_matches.item_id = item_match_ids.item_id
           JOIN library_items ON library_items.id = item_matches.item_id
           WHERE item_match_ids.scope = 'show'
             AND item_match_ids.namespace = ?
             AND item_match_ids.value = ?
             AND item_matches.state = 'matched'
             AND library_items.kind = 'episode'
             AND library_items.season = ?"#,
        namespace,
        value,
        season
    )
    .fetch_all(&mut *conn)
    .await?;
    Ok(numbers.into_iter().collect())
}
