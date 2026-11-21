//! The Movies or Shows of every Library sharing Persons or genres with a title, which a Similar
//! source narrows a Server down with (`list_titles_sharing`).

use nama_plugin_api::metadata::{ExternalId, ExternalIds};

use super::{Contents, ids_from_json, matched::genres};
use crate::library::title_source::{Roles, TitleKind};

/// A Movie or Show [`Contents::titles_sharing`] found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SharingTitle {
    pub(crate) ids: ExternalIds,
    pub(crate) genres: Vec<String>,
    pub(crate) roles: Roles,
}

impl Contents {
    /// Up to `limit` Movies or Shows of `kind` Matched in any Library, each once, sharing at least
    /// one of `people`, by the ids their Roles have, or of `genres`: those sharing the most Persons
    /// first, then the most genres.
    ///
    /// Each title is read from the stores ADR 0081 keeps once per title, under their key, and one
    /// of its Matches, so a Show is read once rather than per Episode.
    pub(crate) async fn titles_sharing(
        &self,
        kind: TitleKind,
        people: &[ExternalId],
        genres_asked: &[String],
        limit: u32,
    ) -> Result<Vec<SharingTitle>, sqlx::Error> {
        let kind = match kind {
            TitleKind::Movie => "movie",
            TitleKind::Show => "show",
        };
        let people: Vec<String> = people.iter().map(ExternalId::to_string).collect();
        let people = serde_json::Value::from(people).to_string();
        let genres_asked = serde_json::Value::from(genres_asked.to_vec()).to_string();
        // ponytail: reads every title of the kind's Roles and one Match each per call; index
        // `title_roles` by Person if a large Server's refresh shows it.
        let rows = sqlx::query!(
            r#"WITH asked AS (SELECT kind, people, genres,
                                     IIF(kind = 'movie', 'title', 'show') AS scope
                              FROM (SELECT ? AS kind, ? AS people, ? AS genres)),
               -- Each title once, by its per-title key, with one Item Matched to it through the
               -- Plugin that keyed it: a row outlives its Matches, and another source's Match
               -- isn't this one's title.
               titles AS (
                 SELECT details.plugin, details.namespace, details.value,
                        (SELECT item_matches.item_id
                         FROM item_match_ids
                         JOIN item_matches ON item_matches.item_id = item_match_ids.item_id
                         JOIN library_items ON library_items.id = item_matches.item_id
                         WHERE item_match_ids.scope = asked.scope
                           AND item_match_ids.namespace = details.namespace
                           AND item_match_ids.value = details.value
                           AND item_matches.state = 'matched'
                           AND item_matches.plugin = details.plugin
                           AND library_items.kind = IIF(asked.kind = 'movie', 'movie', 'episode')
                         LIMIT 1) AS item_id
                 FROM title_details AS details, asked
                 WHERE details.kind = asked.kind),
               sharing AS (
                 SELECT titles.plugin, titles.namespace, titles.value, item_matches.item_id,
                        item_matches.genres,
                        (SELECT COUNT(DISTINCT roles.person_namespace || '-' || roles.person_value)
                         FROM title_roles AS roles
                         WHERE roles.plugin = titles.plugin AND roles.kind = asked.kind
                           AND roles.namespace = titles.namespace AND roles.value = titles.value
                           AND roles.person_namespace || '-' || roles.person_value
                               IN (SELECT value FROM json_each(asked.people))) AS people,
                        (SELECT COUNT(*) FROM json_each(coalesce(item_matches.genres, '[]')) AS genre
                         WHERE genre.value IN (SELECT value FROM json_each(asked.genres)))
                            AS shared_genres
                 FROM titles
                 JOIN item_matches ON item_matches.item_id = titles.item_id, asked)
               SELECT sharing.plugin AS "plugin!", sharing.namespace AS "namespace!",
                      sharing.value AS "value!", sharing.genres,
                      (SELECT json_group_object(ids.namespace, ids.value) FROM item_match_ids AS ids
                       WHERE ids.item_id = sharing.item_id AND ids.scope = asked.scope) AS "ids: String"
               FROM sharing, asked
               WHERE sharing.people > 0 OR sharing.shared_genres > 0
               ORDER BY sharing.people DESC, sharing.shared_genres DESC,
                        sharing.plugin, sharing.namespace, sharing.value
               LIMIT ?"#,
            kind,
            people,
            genres_asked,
            limit,
        )
        .fetch_all(self.db.reader())
        .await?;
        let mut found = Vec::with_capacity(rows.len());
        for row in rows {
            // Stored only once checked, so this skips nothing a title source gave.
            let Ok(key) = ExternalId::new(&row.namespace, row.value) else {
                continue;
            };
            let roles = self
                .roles(&row.plugin, kind, &ExternalIds::from_iter([key]))
                .await?;
            found.push(SharingTitle {
                ids: ids_from_json(row.ids.as_deref()),
                genres: genres(row.genres.as_deref())?,
                roles,
            });
        }
        Ok(found)
    }
}

#[cfg(test)]
#[path = "sharing_tests.rs"]
mod tests;
