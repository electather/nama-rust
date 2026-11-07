//! Whether each Plugin is enabled, and its settings, kept in the database. They come from the
//! config's `[plugins.*]` tables once, on the first start after upgrading; from then on the
//! database is the only store.

use std::collections::BTreeMap;

use crate::{config::PluginConfig, db::Database};

/// The `schema_meta` key whose presence records that the config was imported.
const IMPORTED: &str = "plugin_settings_imported";

/// Every stored Plugin's settings, by Plugin id, or `None` until [`import`] has run.
///
/// # Errors
///
/// If the database can't be read, or holds settings that aren't a JSON object of setting values.
pub async fn stored(db: &Database) -> Result<Option<BTreeMap<String, PluginConfig>>, sqlx::Error> {
    let imported = sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM schema_meta WHERE key = ?) AS "imported!: bool""#,
        IMPORTED
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

/// Stores `plugins`, the config's `[plugins.*]` tables, as each Plugin's settings, and records the
/// import so [`stored`] returns them from now on.
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
                IMPORTED
            )
            .execute(&mut **tx)
            .await?;
            Ok(())
        })
    })
    .await
}
