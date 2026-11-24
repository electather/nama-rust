//! The images Matches store, with the headshots of the Roles of the titles they Match, which
//! artwork serves and keeps while some Match stores them.

use nama_plugin_api::metadata::ExternalIds;

use super::{Contents, ids_from_json, ids_sql};
use crate::library::{Role, TitleImage};

/// A Poster a Match stores, as its Plugin and path, and whose it is: see [`Contents::posters`].
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct TitlePoster {
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
               AS "stored!: bool""#,
            plugin,
            image
        )
        .fetch_one(self.db.reader())
        .await
    }

    /// Each image a Match stores, and each headshot of the Roles of the titles they Match, as its
    /// Plugin and path, in the Library `library` or else in every Library.
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
                               AND keyed.value = title_roles.value)"#,
            library
        )
        .fetch_all(self.db.reader())
        .await?;
        Ok(rows.into_iter().map(|row| (row.plugin, row.path)).collect())
    }

    /// Each Backdrop, titled ones among them, and then each Title Logo a Match in the Library
    /// `library` stores, as its Plugin and path.
    pub(crate) async fn backdrops_and_logos(
        &self,
        library: i64,
    ) -> Result<(Vec<(String, String)>, Vec<(String, String)>), sqlx::Error> {
        let rows = sqlx::query!(
            r#"WITH matched AS (
                   SELECT item_matches.backdrop, item_matches.backdrop_plugin,
                          item_matches.titled_backdrop, item_matches.titled_backdrop_plugin,
                          item_matches.logo, item_matches.logo_plugin
                   FROM item_matches
                   JOIN library_items ON library_items.id = item_matches.item_id
                   JOIN library_files ON library_files.id = library_items.file_id
                   WHERE item_matches.state = 'matched' AND library_files.library_id = ?)
               SELECT backdrop_plugin AS "plugin!", backdrop AS "path!", FALSE AS "logo!: bool"
               FROM matched WHERE backdrop IS NOT NULL
               UNION
               SELECT titled_backdrop_plugin, titled_backdrop, FALSE FROM matched
               WHERE titled_backdrop IS NOT NULL
               UNION
               SELECT logo_plugin, logo, TRUE FROM matched WHERE logo IS NOT NULL"#,
            library
        )
        .fetch_all(self.db.reader())
        .await?;
        let (mut backdrops, mut logos) = (Vec::new(), Vec::new());
        for row in rows {
            let kind = if row.logo { &mut logos } else { &mut backdrops };
            kind.push((row.plugin, row.path));
        }
        Ok((backdrops, logos))
    }

    /// Each Poster, a title's, its Season's or its Show's, a Match in the Library `library`
    /// stores, with whose it is.
    pub(crate) async fn posters(&self, library: i64) -> Result<Vec<TitlePoster>, sqlx::Error> {
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
            .map(|row| TitlePoster {
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
