//! The Built-in Plugins: every Plugin compiled into the Server, listed here and nowhere else.

use std::{collections::BTreeMap, sync::Arc};

use nama_plugin_api::{Plugin, PluginId, settings::Settings};
use nama_plugin_local_auth::LocalAuth;
use nama_plugin_log_events::LogEvents;
use nama_plugin_tmdb::Tmdb;
use tracing::{error, info, warn};

use super::{Described, EnableError, Registry, contain_sync};
use crate::config::PluginConfig;

/// How to make each Built-in Plugin.
fn all() -> [fn() -> Arc<dyn Plugin>; 3] {
    [
        || Arc::new(LocalAuth),
        || Arc::new(LogEvents),
        || Arc::new(Tmdb::new()),
    ]
}

impl Registry {
    /// A registry with every Built-in Plugin enabled with its settings from `config`, except
    /// those `config` disables. A Plugin `config` doesn't name is enabled with no settings.
    ///
    /// A Plugin that fails to enable, such as one missing a required setting, is logged and left
    /// out.
    #[must_use]
    pub fn with_builtins(config: &BTreeMap<String, PluginConfig>) -> Self {
        let registry = Self::new();
        let mut known = Vec::new();
        for new in all() {
            // Making a Plugin and its `id` are Plugin code, so a panic in either is contained.
            let Ok(plugin) = contain_sync(new) else {
                error!("a built-in plugin panicked while being made");
                continue;
            };
            let id = match contain_sync(|| plugin.id()) {
                Ok(id) => id,
                Err(message) => {
                    error!(message, "a built-in plugin panicked while giving its id");
                    continue;
                }
            };
            known.push(id.clone());
            match contain_sync(|| (plugin.version().to_owned(), plugin.settings_schema())) {
                Ok((version, settings_schema)) => {
                    let described = Described {
                        version,
                        settings_schema,
                        new,
                    };
                    registry.write(|state| state.builtins.insert(id.clone(), described));
                }
                Err(message) => {
                    error!(plugin = %id, message, "a built-in plugin panicked while describing itself");
                    continue;
                }
            }
            let config = config.get(id.as_str()).cloned().unwrap_or_default();
            if !config.enabled {
                info!(plugin = %id, "plugin disabled in its settings");
                continue;
            }
            match registry.enable_with_settings(plugin, Settings::new(config.settings)) {
                Ok(()) => {}
                // Not an error: a Plugin such as TMDB needs an API key before it can run.
                Err(err @ EnableError::InvalidSettings { .. }) => {
                    warn!(plugin = %id, "plugin not enabled: {err}");
                }
                Err(err) => error!(plugin = %id, "could not enable plugin: {err}"),
            }
        }
        for id in config.keys() {
            if !known.iter().any(|known| known.as_str() == id) {
                warn!(plugin = %id, "settings name a plugin this Server doesn't have");
            }
        }
        registry
    }

    /// Enables the Built-in Plugin `id` with `settings`, as a new instance that registers afresh.
    /// If it is enabled, it is disabled first, so nothing it registered with its old settings
    /// stays.
    ///
    /// # Errors
    ///
    /// If no Built-in Plugin has the id, the settings don't fit its schema, or it panics. It is
    /// left disabled.
    pub fn enable_builtin(&self, id: &PluginId, settings: Settings) -> Result<(), EnableError> {
        let new = self
            .read(|state| state.builtins.get(id).map(|described| described.new))
            .ok_or_else(|| EnableError::NotBuiltin(id.clone()))?;
        let plugin = contain_sync(new).map_err(|message| EnableError::Panicked {
            plugin: id.clone(),
            message,
        })?;
        self.disable(id);
        self.enable_with_settings(plugin, settings)
    }
}
