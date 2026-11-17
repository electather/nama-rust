//! A Library's Backdrop: that of the title most recently added to it.

use std::collections::HashMap;

use nama_plugin_api::metadata::{ExternalId, ExternalIds};

use super::{Contents, ids_from_json, ids_sql, primary_id_in};

/// A Library's downloaded Backdrop: see [`Contents::newest_backdrops`].
#[derive(Debug)]
pub(crate) struct NewestBackdrop {
    /// The Plugin whose title source serves the path, which may differ from the Match's.
    pub(crate) plugin: String,
    pub(crate) path: String,
    /// The Movie's id, or for an Episode its Show's, whose own Custom Backdrop goes first.
    pub(crate) title: ExternalId,
    /// Every id of that title, any of which a Custom Backdrop may be kept under.
    pub(crate) ids: ExternalIds,
}

impl Contents {
    /// The downloaded Backdrop of the Matched title most recently added to each Library, of
    /// those with one, by Library id. A Library with no such title has none.
    pub(crate) async fn newest_backdrops(
        &self,
    ) -> Result<HashMap<i64, NewestBackdrop>, sqlx::Error> {
        // Item ids are AUTOINCREMENT, so the highest is the Item added last.
        macro_rules! select {
            ($ids:literal) => {
                sqlx::query!(
                    r#"SELECT library_id AS "library!", plugin AS "plugin!",
                              backdrop AS "backdrop!", ids AS "ids?: String",
                              title_sources AS "title_sources!"
                       FROM (SELECT library_files.library_id,
                                    item_matches.backdrop_plugin AS plugin,
                                    item_matches.backdrop, libraries.title_sources, "#
                        + $ids
                        + r#"IIF(library_items.kind = 'movie', 'title', 'show')) AS ids,
                                    ROW_NUMBER() OVER (PARTITION BY library_files.library_id
                                                       ORDER BY library_items.id DESC) AS newest
                             FROM item_matches
                             JOIN library_items ON library_items.id = item_matches.item_id
                             JOIN library_files ON library_files.id = library_items.file_id
                             JOIN libraries ON libraries.id = library_files.library_id
                             WHERE item_matches.state = 'matched'
                               AND item_matches.backdrop IS NOT NULL)
                       WHERE newest = 1"#
                )
            };
        }
        let rows = ids_sql!(select).fetch_all(self.db.reader()).await?;
        Ok(rows
            .into_iter()
            .filter_map(|row| {
                let ids = ids_from_json(row.ids.as_deref());
                let backdrop = NewestBackdrop {
                    title: primary_id_in(&ids, &row.title_sources)?,
                    ids,
                    plugin: row.plugin,
                    path: row.backdrop,
                };
                Some((row.library, backdrop))
            })
            .collect())
    }
}
