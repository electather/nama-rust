//! Where a title is Matched in any Library, which Requests, Custom Images and Watch sync ask.

use std::collections::HashSet;

use nama_plugin_api::metadata::{ExternalId, ExternalIds};
use sqlx::SqliteConnection;

use super::{Contents, ids_from_json, ids_sql};

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

/// Where an Episode sits in its Show, as the file Matched to it numbers it.
#[derive(Debug)]
pub(crate) struct EpisodeNumbers {
    /// Every id of its Show.
    pub(crate) show_ids: ExternalIds,
    pub(crate) season: i64,
    pub(crate) episode: i64,
}

impl Contents {
    /// The Show, season and number of the Episode known by any of `ids`, Matched in any Library,
    /// or `None` if none has it. Where several Items have it, the oldest Library's first by path
    /// answers.
    pub(crate) async fn episode_numbers(
        &self,
        ids: &[ExternalId],
    ) -> Result<Option<EpisodeNumbers>, sqlx::Error> {
        let asked: Vec<[&str; 2]> = (ids.iter())
            .map(|id| [id.namespace(), id.value()])
            .collect();
        // Pairs, not `ids_json`'s joined strings, so each is found through `item_match_ids_by_id`.
        let asked = serde_json::Value::from(asked).to_string();
        // Matching finds an Episode by the numbers its file's name gives, so they are the Episode's.
        macro_rules! select {
            ($ids:literal) => {
                sqlx::query!(
                    r#"SELECT library_items.season AS "season!", library_items.episode AS "episode!", "#
                        + $ids
                        + r#"'show') AS "show_ids?: String"
                       FROM json_each(?) AS asked
                       JOIN item_match_ids AS named
                           ON named.scope = 'title'
                          AND named.namespace = json_extract(asked.value, '$[0]')
                          AND named.value = json_extract(asked.value, '$[1]')
                       JOIN item_matches ON item_matches.item_id = named.item_id
                       JOIN library_items ON library_items.id = item_matches.item_id
                       JOIN library_files ON library_files.id = library_items.file_id
                       WHERE item_matches.state = 'matched' AND library_items.kind = 'episode'
                       ORDER BY library_files.library_id, library_files.path
                       LIMIT 1"#,
                    asked
                )
            };
        }
        let row = ids_sql!(select).fetch_optional(self.db.reader()).await?;
        Ok(row.map(|row| EpisodeNumbers {
            show_ids: ids_from_json(row.show_ids.as_deref()),
            season: row.season,
            episode: row.episode,
        }))
    }
}

#[cfg(test)]
#[path = "any_library_tests.rs"]
mod tests;
