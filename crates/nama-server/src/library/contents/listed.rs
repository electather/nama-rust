//! One page of a Library's Matched Movies or Episodes, as an Automation Plugin lists them, and
//! what it reads of the titles on one.

use nama_plugin_api::metadata::{ExternalId, ExternalIds};

use super::{
    Contents, ItemKind, ids_from_json, ids_sql, matched::genres, matched_items, parse_hint,
};

/// A Movie or Episode of `matched_items` on a page [`Contents::titles_page`] reads, with what the
/// Match of its first Version, by path, stores.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Listed {
    /// Its preferred id, its `own_id` in `matched_items`, which is also the page's order.
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
    /// An Episode's Show's preferred id, its `listed_id` in `matched_items`; `None` for a Movie.
    pub(crate) show_id: Option<ExternalId>,
    /// Every id of the Movie or Episode, `id` among them.
    pub(crate) ids: ExternalIds,
    /// Every id of an Episode's Show, `show_id` among them; empty for a Movie.
    pub(crate) show_ids: ExternalIds,
}

impl Contents {
    /// Up to `limit` of the Library `library`'s Matched titles of `kind`, all of them for `None`,
    /// that come after `after`, by preferred id: namespace, then shorter value, then value, so a
    /// title Matched between two pages never moves one already listed. Only the Items
    /// `matched_items` keeps are listed.
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
        // ponytail: whole-Library read. `own_id` splits at its first `-`, as `ExternalId` reads
        // it, so pages stay in id order. `items` is MATERIALIZED, or SQLite would run `own_id`'s
        // subquery again at each use.
        macro_rules! select {
            ($ids:literal, ($($matched:tt)+)) => {
                sqlx::query!(
                    r#"WITH items AS MATERIALIZED (
                           SELECT matched_items.own_id, matched_items.listed_id,
                                  matched_items.path, matched_items.season, matched_items.episode,
                                  matched_items.added_at, item_matches.title,
                                  item_matches.show_title, item_matches.genres,
                                  item_matches.content_rating, "#
                        + $ids
                        + r#"'title') AS ids, "#
                        + $ids
                        + r#"'show') AS show_ids
                           FROM "#
                        + $($matched)+
                        + r#" AS matched_items
                           JOIN item_matches ON item_matches.item_id = matched_items.item_id
                           WHERE matched_items.library_id = ?1 AND matched_items.kind = ?2),
                       listed AS (
                           SELECT *, substr(own_id, 1, instr(own_id, '-') - 1) AS namespace,
                                  substr(own_id, instr(own_id, '-') + 1) AS value,
                                  ROW_NUMBER() OVER (PARTITION BY own_id ORDER BY path)
                                      AS version,
                                  MAX(added_at) OVER (PARTITION BY own_id) AS newest_added_at
                           FROM items)
                       SELECT namespace AS "namespace!: String", value AS "value!: String",
                              title AS "title!", show_title, season, episode,
                              newest_added_at AS "newest_added_at!: i64",
                              genres, content_rating, listed_id AS "listed_id!: String",
                              ids AS "ids?: String", show_ids AS "show_ids?: String"
                       FROM listed
                       WHERE version = 1
                         AND (?3 IS NULL
                              OR (namespace, length(value), value) > (?3, length(?4), ?4))
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
        let rows = ids_sql!(matched_items, select)
            .fetch_all(self.db.reader())
            .await?;
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
                show_id: parse_hint(&row.listed_id).filter(|_| episode),
                ids: ids_from_json(row.ids.as_deref()),
                show_ids: ids_from_json(row.show_ids.as_deref().filter(|_| episode)),
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
