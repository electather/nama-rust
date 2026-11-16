//! Putting a Plugin back as it ran before a change to its grants failed, so what runs agrees with
//! what is stored (ADR 0049).

use std::{
    collections::{BTreeMap, HashSet},
    fmt::Display,
};

use nama_plugin_api::{
    PluginId,
    automation::Permission,
    settings::{SettingValue, Settings},
};
use tracing::error;

use super::{PluginSettings, stored::set_enabled};

impl PluginSettings {
    /// Puts the Plugin `id` back as it ran before a change that failed with `failed` and saved
    /// nothing: running with `settings` and the Plugin Permissions `ran_with`, or not running if
    /// that is `None`. Should it not enable again, saves it as disabled, so it is stored as it
    /// runs, and logs both errors.
    pub(super) async fn put_back(
        &self,
        id: &PluginId,
        settings: &BTreeMap<String, SettingValue>,
        ran_with: Option<&HashSet<Permission>>,
        failed: &impl Display,
    ) {
        self.registry.disable(id).await;
        let Some(ran_with) = ran_with else {
            return;
        };
        let enabled = self.registry.enable_by_id_granting(
            id,
            Settings::new(settings.clone()),
            ran_with.clone(),
        );
        match enabled {
            Ok(()) => self.queue_came_up(id),
            Err(refused) => {
                error!(
                    plugin = %id,
                    "could not change the plugin's grants: {failed}; nor enable it again with \
                     those it had: {refused}"
                );
                if let Err(err) = set_enabled(&self.db, id.as_str(), false).await {
                    error!(plugin = %id, "could not save that the plugin is disabled: {err}");
                }
            }
        }
    }
}
