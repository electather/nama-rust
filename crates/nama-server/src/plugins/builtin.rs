//! The Built-in Plugins: every Plugin compiled into the Server, listed here and nowhere else.

use std::{collections::BTreeMap, sync::Arc};

use nama_plugin_api::Plugin;
use nama_plugin_log_events::LogEvents;
use tracing::{error, info, warn};

use super::Registry;
use crate::config::PluginConfig;

/// Every Built-in Plugin.
fn all() -> Vec<Arc<dyn Plugin>> {
    vec![Arc::new(LogEvents)]
}

impl Registry {
    /// A registry with every Built-in Plugin enabled, except those `config` disables.
    ///
    /// A Plugin that fails to enable is logged and left out.
    #[must_use]
    pub fn with_builtins(config: &BTreeMap<String, PluginConfig>) -> Self {
        let registry = Self::new();
        let builtins = all();
        for plugin in builtins.iter().cloned() {
            let id = plugin.id();
            if config
                .get(id.as_str())
                .is_some_and(|settings| !settings.enabled)
            {
                info!(plugin = %id, "plugin disabled in config");
                continue;
            }
            if let Err(err) = registry.enable(plugin) {
                error!(plugin = %id, "could not enable plugin: {err}");
            }
        }
        for id in config.keys() {
            if !builtins.iter().any(|plugin| plugin.id().as_str() == id) {
                warn!(plugin = %id, "config names a plugin this Server doesn't have");
            }
        }
        registry
    }
}
