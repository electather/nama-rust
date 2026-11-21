//! The Plugin settings as the database stores them, in `plugin_settings` and `schema_meta`, and
//! what a Plugin holds when nothing is stored.

use std::collections::{BTreeMap, HashSet};

use nama_plugin_api::{automation::Permission, settings::SettingValue};
use nama_proto::nama::v1::PluginKind;

use super::{Grants, StoredPlugin};
use crate::{
    config::PluginConfig,
    db::Database,
    plugins::{Listed, Registry},
};

/// The `schema_meta` key whose presence records that the config was imported.
const IMPORTED_KEY: &str = "plugin_settings_imported";

/// Every stored Plugin's settings, by Plugin id, or `None` until the first start imported the
/// config's tables.
///
/// # Errors
///
/// If the database can't be read, or holds settings that aren't a JSON object of setting values.
pub async fn stored(db: &Database) -> Result<Option<BTreeMap<String, PluginConfig>>, sqlx::Error> {
    let imported = sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM schema_meta WHERE key = ?) AS "imported!: bool""#,
        IMPORTED_KEY
    )
    .fetch_one(db.reader())
    .await?;
    if !imported {
        return Ok(None);
    }
    let rows = sqlx::query!(
        r#"SELECT plugin_id, enabled AS "enabled: bool", settings FROM plugin_settings"#
    )
    .fetch_all(db.reader())
    .await?;
    rows.into_iter()
        .map(|row| {
            let settings = serde_json::from_str(&row.settings)
                .map_err(|err| sqlx::Error::Decode(err.into()))?;
            let config = PluginConfig {
                enabled: row.enabled,
                settings,
            };
            Ok((row.plugin_id, config))
        })
        .collect::<Result<_, _>>()
        .map(Some)
}

/// The Plugin Permissions an Admin granted each stored Plugin, by Plugin id (ADR 0049).
///
/// # Errors
///
/// If the database can't be read, or holds grants that aren't a JSON array of Permission names.
pub async fn grants(db: &Database) -> Result<BTreeMap<String, HashSet<Permission>>, sqlx::Error> {
    let rows = sqlx::query!("SELECT plugin_id, granted_permissions FROM plugin_settings")
        .fetch_all(db.reader())
        .await?;
    rows.into_iter()
        .map(|row| {
            let granted = serde_json::from_str(&row.granted_permissions)
                .map_err(|err| sqlx::Error::Decode(err.into()))?;
            Ok((row.plugin_id, granted))
        })
        .collect()
}

/// Stores `plugins`, read from the config's `[plugins.*]` tables, as each Plugin's settings, and
/// records the import so [`stored`] returns them from now on. Fails if the import already ran.
pub(super) async fn import(
    db: &Database,
    plugins: &BTreeMap<String, PluginConfig>,
) -> Result<(), sqlx::Error> {
    let rows = plugins
        .iter()
        .map(|(id, config)| {
            let settings = serde_json::to_string(&config.settings)?;
            Ok((id.clone(), config.enabled, settings))
        })
        .collect::<Result<Vec<_>, serde_json::Error>>()
        .map_err(|err| sqlx::Error::Encode(err.into()))?;
    db.write(move |tx| {
        Box::pin(async move {
            for (plugin_id, enabled, settings) in rows {
                sqlx::query!(
                    "INSERT INTO plugin_settings (plugin_id, enabled, settings) VALUES (?, ?, ?)",
                    plugin_id,
                    enabled,
                    settings
                )
                .execute(&mut **tx)
                .await?;
            }
            sqlx::query!(
                "INSERT INTO schema_meta (key, value) VALUES (?, 'yes')",
                IMPORTED_KEY
            )
            .execute(&mut **tx)
            .await?;
            Ok(())
        })
    })
    .await
}

/// Stores whether the Plugin `id` is enabled and its `settings`, replacing any stored.
pub(super) async fn save(
    db: &Database,
    id: &str,
    enabled: bool,
    settings: &BTreeMap<String, SettingValue>,
) -> Result<(), sqlx::Error> {
    let plugin_id = id.to_owned();
    let settings =
        serde_json::to_string(settings).map_err(|err| sqlx::Error::Encode(err.into()))?;
    db.write(move |tx| {
        Box::pin(async move {
            sqlx::query!(
                "INSERT INTO plugin_settings (plugin_id, enabled, settings) VALUES (?, ?, ?)
                 ON CONFLICT (plugin_id) DO UPDATE
                 SET enabled = excluded.enabled, settings = excluded.settings",
                plugin_id,
                enabled,
                settings
            )
            .execute(&mut **tx)
            .await?;
            Ok(())
        })
    })
    .await
}

