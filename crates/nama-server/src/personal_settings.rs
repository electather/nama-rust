//! Personal settings (ADR 0046): what each User sets for themself in a Plugin whose Manifest
//! declares them, such as their ntfy topic. The Kernel keeps them for the Plugin, since Plugins
//! have no storage, and hands a Plugin the values of the User a call is for.
//!
//! This module stores what it is given; checking values against the Manifest is its caller's.
//! Values are secrets in logs, and a secret one never goes back to a client.

use std::collections::BTreeMap;

use nama_plugin_api::PluginId;

use crate::{
    db::Database,
    users::{Permissions, User},
};

/// `user`'s Personal settings for `plugin`: each key's value and whether it is secret.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the client API that sets them comes in its own change"
    )
)]
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
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the client API that sets them comes in its own change"
    )
)]
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
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the client API that sets them comes in its own change"
    )
)]
pub(crate) async fn delete(db: &Database, user: i64, plugin: &PluginId) -> Result<(), sqlx::Error> {
    replace(db, user, plugin, Vec::new()).await
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

#[cfg(test)]
#[path = "personal_settings_tests.rs"]
mod tests;
