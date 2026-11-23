//! A title's Roles, stored once per title in `title_roles` rather than on each of its Matches
//! (ADR 0081), under the key `title_details` uses.

use std::{cmp::Reverse, collections::HashSet};

use nama_plugin_api::metadata::{ExternalId, ExternalIds};
use sqlx::SqliteConnection;

use super::{
    Contents, Metadata, Movie, Show,
    details::title_key,
    search::{has_word_starts, match_bands},
};
use crate::library::title_source::{Role, Roles, TitleKind};

/// One of a Person's Roles and the title it is in, as [`Contents::person_roles`] reads it.
#[derive(Debug)]
pub(crate) struct PersonRole {
    pub(crate) kind: TitleKind,
    /// The title's id, as the per-title stores key it.
    pub(crate) title: ExternalId,
    /// The Plugin whose title source gave it, which its headshot is from.
    pub(crate) plugin: String,
    pub(crate) crew: bool,
    pub(crate) role: Role,
}

/// The titles a write of Matches has stored Roles for, each as `title_key` keys it, owned.
pub(super) type Stored = HashSet<(String, &'static str, ExternalId)>;

/// Stores the Roles that `matched` gives its title, a Movie or an Episode's Show, in place of what
/// the same title source gave it before, unless `stored` holds the title already. Every Episode
/// of a Show carries the same Roles, so one write of Matches stores them once.
pub(super) async fn store_once(
    conn: &mut SqliteConnection,
    matched: &Metadata,
    stored: &mut Stored,
) -> Result<(), sqlx::Error> {
    let Some((plugin, kind, id)) = title_key(matched) else {
        return Ok(());
    };
    if !stored.insert((plugin.to_owned(), kind, id.clone())) {
        return Ok(());
    }
    let (namespace, value) = (id.namespace(), id.value());
    sqlx::query!(
        "DELETE FROM title_roles WHERE plugin = ? AND kind = ? AND namespace = ? AND value = ?",
        plugin,
        kind,
        namespace,
        value,
    )
    .execute(&mut *conn)
    .await?;
    let listed = (matched
        .roles
        .cast
        .iter()
        .enumerate()
        .map(|(billed, role)| (false, billed, role)))
    .chain((matched.roles.crew.iter().enumerate()).map(|(billed, role)| (true, billed, role)));
    for (crew, billed, role) in listed {
        let billed = i64::try_from(billed).unwrap_or(i64::MAX);
        let (person_namespace, person_value) = (role.person.namespace(), role.person.value());
        sqlx::query!(
            "INSERT INTO title_roles (plugin, kind, namespace, value, crew, billed,
                                      person_namespace, person_value, name, part, headshot)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            plugin,
            kind,
            namespace,
            value,
            crew,
            billed,
            person_namespace,
            person_value,
            role.name,
            role.part,
            role.headshot,
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

/// Deletes the Roles `plugin`'s title source gave.
pub(super) async fn forget_plugin(
    conn: &mut SqliteConnection,
    plugin: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query!("DELETE FROM title_roles WHERE plugin = ?", plugin)
        .execute(conn)
        .await?;
    Ok(())
}

impl Contents {
    /// The Roles `movie`'s title source gave it, each list in billing order.
    pub(crate) async fn movie_roles<V>(&self, movie: &Movie<V>) -> Result<Roles, sqlx::Error> {
        self.roles(&movie.plugin, "movie", &movie.ids).await
    }

    /// As [`Contents::movie_roles`], for `show`.
    pub(crate) async fn show_roles<V>(&self, show: &Show<V>) -> Result<Roles, sqlx::Error> {
        let Some(plugin) = show.plugin() else {
            return Ok(Roles::default());
        };
        self.roles(plugin, "show", &show.ids).await
    }

    pub(super) async fn roles(
        &self,
        plugin: &str,
        kind: &str,
        ids: &ExternalIds,
    ) -> Result<Roles, sqlx::Error> {
        let mut roles = Roles::default();
        let Some(id) = ids.iter().next() else {
            return Ok(roles);
        };
        let (namespace, value) = (id.namespace(), id.value());
        let rows = sqlx::query!(
            r#"SELECT crew AS "crew: bool", person_namespace, person_value, name, part, headshot
               FROM title_roles
               WHERE plugin = ? AND kind = ? AND namespace = ? AND value = ?
               ORDER BY crew, billed"#,
            plugin,
            kind,
            namespace,
            value,
        )
        .fetch_all(self.db.reader())
        .await?;
        for row in rows {
            // Stored only once checked, so this skips nothing a title source gave.
            let Ok(person) = ExternalId::new(&row.person_namespace, row.person_value) else {
                continue;
            };
            let role = Role {
                person,
                name: row.name,
                part: row.part,
                headshot: row.headshot,
            };
            if row.crew {
                &mut roles.crew
            } else {
                &mut roles.cast
            }
            .push(role);
        }
        Ok(roles)
    }
}

impl Contents {
    /// Every Role of `person`, in any title, Cast before Crew. A Role outlives its title's
    /// Matches, so its title may be in no Library.
    pub(crate) async fn person_roles(
        &self,
        person: &ExternalId,
    ) -> Result<Vec<PersonRole>, sqlx::Error> {
        let (namespace, value) = (person.namespace(), person.value());
        let rows = sqlx::query!(
            r#"SELECT plugin, kind, namespace, value, crew AS "crew!: bool", name, part, headshot
               FROM title_roles
               WHERE person_namespace = ? AND person_value = ?
               ORDER BY crew, plugin, kind, namespace, value, billed"#,
            namespace,
            value,
        )
        .fetch_all(self.db.reader())
        .await?;
        Ok((rows.into_iter())
            // Stored only once checked, so this skips nothing a title source gave.
            .filter_map(|row| {
                Some(PersonRole {
                    kind: if row.kind == "show" {
                        TitleKind::Show
                    } else {
                        TitleKind::Movie
                    },
                    title: ExternalId::new(&row.namespace, row.value).ok()?,
                    plugin: row.plugin,
                    crew: row.crew,
                    role: Role {
                        person: person.clone(),
                        name: row.name,
                        part: row.part,
                        headshot: row.headshot,
                    },
                })
            })
            .collect())
    }

    /// The People with a Role whose names have a word starting with each word of `query`,
    /// ignoring case and accents, as a search finds titles: those whose names fit it best first,
    /// as [`match_bands`] grades a title, then those with the most Roles.
    pub(crate) async fn people_named(&self, query: &str) -> Result<Vec<ExternalId>, sqlx::Error> {
        // ponytail: reads every Person's name each search; index names as `title_search` indexes
        // titles if a large Server's searches get slow.
        let rows = sqlx::query!(
            r#"SELECT person_namespace AS "namespace!", person_value AS "value!",
                      MIN(name) AS "name!: String",
                      COUNT(*) AS "roles!: i64"
               FROM title_roles
               GROUP BY person_namespace, person_value"#
        )
        .fetch_all(self.db.reader())
        .await?;
        let mut named: Vec<_> = (rows.into_iter())
            .filter(|row| has_word_starts(query, &row.name))
            .collect();
        named.sort_by_cached_key(|row| (match_bands(query, &row.name, None), Reverse(row.roles)));
        Ok((named.into_iter())
            .filter_map(|row| ExternalId::new(&row.namespace, row.value).ok())
            .collect())
    }
}

impl<V> Show<V> {
    /// The Plugin that Matched it, which its Roles are keyed by and its headshots are from: its
    /// first Episode's, whose Match's metadata it has. `None` without Episodes.
    pub(crate) fn plugin(&self) -> Option<&str> {
        self.episodes()
            .next()
            .map(|episode| episode.plugin.as_str())
    }
}

#[cfg(test)]
#[path = "roles_tests.rs"]
mod tests;