/// Saves whether the Plugin `plugin_id` is enabled, keeping its stored settings.
pub(super) async fn set_enabled(
    db: &Database,
    plugin_id: &str,
    enabled: bool,
) -> Result<(), sqlx::Error> {
    let plugin_id = plugin_id.to_owned();
    db.write(move |tx| {
        Box::pin(async move {
            sqlx::query!(
                "INSERT INTO plugin_settings (plugin_id, enabled, settings) VALUES (?, ?, '{}')
                 ON CONFLICT (plugin_id) DO UPDATE SET enabled = excluded.enabled",
                plugin_id,
                enabled
            )
            .execute(&mut **tx)
            .await?;
            Ok(())
        })
    })
    .await
}

/// Saves the Plugin `plugin_id` as enabled with `granted` as its Plugin Permissions, replacing
/// those granted before and keeping its stored settings.
pub(super) async fn set_enabled_granting(
    db: &Database,
    plugin_id: &str,
    granted: &HashSet<Permission>,
) -> Result<(), sqlx::Error> {
    let plugin_id = plugin_id.to_owned();
    let names = granted_names(granted)?;
    db.write(move |tx| {
        Box::pin(async move {
            sqlx::query!(
                "INSERT INTO plugin_settings (plugin_id, enabled, settings, granted_permissions)
                 VALUES (?, TRUE, '{}', ?)
                 ON CONFLICT (plugin_id) DO UPDATE
                 SET enabled = excluded.enabled, granted_permissions = excluded.granted_permissions",
                plugin_id,
                names
            )
            .execute(&mut **tx)
            .await?;
            Ok(())
        })
    })
    .await
}

/// The Plugin Permissions each Built-in Plugin `registry` knows requests, which it always holds
/// (ADR 0079), of those that request any or that `grants` stores some for.
pub(super) fn builtin_grants(registry: &Registry, grants: &Grants) -> Grants {
    (registry.plugins().into_iter())
        .filter(|plugin| plugin.kind == PluginKind::PLUGIN_KIND_BUILT_IN)
        .filter_map(|plugin| {
            let requested: HashSet<_> = plugin.manifest.ok()?.permissions.into_iter().collect();
            let has_row = grants.contains_key(plugin.id.as_str());
            (has_row || !requested.is_empty()).then(|| (plugin.id.to_string(), requested))
        })
        .collect()
}

/// Saves each Plugin `builtin` names with the Plugin Permissions it holds, replacing those stored
/// and keeping whether it is enabled and its settings if they are stored, or else as a Built-in
/// with nothing stored starts.
pub(super) async fn save_builtin_grants(
    db: &Database,
    builtin: &Grants,
) -> Result<(), sqlx::Error> {
    let rows = (builtin.iter())
        .map(|(plugin_id, granted)| Ok((plugin_id.clone(), granted_names(granted)?)))
        .collect::<Result<Vec<_>, sqlx::Error>>()?;
    if rows.is_empty() {
        return Ok(());
    }
    db.write(move |tx| {
        Box::pin(async move {
            for (plugin_id, names) in rows {
                sqlx::query!(
                    "INSERT INTO plugin_settings (plugin_id, enabled, settings, granted_permissions)
                     VALUES (?, TRUE, '{}', ?)
                     ON CONFLICT (plugin_id) DO UPDATE
                     SET granted_permissions = excluded.granted_permissions",
                    plugin_id,
                    names
                )
                .execute(&mut **tx)
                .await?;
            }
            Ok(())
        })
    })
    .await
}

/// `granted` as `plugin_settings.granted_permissions` stores them: a JSON array of their names.
fn granted_names(granted: &HashSet<Permission>) -> Result<String, sqlx::Error> {
    let mut names: Vec<_> = granted.iter().collect();
    // Sorted so the same grants always read the same in the database.
    names.sort_by_key(|permission| **permission as i32);
    serde_json::to_string(&names).map_err(|err| sqlx::Error::Encode(err.into()))
}

/// `plugin` with what `stored` holds for it, taken out of `stored`.
pub(super) fn with_stored(
    plugin: Listed,
    (settings, grants): &mut (BTreeMap<String, PluginConfig>, Grants),
) -> StoredPlugin {
    let stored = settings
        .remove(plugin.id.as_str())
        .unwrap_or_else(|| never_stored(plugin.kind));
    let granted = grants.remove(plugin.id.as_str()).unwrap_or_default();
    StoredPlugin {
        plugin,
        stored,
        granted,
    }
}

/// What a Plugin of `kind` with nothing stored starts as: no settings, and enabled only if it is
/// Built-in. An Installed Plugin waits for an admin to enable it.
pub(super) fn never_stored(kind: PluginKind) -> PluginConfig {
    PluginConfig {
        enabled: kind == PluginKind::PLUGIN_KIND_BUILT_IN,
        settings: BTreeMap::new(),
    }
}
