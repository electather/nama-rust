//! One page of a Library's Matched Movies or Episodes, as an Automation Plugin lists them, and
//! what it reads of the titles on one.

use nama_plugin_api::metadata::{ExternalId, ExternalIds};

use super::{
    Contents, ItemKind, ids_from_json, ids_sql, listed_id_sql, matched::genres, parse_hint,
};

/// A Matched Movie or Episode on a page [`Contents::titles_page`] reads, with what the Match of
/// its first Version, by path, stores.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Listed {
    /// Its preferred id in its Library's title source order: see [`primary_id`](super::primary_id).
    /// It is also the page's order.
    pub(crate) id: ExternalId,
    pub(crate) title: String,
    /// The title of an Episode's Show; `None` for a Movie.
    pub(crate) show_title: Option<String>,
    /// An Episode's Season number; `None` for a Movie.
    pub(crate) season: Option<i64>,
    /// An Episode's number within its Season; `None` for a Movie.
    pub(crate) episode: Option<i64>,
    /// When a scan stored its newest Version, in seconds since the Unix epoch.
    pub(crate) added_at: i64,
    /// A Movie's genres, or an Episode's Show's.
    pub(crate) genres: Vec<String>,
    /// A Movie's Content Rating, or an Episode's Show's.
    pub(crate) content_rating: Option<String>,
    /// An Episode's Show's preferred id, picked as `id` is; `None` for a Movie.
    pub(crate) show_id: Option<ExternalId>,
}

impl Contents {
    /// Up to `limit` of the Library `library`'s Matched titles of `kind`, all of them for `None`,
    /// that come after `after`, by preferred id: namespace, then shorter value, then value, so a
    /// title Matched between two pages never moves one already listed. An Episode without a Show
    /// id is left out, as [`Contents::shows`] leaves it out.
    #[expect(
        clippy::too_many_lines,
        reason = "one query, whose columns each row fills a title with"
    )]
    pub(crate) async fn titles_page(
        &self,
        library: i64,
        kind: ItemKind,
        after: Option<&ExternalId>,
        limit: Option<usize>,
    ) -> Result<Vec<Listed>, sqlx::Error> {
        let (after_namespace, after_value) = after.map(|id| (id.namespace(), id.value())).unzip();
        // SQLite reads a negative LIMIT as none.
        let limit = limit.map_or(-1, |limit| i64::try_from(limit).unwrap_or(i64::MAX));
        // An Item's preferred id is its `title` id whose namespace comes first in the Library's
        // comma-separated `title_sources`, or else the first by namespace, as `primary_id` picks.
        macro_rules! select {
            ($listed_id:literal) => {
                sqlx::query!(
                    r#"WITH preferred AS (
                           SELECT ids.namespace, ids.value, item_matches.title,
                                  item_matches.show_title, library_items.season,
                                  library_items.episode, library_items.added_at,
                                  library_files.path, item_matches.genres,
                                  item_matches.content_rating, "#
                        + $listed_id
                        + r#"IIF(library_items.kind = 'movie', 'title', 'show')) AS listed_id,
                                  ROW_NUMBER() OVER (
                                      PARTITION BY ids.item_id
                                      ORDER BY instr(',' || libraries.title_sources || ',',
                                                     ',' || ids.namespace || ',') = 0,
                                               instr(',' || libraries.title_sources || ',',
                                                     ',' || ids.namespace || ','),
                                               ids.namespace) AS rank
                           FROM item_matches
                           JOIN item_match_ids AS ids ON ids.item_id = item_matches.item_id
                                                     AND ids.scope = 'title'
                           JOIN library_items ON library_items.id = item_matches.item_id
                           JOIN library_files ON library_files.id = library_items.file_id
                           JOIN libraries ON libraries.id = library_files.library_id
                           WHERE library_files.library_id = ?1
                             AND library_items.kind = ?2
                             AND item_matches.state = 'matched'
                             AND (library_items.kind = 'movie'
                                  OR EXISTS (SELECT 1 FROM item_match_ids AS show
                                             WHERE show.item_id = item_matches.item_id
                                               AND show.scope = 'show'))),
                       listed AS (
                           SELECT *,
                                  ROW_NUMBER() OVER (PARTITION BY namespace, value ORDER BY path)
                                      AS version,
                                  MAX(added_at) OVER (PARTITION BY namespace, value)
                                      AS newest_added_at
                           FROM preferred
                           WHERE rank = 1
                             AND (?3 IS NULL
                                  OR (namespace, length(value), value) > (?3, length(?4), ?4)))
                       SELECT namespace AS "namespace!", value AS "value!", title AS "title!",
                              show_title, season, episode,
                              newest_added_at AS "newest_added_at!: i64",
                              genres, content_rating, listed_id AS "listed_id?: String"
                       FROM listed
                       WHERE version = 1
                       ORDER BY namespace, length(value), value
                       LIMIT ?5"#,
                    library,
                    kind,
                    after_namespace,
                    after_value,
                    limit
                )
            };
        }
        let rows = listed_id_sql!(select).fetch_all(self.db.reader()).await?;
        let mut listed = Vec::with_capacity(rows.len());
        for row in rows {
            let Ok(id) = ExternalId::new(&row.namespace, row.value) else {
                continue;
            };
            let episode = kind == ItemKind::Episode;
            listed.push(Listed {
                id,
                title: row.title,
                show_title: row.show_title.filter(|_| episode),
                season: row.season.filter(|_| episode),
                episode: row.episode.filter(|_| episode),
                added_at: row.newest_added_at,
                genres: genres(row.genres.as_deref())?,
                content_rating: row.content_rating,
                // A Movie is listed under its own id, which isn't a Show's.
                show_id: row
                    .listed_id
                    .as_deref()
                    .and_then(parse_hint)
                    .filter(|_| episode),
            });
        }
        Ok(listed)
    }

    /// Every id the Match gives the Show of the Episode `episode` of the Library `library`, and
    /// the Episode's Season number, as its first Version, by path, stores them. `None` if the
    /// Library doesn't have it.
    pub(crate) async fn season_of(
        &self,
        library: i64,
        episode: &ExternalId,
    ) -> Result<Option<(ExternalIds, i64)>, sqlx::Error> {
        let (namespace, value) = (episode.namespace(), episode.value());
        macro_rules! select {
            ($ids:literal) => {
                sqlx::query!(
                    r#"SELECT library_items.season AS "season!", "#
                        + $ids
                        + r#"'show') AS "ids?: String"
                       FROM item_match_ids AS named
                       JOIN item_matches ON item_matches.item_id = named.item_id
                       JOIN library_items ON library_items.id = item_matches.item_id
                       JOIN library_files ON library_files.id = library_items.file_id
                       WHERE named.scope = 'title' AND named.namespace = ? AND named.value = ?
                         AND library_files.library_id = ?
                         AND library_items.kind = 'episode'
                         AND item_matches.state = 'matched'
                       ORDER BY library_files.path
                       LIMIT 1"#,
                    namespace,
                    value,
                    library
                )
            };
        }
        let found = ids_sql!(select).fetch_optional(self.db.reader()).await?;
        Ok(found.map(|row| (ids_from_json(row.ids.as_deref()), row.season)))
    }
}

#[cfg(test)]
#[path = "listed_tests.rs"]
mod tests;
