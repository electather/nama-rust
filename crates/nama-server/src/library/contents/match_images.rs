//! The images Matches store, which artwork serves and keeps while some Match stores them.

use super::Contents;

impl Contents {
    pub(crate) async fn stores_image(
        &self,
        plugin: &str,
        image: &str,
    ) -> Result<bool, sqlx::Error> {
        sqlx::query_scalar!(
            r#"SELECT EXISTS (SELECT 1 FROM item_matches
                              WHERE state = 'matched'
                                AND ((poster_plugin = ?1 AND poster = ?2)
                                     OR (backdrop_plugin = ?1 AND backdrop = ?2)
                                     OR (titled_backdrop_plugin = ?1 AND titled_backdrop = ?2)
                                     OR (logo_plugin = ?1 AND logo = ?2)
                                     OR (show_poster_plugin = ?1 AND show_poster = ?2)
                                     OR (plugin = ?1 AND still = ?2)))
               AS "stored!: bool""#,
            plugin,
            image
        )
        .fetch_one(self.db.reader())
        .await
    }

    /// Each image a Match stores, as its Plugin and path, in the Library `library` or else in
    /// every Library.
    pub(crate) async fn images(
        &self,
        library: Option<i64>,
    ) -> Result<Vec<(String, String)>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"WITH matched AS (
                   SELECT item_matches.plugin, item_matches.poster, item_matches.poster_plugin,
                          item_matches.backdrop, item_matches.backdrop_plugin,
                          item_matches.titled_backdrop, item_matches.titled_backdrop_plugin,
                          item_matches.logo, item_matches.logo_plugin, item_matches.still,
                          item_matches.show_poster, item_matches.show_poster_plugin
                   FROM item_matches
                   JOIN library_items ON library_items.id = item_matches.item_id
                   JOIN library_files ON library_files.id = library_items.file_id
                   WHERE item_matches.state = 'matched'
                     AND (?1 IS NULL OR library_files.library_id = ?1))
               SELECT poster_plugin AS "plugin!", poster AS "path!" FROM matched
               WHERE poster IS NOT NULL
               UNION
               SELECT backdrop_plugin, backdrop FROM matched WHERE backdrop IS NOT NULL
               UNION
               SELECT titled_backdrop_plugin, titled_backdrop FROM matched
               WHERE titled_backdrop IS NOT NULL
               UNION
               SELECT logo_plugin, logo FROM matched WHERE logo IS NOT NULL
               UNION
               SELECT show_poster_plugin, show_poster FROM matched WHERE show_poster IS NOT NULL
               UNION
               SELECT plugin, still FROM matched WHERE still IS NOT NULL"#,
            library
        )
        .fetch_all(self.db.reader())
        .await?;
        Ok(rows.into_iter().map(|row| (row.plugin, row.path)).collect())
    }
}
