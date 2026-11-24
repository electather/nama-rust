//! Watch sync's side of Watch State: telling each Watch sync Plugin of a User's changes
//! (scrobble), the form changes take to and from those Plugins, and the cursor each Plugin's next
//! import of a User's Watch State continues from.

use std::sync::Arc;

use buffa::EnumValue;
use nama_plugin_api::{
    PluginError, PluginId,
    contract::{ScrobbleAsked, SyncedWatchState, TitleRefKind},
    metadata::ExternalId,
    points::{WatchStateChange, WatchStateChanges},
    watch_sync::WatchSync,
};
use sqlx::SqliteConnection;
use tracing::warn;

use super::title_source::title_id;
use crate::{
    db::Database,
    plugins::{ExtensionPoints, without_secrets},
    users::{personal_settings_of, split_personal_settings},
};

/// Tells each enabled Watch sync Plugin of `told`, for a User who set any of its Personal
/// settings, in its scope so disabling it stops the call, without waiting. A failure is logged
/// and shown on the Plugins page, and not asked again. Must be called within a Tokio runtime.
pub(super) fn scrobble(db: &Arc<Database>, points: &ExtensionPoints, told: &WatchStateChanges) {
    for plugin in points.providers::<dyn WatchSync>() {
        let asked = ScrobbleAsked {
            user_id: told.user_id,
            changes: told.changes.iter().map(synced).collect(),
            ..ScrobbleAsked::default()
        };
        let telling = scrobble_to(Arc::clone(db), points.clone(), plugin.clone(), asked);
        points.spawn_in(&plugin, telling);
    }
}

/// Tells the Plugin `plugin` of `asked`'s changes, with the User's Personal settings for it.
async fn scrobble_to(
    db: Arc<Database>,
    points: ExtensionPoints,
    plugin: PluginId,
    asked: ScrobbleAsked,
) {
    let user = asked.user_id;
    let settings = match personal_settings_of(&db, user, &plugin).await {
        Ok(settings) if settings.is_empty() => return,
        Ok(settings) => settings,
        Err(err) => {
            warn!(plugin = %plugin, user, "could not read Personal settings to scrobble: {err}");
            return;
        }
    };
    let (personal_settings, secrets) = split_personal_settings(settings);
    let asked = ScrobbleAsked {
        personal_settings: personal_settings.into_iter().collect(),
        ..asked
    };
    let answer = points
        .ask(&plugin, |sync: Arc<dyn WatchSync>| {
            let (asked, secrets) = (&asked, &secrets);
            async move {
                (sync.scrobble(asked).await)
                    .map_err(|err| PluginError::from(without_secrets(err.to_string(), secrets)))
            }
        })
        .await;
    if let Some(Err(failure)) = answer.map(|answer| answer.result) {
        let error = failure.message;
        warn!(plugin = %plugin, user, error, "scrobbling a User's Watch State failed");
    }
}

/// `change` as a Watch sync Plugin is told of it.
fn synced(change: &WatchStateChange) -> SyncedWatchState {
    let kind = if change.episode {
        TitleRefKind::TITLE_REF_KIND_EPISODE
    } else {
        TitleRefKind::TITLE_REF_KIND_MOVIE
    };
    SyncedWatchState {
        kind: kind.into(),
        // Every id was stored from a Match, so each parses.
        ids: (change.ids.iter())
            .filter_map(|id| id.parse::<ExternalId>().ok())
            .map(|id| title_id(&id))
            .collect(),
        played: change.played,
        position_ms: change.position_ms,
        duration_ms: change.duration_ms,
        changed_at_ms: change.changed_at_ms,
        ..SyncedWatchState::default()
    }
}

/// `state`, which a Watch sync Plugin imported, as a change to take, or `None` if it names
/// neither a Movie nor an Episode or no well-formed id.
pub(crate) fn imported(state: &SyncedWatchState) -> Option<WatchStateChange> {
    let episode = match state.kind {
        EnumValue::Known(TitleRefKind::TITLE_REF_KIND_MOVIE) => false,
        EnumValue::Known(TitleRefKind::TITLE_REF_KIND_EPISODE) => true,
        EnumValue::Known(TitleRefKind::TITLE_REF_KIND_UNSPECIFIED) | EnumValue::Unknown(_) => {
            return None;
        }
    };
    let ids: Vec<String> = (state.ids.iter())
        .filter_map(|id| ExternalId::new(&id.namespace, id.value.clone()).ok())
        .map(|id| id.to_string())
        .collect();
    (!ids.is_empty()).then_some(WatchStateChange {
        episode,
        ids,
        played: state.played,
        position_ms: state.position_ms,
        duration_ms: state.duration_ms,
        changed_at_ms: state.changed_at_ms,
    })
}

/// Where the Plugin `plugin`'s next import of the User `user`'s Watch State continues from: the
/// cursor its last import returned, or empty before the first.
pub(crate) async fn import_cursor(
    db: &Database,
    user: i64,
    plugin: &PluginId,
) -> Result<String, sqlx::Error> {
    let plugin = plugin.as_str();
    let cursor = sqlx::query_scalar!(
        "SELECT cursor FROM watch_sync_cursors WHERE user_id = ? AND plugin_id = ?",
        user,
        plugin
    )
    .fetch_optional(db.reader())
    .await?;
    Ok(cursor.unwrap_or_default())
}

/// Keeps `cursor` as where the Plugin `plugin`'s next import of the User `user`'s Watch State
/// continues from.
pub(crate) async fn keep_import_cursor(
    db: &Database,
    user: i64,
    plugin: &PluginId,
    cursor: String,
) -> Result<(), sqlx::Error> {
    let plugin = plugin.as_str().to_owned();
    db.write(move |tx| {
        Box::pin(async move {
            sqlx::query!(
                "INSERT INTO watch_sync_cursors (user_id, plugin_id, cursor) VALUES (?, ?, ?)
                 ON CONFLICT (user_id, plugin_id) DO UPDATE SET cursor = excluded.cursor",
                user,
                plugin,
                cursor
            )
            .execute(&mut **tx)
            .await?;
            Ok(())
        })
    })
    .await
}

/// Deletes every User's import cursor of the Plugin `plugin`.
pub(crate) async fn delete_plugins_import_cursors(
    conn: &mut SqliteConnection,
    plugin: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query!("DELETE FROM watch_sync_cursors WHERE plugin_id = ?", plugin)
        .execute(conn)
        .await?;
    Ok(())
}

#[cfg(test)]
#[path = "watch_sync_tests.rs"]
mod tests;
