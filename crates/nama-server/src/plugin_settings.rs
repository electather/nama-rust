//! Whether each Plugin is enabled, and its settings, kept in the database. They come from the
//! config's `[plugins.*]` tables once, on the first start; from then on the database is the only
//! store.

use std::collections::BTreeMap;

use nama_plugin_api::settings::SettingValue;

use crate::{config::PluginConfig, db::Database};

/// The `schema_meta` key whose presence records that the config was imported.
const IMPORTED_KEY: &str = "plugin_settings_imported";

/// Every stored Plugin's settings, by Plugin id, or `None` until [`import`] has run.
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

/// Stores `plugins`, read from the config's `[plugins.*]` tables, as each Plugin's settings, and
/// records the import so [`stored`] returns them from now on.
///
/// # Errors
///
/// If the database can't be written, or the import already ran.
pub async fn import(
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

/// Stores `settings` as the Plugin `id`'s, replacing any stored, and leaves whether it is enabled
/// as it was: enabled for a Plugin with nothing stored yet.
///
/// # Errors
///
/// If the database can't be written.
pub async fn save_settings(
    db: &Database,
    id: &str,
    settings: &BTreeMap<String, SettingValue>,
) -> Result<(), sqlx::Error> {
    let plugin_id = id.to_owned();
    let settings =
        serde_json::to_string(settings).map_err(|err| sqlx::Error::Encode(err.into()))?;
    db.write(move |tx| {
        Box::pin(async move {
            sqlx::query!(
                "INSERT INTO plugin_settings (plugin_id, enabled, settings) VALUES (?, 1, ?)
                 ON CONFLICT (plugin_id) DO UPDATE SET settings = excluded.settings",
                plugin_id,
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
///
/// # Errors
///
/// If the database can't be written.
pub async fn set_enabled(db: &Database, plugin_id: &str, enabled: bool) -> Result<(), sqlx::Error> {
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
