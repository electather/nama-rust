//! The images Matches store, with the headshots of the Roles and the thumbnails of the Trailers
//! of the titles they Match, which artwork serves and keeps while some Match stores them.

use nama_plugin_api::metadata::ExternalIds;

use super::{Contents, ids_from_json, ids_sql};
use crate::library::{Role, TitleImage};

/// A Poster or Backdrop a Match stores, as its Plugin and path, and whose it is: see
/// [`Contents::posters`] and [`Contents::backdrops`].
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct StoredTitleImage {
    pub(crate) plugin: String,
    pub(crate) path: String,
    /// Every id the Match gives the Movie, or the Episode's Show.
    pub(crate) ids: ExternalIds,
    pub(crate) image: TitleImage,
}

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
                   OR EXISTS (SELECT 1 FROM title_roles
                              WHERE plugin = ?1 AND headshot = ?2
                                -- Kept while a Match stores the title it's keyed by.
                                AND EXISTS (SELECT 1 FROM item_match_ids AS keyed
                                            JOIN item_matches
                                              ON item_matches.item_id = keyed.item_id
                                            WHERE item_matches.state = 'matched'
                                              AND item_matches.plugin = title_roles.plugin
                                              AND keyed.scope = iif(title_roles.kind = 'movie',
                                                                    'title', 'show')
                                              AND keyed.namespace = title_roles.namespace
                                              AND keyed.value = title_roles.value))
                   -- ponytail: reads every Trailer of `plugin`'s titles; index the thumbnails in a
                   -- table of their own if serving them gets slow.
                   OR EXISTS (SELECT 1 FROM title_details, json_each(title_details.trailers) AS trailer
                              WHERE title_details.plugin = ?1
                                AND json_extract(trailer.value, '$.thumbnail') = ?2
                                -- As a headshot is kept.
                                AND EXISTS (SELECT 1 FROM item_match_ids AS keyed
                                            JOIN item_matches
                                              ON item_matches.item_id = keyed.item_id
                                            WHERE item_matches.state = 'matched'
                                              AND item_matches.plugin = title_details.plugin
                                              AND keyed.scope = iif(title_details.kind = 'movie',
                                                                    'title', 'show')
                                              AND keyed.namespace = title_details.namespace
                                              AND keyed.value = title_details.value))
               AS "stored!: bool""#,
            plugin,
            image
        )
        .fetch_one(self.db.reader())
        .await
    }

    /// Each image a Match stores, and each headshot and Trailer thumbnail of the titles they
    /// Match, as its Plugin and path, in the Library `library` or else in every Library.
    pub(crate) async fn images(
        &self,
        library: Option<i64>,
    ) -> Result<Vec<(String, String)>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"WITH matched AS (
                   SELECT item_matches.item_id, item_matches.plugin, item_matches.poster,
                          item_matches.poster_plugin,
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
               SELECT plugin, still FROM matched WHERE still IS NOT NULL
               UNION
               SELECT title_roles.plugin, title_roles.headshot FROM title_roles
               WHERE title_roles.headshot IS NOT NULL
                 -- As `stores_image` keeps it.
                 AND EXISTS (SELECT 1 FROM matched
                             JOIN item_match_ids AS keyed ON keyed.item_id = matched.item_id
                             WHERE matched.plugin = title_roles.plugin
                               AND keyed.scope = iif(title_roles.kind = 'movie', 'title', 'show')
                               AND keyed.namespace = title_roles.namespace
                               AND keyed.value = title_roles.value)
               UNION
               SELECT title_details.plugin, json_extract(trailer.value, '$.thumbnail')
               FROM title_details, json_each(title_details.trailers) AS trailer
               WHERE json_extract(trailer.value, '$.thumbnail') IS NOT NULL
                 AND EXISTS (SELECT 1 FROM matched
                             JOIN item_match_ids AS keyed ON keyed.item_id = matched.item_id
                             WHERE matched.plugin = title_details.plugin
                               AND keyed.scope = iif(title_details.kind = 'movie', 'title', 'show')
                               AND keyed.namespace = title_details.namespace
                               AND keyed.value = title_details.value)"#,
            library
        )
        .fetch_all(self.db.reader())
        .await?;
        Ok(rows.into_iter().map(|row| (row.plugin, row.path)).collect())
    }

    /// Each titled Backdrop, and then each Title Logo, a Match in the Library `library` stores, as
    /// its Plugin and path.
    pub(crate) async fn titled_backdrops_and_title_logos(
        &self,
        library: i64,
    ) -> Result<(Vec<(String, String)>, Vec<(String, String)>), sqlx::Error> {
        let rows = sqlx::query!(
            r#"WITH matched AS (
                   SELECT item_matches.titled_backdrop, item_matches.titled_backdrop_plugin,
                          item_matches.logo, item_matches.logo_plugin
                   FROM item_matches
                   JOIN library_items ON library_items.id = item_matches.item_id
                   JOIN library_files ON library_files.id = library_items.file_id
                   WHERE item_matches.state = 'matched' AND library_files.library_id = ?)
               SELECT titled_backdrop_plugin AS "plugin!", titled_backdrop AS "path!",
                      FALSE AS "title_logo!: bool"
               FROM matched WHERE titled_backdrop IS NOT NULL
               UNION
               SELECT logo_plugin, logo, TRUE FROM matched WHERE logo IS NOT NULL"#,
            library
        )
        .fetch_all(self.db.reader())
        .await?;
        let (mut titled_backdrops, mut title_logos) = (Vec::new(), Vec::new());
        for row in rows {
            let kind = if row.title_logo {
                &mut title_logos
            } else {
                &mut titled_backdrops
            };
            kind.push((row.plugin, row.path));
        }
        Ok((titled_backdrops, title_logos))
    }

    /// Each Backdrop, not titled, a Match in the Library `library` stores, with whose it is: the
    /// Movie's, or the Episode's Show's.
    pub(crate) async fn backdrops(
        &self,
        library: i64,
    ) -> Result<Vec<StoredTitleImage>, sqlx::Error> {
        macro_rules! select {
            ($ids:literal) => {
                sqlx::query!(
                    r#"SELECT DISTINCT item_matches.backdrop_plugin AS "plugin!",
                              item_matches.backdrop AS "path!",
                              library_items.kind = 'movie' AS "of_movie!: bool", "#
                        + $ids
                        + r#"IIF(library_items.kind = 'movie', 'title', 'show'))
                              AS "ids?: String"
                       FROM item_matches
                       JOIN library_items ON library_items.id = item_matches.item_id
                       JOIN library_files ON library_files.id = library_items.file_id
                       WHERE item_matches.state = 'matched' AND library_files.library_id = ?
                         AND item_matches.backdrop IS NOT NULL"#,
                    library
                )
            };
        }
        let rows = ids_sql!(select).fetch_all(self.db.reader()).await?;
        Ok((rows.into_iter())
            .map(|row| StoredTitleImage {
                image: if row.of_movie {
                    TitleImage::Movie(Role::Backdrop)
                } else {
                    TitleImage::Show(Role::Backdrop)
                },
                plugin: row.plugin,
                path: row.path,
                ids: ids_from_json(row.ids.as_deref()),
            })
            .collect())
    }

    /// Each Poster, a title's, its Season's or its Show's, a Match in the Library `library`
    /// stores, with whose it is.
    pub(crate) async fn posters(&self, library: i64) -> Result<Vec<StoredTitleImage>, sqlx::Error> {
        macro_rules! select {
            ($ids:literal) => {
                sqlx::query!(
                    r#"WITH matched AS (
                           SELECT item_matches.poster, item_matches.poster_plugin,
                                  item_matches.show_poster, item_matches.show_poster_plugin,
                                  library_items.season, "#
                        + $ids
                        + r#"IIF(library_items.kind = 'movie', 'title', 'show')) AS ids
                           FROM item_matches
                           JOIN library_items ON library_items.id = item_matches.item_id
                           JOIN library_files ON library_files.id = library_items.file_id
                           WHERE item_matches.state = 'matched' AND library_files.library_id = ?)
                       SELECT poster_plugin AS "plugin!", poster AS "path!",
                              FALSE AS "of_show!: bool", season, ids AS "ids?: String"
                       FROM matched WHERE poster IS NOT NULL
                       UNION
                       SELECT show_poster_plugin, show_poster, TRUE, NULL, ids FROM matched
                       WHERE show_poster IS NOT NULL"#,
                    library
                )
            };
        }
        let rows = ids_sql!(select).fetch_all(self.db.reader()).await?;
        Ok((rows.into_iter())
            .map(|row| StoredTitleImage {
                // Only an Episode's Item has a Season (`library_items`' CHECK).
                image: match (row.of_show, row.season) {
                    (true, _) => TitleImage::Show(Role::Poster),
                    (false, Some(number)) => TitleImage::SeasonPoster(number),
                    (false, None) => TitleImage::Movie(Role::Poster),
                },
                plugin: row.plugin,
                path: row.path,
                ids: ids_from_json(row.ids.as_deref()),
            })
            .collect())
    }
}
