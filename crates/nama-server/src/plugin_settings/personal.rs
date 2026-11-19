//! Reading, saving and deleting one User's Personal settings for a Plugin (ADR 0046), with every
//! rule a save keeps. Saves nothing of the Plugin's, so none of it takes the change lock or
//! follows ADR 0028's order.

use std::collections::BTreeMap;

use nama_plugin_api::{
    Manifest, PluginId,
    settings::{SettingKind, SettingValue, Settings},
};

use super::{ChangeError, ListError, PluginSettings, StoredPlugin, sent::with_stored_secrets};
use crate::{
    plugins::Listed,
    users::{
        delete_personal_settings, personal_settings_of, personal_text, personal_value,
        replace_personal_settings,
    },
};

/// One User's Personal settings for a Plugin that is enabled and declares them.
#[derive(Debug)]
pub(crate) struct Personal {
    /// The Plugin, as the registry lists it.
    pub(crate) plugin: Listed,
    /// Its Manifest, which declares the Personal settings.
    pub(crate) manifest: Manifest,
    /// The User's values, each read as the kind the Manifest declares; one that no longer reads
    /// as its kind is left out. Secrets are among them, so never send these to a client as they
    /// are.
    pub(crate) values: BTreeMap<String, SettingValue>,
}

impl PluginSettings {
    /// `user`'s Personal settings for each enabled Plugin that declares any, sorted by Plugin id.
    ///
    /// # Errors
    ///
    /// If the Plugins or the values can't be read.
    pub(crate) async fn list_personal(&self, user: i64) -> Result<Vec<Personal>, ListError> {
        let mut listed = Vec::new();
        for StoredPlugin { plugin, .. } in self.list().await? {
            if let Some(manifest) = declaring(&plugin) {
                let values = self.personal_values(user, &plugin.id, &manifest).await?;
                listed.push(Personal {
                    plugin,
                    manifest,
                    values,
                });
            }
        }
        Ok(listed)
    }

    /// Replaces `user`'s Personal settings for the Plugin `id` with `sent`, and returns them as
    /// saved. Keys the Manifest doesn't declare fall out, and a secret left out keeps its stored
    /// value. Refused values leave the stored ones as they were.
    ///
    /// # Errors
    ///
    /// [`ChangeError::NoPersonalSettings`] unless the Plugin is enabled and declares Personal
    /// settings, [`ChangeError::InvalidSettings`] if the values don't fit them,
    /// [`ChangeError::Check`] if the Plugin's own check of them fails, with every secret blanked
    /// from its reason, or [`ChangeError::Database`].
    pub(crate) async fn save_personal(
        &self,
        user: i64,
        id: &PluginId,
        sent: BTreeMap<String, SettingValue>,
    ) -> Result<Personal, ChangeError> {
        let found = self.get(id).await?.and_then(|found| {
            let manifest = declaring(&found.plugin)?;
            Some((found, manifest))
        });
        let Some((StoredPlugin { plugin, stored, .. }, manifest)) = found else {
            return Err(ChangeError::NoPersonalSettings(id.clone()));
        };
        let schema = &manifest.personal_settings;
        let earlier = self.personal_values(user, id, &manifest).await?;
        let fitted = with_stored_secrets(schema, sent, &earlier);
        Settings::new(fitted.clone())
            .resolve(schema)
            .map_err(ChangeError::InvalidSettings)?;
        let rows: Vec<_> = fitted
            .into_iter()
            .map(|(key, value)| {
                let secret = schema
                    .fields
                    .iter()
                    .any(|field| field.key == key && field.kind == SettingKind::Secret);
                (key, personal_text(value), secret)
            })
            .collect();
        let personal = rows
            .iter()
            .map(|(key, text, _)| (key.clone(), text.clone()))
            .collect();
        self.registry
            .check_personal_settings(id, Settings::new(stored.settings), &personal)
            .await
            .map_err(|error| ChangeError::Check {
                plugin: id.clone(),
                error,
                setting: None,
            })?;
        replace_personal_settings(&self.db, user, id, rows).await?;
        let values = self.personal_values(user, id, &manifest).await?;
        Ok(Personal {
            plugin,
            manifest,
            values,
        })
    }

    /// Deletes all of `user`'s Personal settings for the Plugin `id`.
    ///
    /// # Errors
    ///
    /// If they can't be deleted.
    pub(crate) async fn delete_personal(
        &self,
        user: i64,
        id: &PluginId,
    ) -> Result<(), sqlx::Error> {
        delete_personal_settings(&self.db, user, id).await
    }

    /// `user`'s stored Personal settings for the Plugin `id`, each read back as the kind
    /// `manifest` declares it; one that no longer reads as its kind is left out.
    async fn personal_values(
        &self,
        user: i64,
        id: &PluginId,
        manifest: &Manifest,
    ) -> Result<BTreeMap<String, SettingValue>, sqlx::Error> {
        let mut stored = personal_settings_of(&self.db, user, id).await?;
        Ok(manifest
            .personal_settings
            .fields
            .iter()
            .filter_map(|field| {
                let (text, _) = stored.remove(&field.key)?;
                Some((field.key.clone(), personal_value(field.kind, text)?))
            })
            .collect())
    }
}

/// `plugin`'s Manifest if it is enabled and declares Personal settings.
fn declaring(plugin: &Listed) -> Option<Manifest> {
    let manifest = plugin.manifest.as_ref().ok()?;
    (plugin.enabled && !manifest.personal_settings.fields.is_empty()).then(|| manifest.clone())
}

#[cfg(test)]
#[path = "personal_tests.rs"]
mod tests;
