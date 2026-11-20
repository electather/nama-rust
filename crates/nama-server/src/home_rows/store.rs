//! What `home_rows` keeps in SQLite and the small helpers around it.

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use nama_plugin_api::{PluginId, home_row::HomeRowMethod, metadata::ExternalId};
use serde::{Deserialize, Serialize};
use sqlx::SqliteConnection;

use super::{Kind, OnServer, Row, Title};
use crate::library::{FoundMovie, FoundShow, LibraryMovie, LibraryShow};

/// What a row stores as JSON, `json`, or none if it doesn't read.
pub(super) fn from_json<T: for<'de> Deserialize<'de>>(json: &str) -> Vec<T> {
    serde_json::from_str(json).unwrap_or_default()
}

/// Adds `title` to `found` under each of its ids `wanted` names that an older Library's title
/// didn't take.
pub(super) fn add_found(
    found: &mut HashMap<Title, OnServer>,
    kind: Kind,
    wanted: &[ExternalId],
    title: &OnServer,
) {
    let ids = match title {
        OnServer::Movie(movie) => &movie.ids,
        OnServer::Show(show) => &show.ids,
    };
    for id in ids.iter().filter(|id| wanted.contains(id)) {
        let key = Title(kind, id.to_string());
        found.entry(key).or_insert_with(|| title.clone());
    }
}

/// Refreshes the Plugin `plugin`'s rows for `user`, or its shared rows for `None`, once a User
/// they're for opens home after `stale_at`.
pub(super) async fn set_stale_at(
    conn: &mut SqliteConnection,
    plugin: &str,
    user: Option<i64>,
    stale_at: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO home_row_refreshes (plugin_id, user_id, stale_at) VALUES (?, ?, ?)
         ON CONFLICT (plugin_id, coalesce(user_id, 0)) DO UPDATE SET stale_at = excluded.stale_at",
        plugin,
        user,
        stale_at
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// Deletes, with when they were refreshed, each Plugin's rows that `gone` names by the method
/// that gave them, every User's for [`HomeRowMethod::User`].
pub(super) async fn delete_rows(
    conn: &mut SqliteConnection,
    gone: &HashSet<(PluginId, HomeRowMethod)>,
) -> Result<(), sqlx::Error> {
    for (plugin, method) in gone {
        let plugin = plugin.as_str();
        let users = *method == HomeRowMethod::User;
        sqlx::query!(
            "DELETE FROM home_rows WHERE plugin_id = ? AND (user_id IS NOT NULL) = ?",
            plugin,
            users
        )
        .execute(&mut *conn)
        .await?;
        sqlx::query!(
            "DELETE FROM home_row_refreshes WHERE plugin_id = ? AND (user_id IS NOT NULL) = ?",
            plugin,
            users
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

/// [`HomeRows::replace`]'s write: replaces the Plugin `plugin`'s rows for `user`, or its shared
/// rows for `None`, with `rows`, stale at `stale_at`, only while their put-off is stored.
pub(super) async fn replace_if_marked(
    conn: &mut SqliteConnection,
    plugin: &str,
    user: Option<i64>,
    rows: Vec<Row>,
    stale_at: i64,
) -> Result<(), sqlx::Error> {
    let marked = sqlx::query!(
        "UPDATE home_row_refreshes SET stale_at = ? WHERE plugin_id = ? AND user_id IS ?",
        stale_at,
        plugin,
        user
    )
    .execute(&mut *conn)
    .await?;
    if marked.rows_affected() == 0 {
        return Ok(());
    }
    sqlx::query!(
        "DELETE FROM home_rows WHERE plugin_id = ? AND user_id IS ?",
        plugin,
        user
    )
    .execute(&mut *conn)
    .await?;
    insert_rows(conn, plugin, user, rows).await
}

/// Stores `rows` as the Plugin `plugin`'s for `user`, or its shared rows for `None`, in their
/// order.
pub(super) async fn insert_rows(
    conn: &mut SqliteConnection,
    plugin: &str,
    user: Option<i64>,
    rows: Vec<Row>,
) -> Result<(), sqlx::Error> {
    for (row, position) in rows.into_iter().zip(0_i64..) {
        let titles = to_json(&row.titles)?;
        let missing = to_json(&row.missing)?;
        sqlx::query!(
            "INSERT INTO home_rows (plugin_id, user_id, position, key, heading, titles, missing)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            plugin,
            user,
            position,
            row.key,
            row.heading,
            titles,
            missing
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

pub(super) fn to_json(value: &impl Serialize) -> Result<String, sqlx::Error> {
    serde_json::to_string(value).map_err(|err| sqlx::Error::Encode(err.into()))
}

pub(super) fn found_movie(library: i64, movie: &LibraryMovie) -> OnServer {
    OnServer::Movie(FoundMovie {
        library,
        id: movie.id.clone(),
        ids: movie.ids.clone(),
        title: movie.title.clone(),
        release_date: movie.release_date.clone(),
        poster: movie.poster.clone(),
    })
}

pub(super) fn found_show(library: i64, show: &LibraryShow) -> OnServer {
    OnServer::Show(FoundShow {
        library,
        id: show.id.clone(),
        ids: show.ids.clone(),
        title: show.title.clone(),
        poster: show.poster().cloned(),
    })
}

/// A Plugin and whose of its rows: one User's, or its shared rows for `None`.
pub(super) type Scope = (PluginId, Option<i64>);

/// A scope's place in [`HomeRows::refreshing`], given up when dropped, even by a refresh
/// cancelled as its Plugin is disabled.
pub(super) struct Refreshing {
    set: Arc<Mutex<HashSet<Scope>>>,
    scope: Scope,
}

impl Refreshing {
    /// Takes `scope`'s place in `set`, or `None` if a refresh holds it.
    pub(super) fn start(set: &Arc<Mutex<HashSet<Scope>>>, scope: Scope) -> Option<Self> {
        let taken = lock(set).insert(scope.clone());
        taken.then(|| Self {
            set: Arc::clone(set),
            scope,
        })
    }
}

impl Drop for Refreshing {
    fn drop(&mut self) {
        lock(&self.set).remove(&self.scope);
    }
}

// No code that could panic runs while one is held, so a poisoned lock's value is still whole.
pub(super) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
