//! Personal settings (ADR 0046): what each User sets for themself in a Plugin whose Manifest
//! declares them, such as their ntfy topic. The Kernel keeps them for the Plugin, since Plugins
//! have no storage, and hands a Plugin the values of the User a call is for.
//!
//! This module stores what it is given: Plugin settings checks values against the Manifest
//! before it saves them (`src/plugin_settings/personal.rs`). Each value is stored as the text a
//! Plugin is handed ([`text`], read back with [`value`]). Values are secrets in logs, and a secret
//! one never goes back to a client.

use std::collections::BTreeMap;

use nama_plugin_api::{
    PluginId,
    settings::{SettingKind, SettingValue},
};
use sqlx::SqliteConnection;

use crate::{
    db::Database,
    users::{Permissions, User},
};

/// `user`'s Personal settings for `plugin`: each key's value and whether it is secret.
pub(crate) async fn get(
    db: &Database,
    user: i64,
    plugin: &PluginId,
) -> Result<BTreeMap<String, (String, bool)>, sqlx::Error> {
    let plugin = plugin.as_str();
    let rows = sqlx::query!(
        r#"SELECT key, value, secret AS "secret: bool" FROM personal_settings
           WHERE user_id = ? AND plugin_id = ?"#,
        user,
        plugin
    )
    .fetch_all(db.reader())
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| (row.key, (row.value, row.secret)))
        .collect())
}

/// Replaces all of `user`'s Personal settings for `plugin` with `values`, each a key, its value and
/// whether it is secret, in one write.
pub(crate) async fn replace(
    db: &Database,
    user: i64,
    plugin: &PluginId,
    values: Vec<(String, String, bool)>,
) -> Result<(), sqlx::Error> {
    let plugin = plugin.as_str().to_owned();
    db.write(move |tx| {
        Box::pin(async move {
            sqlx::query!(
                "DELETE FROM personal_settings WHERE user_id = ? AND plugin_id = ?",
                user,
                plugin
            )
            .execute(&mut **tx)
            .await?;
            for (key, value, secret) in values {
                sqlx::query!(
                    "INSERT INTO personal_settings (user_id, plugin_id, key, value, secret)
                     VALUES (?, ?, ?, ?, ?)",
                    user,
                    plugin,
                    key,
                    value,
                    secret
                )
                .execute(&mut **tx)
                .await?;
            }
            Ok(())
        })
    })
    .await
}

/// Deletes all of `user`'s Personal settings for `plugin`.
pub(crate) async fn delete(db: &Database, user: i64, plugin: &PluginId) -> Result<(), sqlx::Error> {
    replace(db, user, plugin, Vec::new()).await
}

/// Deletes every User's Personal settings for `plugin`.
pub(crate) async fn delete_plugin(
    conn: &mut SqliteConnection,
    plugin: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query!("DELETE FROM personal_settings WHERE plugin_id = ?", plugin)
        .execute(conn)
        .await?;
    Ok(())
}

/// Each User with any Personal setting for `plugin`, with their values by key and, apart, the
/// values that are secret, to blank from what a failed call says. Whom to tell is the caller's.
pub(crate) async fn recipients(
    db: &Database,
    plugin: &PluginId,
) -> Result<Vec<(User, BTreeMap<String, String>, Vec<String>)>, sqlx::Error> {
    let plugin = plugin.as_str();
    let rows = sqlx::query!(
        r#"SELECT users.id AS "id!", users.display_username AS name,
                  users.permissions AS "permissions: Permissions", users.created_at,
                  personal_settings.key, personal_settings.value,
                  personal_settings.secret AS "secret: bool"
           FROM personal_settings JOIN users ON users.id = personal_settings.user_id
           WHERE personal_settings.plugin_id = ?
           ORDER BY users.id"#,
        plugin
    )
    .fetch_all(db.reader())
    .await?;
    let mut recipients: Vec<(User, BTreeMap<String, String>, Vec<String>)> = Vec::new();
    for row in rows {
        if recipients.last().is_none_or(|(user, ..)| user.id != row.id) {
            let user = User {
                id: row.id,
                name: row.name,
                permissions: row.permissions,
                created_at: row.created_at,
            };
            recipients.push((user, BTreeMap::new(), Vec::new()));
        }
        if let Some((_, values, secrets)) = recipients.last_mut() {
            if row.secret {
                secrets.push(row.value.clone());
            }
            values.insert(row.key, row.value);
        }
    }
    Ok(recipients)
}

/// A User's Personal `settings`, each value by key with whether it is secret, as the values by
/// key a Plugin is asked with, and the secret values to blank from its failure.
pub(crate) fn split(
    settings: BTreeMap<String, (String, bool)>,
) -> (Vec<(String, String)>, Vec<String>) {
    let secrets = (settings.values())
        .filter(|(_, secret)| *secret)
        .map(|(value, _)| value.clone())
        .collect();
    let values = (settings.into_iter())
        .map(|(key, (value, _))| (key, value))
        .collect();
    (values, secrets)
}

/// `value` as it is stored, the text a Plugin gets in a `UserRecipient`: a bool as `true` or
/// `false`, a number in decimal.
pub(crate) fn text(value: SettingValue) -> String {
    match value {
        SettingValue::Text(text) => text,
        SettingValue::Bool(on) => on.to_string(),
        SettingValue::Integer(number) => number.to_string(),
        // Never stored: the Kernel refuses a Manifest whose Personal settings name Libraries or
        // Choices.
        SettingValue::Libraries(_) | SettingValue::Texts(_) => String::new(),
    }
}

/// Stored `text` as a value of `kind`, the inverse of [`text`], or `None` if it doesn't read as
/// one.
pub(crate) fn value(kind: SettingKind, text: String) -> Option<SettingValue> {
    match kind {
        SettingKind::Bool => text.parse().ok().map(SettingValue::Bool),
        SettingKind::Integer => text.parse().ok().map(SettingValue::Integer),
        SettingKind::Text | SettingKind::Secret | SettingKind::Choice | SettingKind::Url => {
            Some(SettingValue::Text(text))
        }
        // Never personal: the Kernel refuses a Manifest whose Personal settings name Libraries or
        // Choices.
        SettingKind::Libraries | SettingKind::Choices => None,
    }
}

#[cfg(test)]
#[path = "personal_settings_tests.rs"]
mod tests;
