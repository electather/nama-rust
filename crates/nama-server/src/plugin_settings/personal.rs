//! Checking a User's Personal settings with the Plugin's own check before they are saved.

use std::collections::BTreeMap;

use nama_plugin_api::{PluginId, settings::Settings};

use super::{ChangeError, PluginSettings, StoredPlugin};

impl PluginSettings {
    /// Refuses `personal`, one User's Personal settings for the Plugin `id` as they would be
    /// stored, if the Plugin's check of them fails with its stored settings. A Plugin without one
    /// passes. Saves nothing of the Plugin's, so it doesn't take the change lock.
    ///
    /// # Errors
    ///
    /// If no Plugin has the id or the Server refused it, the check fails, or the stored settings
    /// can't be read.
    pub(crate) async fn check_personal(
        &self,
        id: &PluginId,
        personal: &BTreeMap<String, String>,
    ) -> Result<(), ChangeError> {
        let (_, StoredPlugin { stored, .. }) = self.listed(id).await?;
        self.registry
            .check_personal_settings(id, Settings::new(stored.settings), personal)
            .await
            .map_err(|error| ChangeError::Check {
                plugin: id.clone(),
                error,
                setting: None,
            })
    }
}
