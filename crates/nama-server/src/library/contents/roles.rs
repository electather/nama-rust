//! A title's Roles, stored once per title in `title_roles` rather than on each of its Matches
//! (ADR 0081), under the key `title_scores` uses.

use std::collections::HashSet;

use nama_plugin_api::metadata::{ExternalId, ExternalIds};
use sqlx::SqliteConnection;

use super::{Contents, Metadata, Movie, Show, scores::title_key};
use crate::library::title_source::{Role, Roles};

/// A title as `title_key` keys it, owned.
type TitleKey = (String, &'static str, ExternalId);

/// Stores the Roles that `matched` gives its title, a Movie or an Episode's Show, in place of what
/// the same title source gave it before, unless `stored` holds the title already. Every Episode
/// of a Show carries the same Roles, so one write of Matches stores them once.
pub(super) async fn store_once(
    conn: &mut SqliteConnection,
    matched: &Metadata,
    stored: &mut HashSet<TitleKey>,
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

    async fn roles(
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
