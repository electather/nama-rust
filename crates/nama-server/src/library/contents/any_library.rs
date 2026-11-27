//! Where a title is Matched in any Library, which Requests, Custom Images and Watch sync ask.

use std::collections::HashSet;

use nama_plugin_api::metadata::{ExternalId, ExternalIds};
use sqlx::SqliteConnection;

use super::{Contents, ids_from_json, ids_sql};
use crate::library::TitleKind;

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

/// Each Library with the title of `kind` known by `id` Matched, by any of its ids, oldest first,
/// with each Content Rating its Matches there store, an Episode's its Show's. Read on `conn` so a
/// write can act on the answer before a scan changes it.
pub(crate) async fn copies_of(
    conn: &mut SqliteConnection,
    kind: TitleKind,
    id: &ExternalId,
) -> Result<Vec<(i64, Option<String>)>, sqlx::Error> {
    let (scope, item_kind) = match kind {
        TitleKind::Movie => ("title", "movie"),
        TitleKind::Show => ("show", "episode"),
    };
    let (namespace, value) = (id.namespace(), id.value());
    let copies = sqlx::query!(
        r#"SELECT DISTINCT library_files.library_id AS "library!", item_matches.content_rating
           FROM item_match_ids
           JOIN item_matches ON item_matches.item_id = item_match_ids.item_id
           JOIN library_items ON library_items.id = item_matches.item_id
           JOIN library_files ON library_files.id = library_items.file_id
           WHERE item_match_ids.scope = ?
             AND item_match_ids.namespace = ?
             AND item_match_ids.value = ?
             AND item_matches.state = 'matched'
             AND library_items.kind = ?
           ORDER BY library_files.library_id"#,
        scope,
        namespace,
        value,
        item_kind
    )
    .fetch_all(&mut *conn)
    .await?;
    Ok((copies.into_iter())
        .map(|copy| (copy.library, copy.content_rating))
        .collect())
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
        let asked = id_pairs_json(ids);
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

    /// The ids of Episode `number` of Season `season` of the Show known by any of `show_ids`,
    /// Matched in any Library, or none if none has it. Where several Items have it, the oldest
    /// Library's first by path answers.
    pub(in crate::library) async fn episode_ids(
        &self,
        show_ids: &[ExternalId],
        (season, number): (i64, i64),
    ) -> Result<ExternalIds, sqlx::Error> {
        let asked = id_pairs_json(show_ids);
        macro_rules! select {
            ($ids:literal) => {
                sqlx::query_scalar!(
                    r#"SELECT "#
                        + $ids
                        + r#"'title') AS "ids?: String"
                       FROM json_each(?) AS asked
                       JOIN item_match_ids AS show
                           ON show.scope = 'show'
                          AND show.namespace = json_extract(asked.value, '$[0]')
                          AND show.value = json_extract(asked.value, '$[1]')
                       JOIN item_matches ON item_matches.item_id = show.item_id
                       JOIN library_items ON library_items.id = item_matches.item_id
                       JOIN library_files ON library_files.id = library_items.file_id
                       WHERE item_matches.state = 'matched'
                         AND library_items.kind = 'episode'
                         AND library_items.season = ?
                         AND library_items.episode = ?
                       ORDER BY library_files.library_id, library_files.path
                       LIMIT 1"#,
                    asked,
                    season,
                    number
                )
            };
        }
        let ids = ids_sql!(select).fetch_optional(self.db.reader()).await?;
        Ok(ids_from_json(ids.flatten().as_deref()))
    }
}

/// `ids` as a JSON array of `[namespace, value]` pairs: pairs, not `ids_json`'s joined strings, so
/// a query finds each through `item_match_ids_by_id`.
fn id_pairs_json(ids: &[ExternalId]) -> String {
    let pairs: Vec<[&str; 2]> = (ids.iter())
        .map(|id| [id.namespace(), id.value()])
        .collect();
    serde_json::Value::from(pairs).to_string()
}

#[cfg(test)]
#[path = "any_library_tests.rs"]
mod tests;
