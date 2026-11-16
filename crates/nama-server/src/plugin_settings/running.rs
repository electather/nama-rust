//! What each Plugin runs with, which after a settings change that failed to enable it differs
//! from what is stored, so a later failed change puts it back as it ran (ADR 0028).

use std::collections::{BTreeMap, HashSet};

use nama_plugin_api::{
    PluginId,
    automation::Permission,
    settings::{SettingValue, Settings},
};

use super::{PluginSettings, lock_ignoring_poison};
use crate::plugins::EnableError;

/// The settings and Plugin Permissions a Plugin was last enabled with.
#[derive(Debug, Clone)]
pub(super) struct Ran {
    pub(super) settings: BTreeMap<String, SettingValue>,
    pub(super) granted: HashSet<Permission>,
}

impl PluginSettings {
    /// Enables the Plugin `id` with `settings` and the Plugin Permissions `granted`, and records
    /// them as what it runs with. Every enable goes through here, so the record is never older
    /// than what runs.
    pub(super) fn enable_with(
        &self,
        id: &PluginId,
        settings: BTreeMap<String, SettingValue>,
        granted: HashSet<Permission>,
    ) -> Result<(), EnableError> {
        self.registry.enable_by_id_granting(
            id,
            Settings::new(settings.clone()),
            granted.clone(),
        )?;
        lock_ignoring_poison(&self.running).insert(id.clone(), Ran { settings, granted });
        Ok(())
    }

    /// What the Plugin `id` runs with, or `None` if it isn't running.
    // The record outlives a disable or a crash, so it counts only while the Plugin is enabled.
    pub(super) fn ran_with(&self, id: &PluginId) -> Option<Ran> {
        if !self.registry.enabled().contains(id) {
            return None;
        }
        lock_ignoring_poison(&self.running).get(id).cloned()
    }
}
