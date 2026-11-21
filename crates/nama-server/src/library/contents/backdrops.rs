//! A Library's Backdrop: that of the title most recently added to it.

use std::collections::HashMap;

use nama_plugin_api::metadata::{ExternalId, ExternalIds};

use super::{Contents, ids_from_json, ids_sql, library_titles, parse_hint};

/// A Library's downloaded Backdrop: see [`Contents::newest_backdrops`].
#[derive(Debug)]
pub(crate) struct NewestBackdrop {
    /// The Plugin whose title source serves the path, which may differ from the Match's.
    pub(crate) plugin: String,
    pub(crate) path: String,
    /// The title's listed id, whose own Custom Backdrop goes first.
    pub(crate) title: ExternalId,
    /// Every id of the Match the title shows, any of which a Custom Backdrop may be kept under.
    pub(crate) ids: ExternalIds,
}

impl Contents {
    /// The downloaded Backdrop each Library shows, by Library id: that of the title it lists that
    /// was added last, as `library_titles` has it, of those whose shown Match has one. A Library
    /// with no such title has none.
    pub(crate) async fn newest_backdrops(
        &self,
    ) -> Result<HashMap<i64, NewestBackdrop>, sqlx::Error> {
        // ponytail: whole-Library read, grouped by `library_titles`.
        macro_rules! select {
            ($ids:literal, ($($titles:tt)+)) => {
                sqlx::query!(
                    r#"SELECT library_id AS "library!: i64", listed_id AS "listed_id!: String",
                              plugin AS "plugin!: String", backdrop AS "backdrop!: String",
                              ids AS "ids?: String",
                              -- Read only to pick the row: SQLite takes a bare column from the
                              -- row MAX picks. Item ids are AUTOINCREMENT, so of titles added
                              -- at once the higher `title_item` was stored last.
                              MAX(printf('%020d%020d', added_at, title_item)) AS "newest!: String"
                       FROM (SELECT library_titles.library_id, library_titles.listed_id,
                                    library_titles.added_at, library_titles.title_item,
                                    item_matches.backdrop_plugin AS plugin, item_matches.backdrop, "#
                        + $ids
                        + r#"IIF(library_titles.kind = 'movie', 'title', 'show')) AS ids
                             FROM "#
                        + $($titles)+
                        + r#" AS library_titles
                             JOIN item_matches ON item_matches.item_id = library_titles.item_id
                             WHERE library_titles.nth = 1 AND item_matches.backdrop IS NOT NULL)
                       GROUP BY library_id"#
                )
            };
        }
        macro_rules! with_ids {
            ($ids:literal) => {
                library_titles!(select, $ids)
            };
        }
        let rows = ids_sql!(with_ids).fetch_all(self.db.reader()).await?;
        Ok(rows
            .into_iter()
            .filter_map(|row| {
                let backdrop = NewestBackdrop {
                    title: parse_hint(&row.listed_id)?,
                    ids: ids_from_json(row.ids.as_deref()),
                    plugin: row.plugin,
                    path: row.backdrop,
                };
                Some((row.library, backdrop))
            })
            .collect())
    }
}

#[cfg(test)]
#[path = "backdrops_tests.rs"]
mod tests;
