//! The Built-in Plugins: every Plugin compiled into the Server, listed here and nowhere else.

use std::{collections::BTreeMap, sync::Arc};

use nama_plugin_api::{Plugin, settings::Settings};
use nama_plugin_local_auth::LocalAuth;
use nama_plugin_log_events::LogEvents;
use nama_plugin_tmdb::Tmdb;
use tracing::{error, info};

use nama_proto::nama::v1::PluginKind;

use super::{Described, Registry, contain_sync, log_not_enabled};
use crate::config::PluginConfig;

fn all() -> Vec<fn() -> Arc<dyn Plugin>> {
    let mut all: Vec<fn() -> Arc<dyn Plugin>> =
        vec![|| Arc::new(LocalAuth), || Arc::new(LogEvents), || {
            Arc::new(Tmdb::new())
        }];
    #[cfg(feature = "testing")]
    all.push(|| Arc::new(nama_plugin_test_choices::TestChoices));
    all
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
        for make in all() {
            let plugin = match contain_sync(make) {
                Ok(plugin) => plugin,
                Err(message) => {
                    error!(message, "a built-in plugin panicked while being made");
                    continue;
                }
            };
            // A Plugin's `manifest` and `settings_check` are Plugin code, so a panic in them is
            // contained.
            let describe = || (plugin.manifest(), plugin.settings_check().is_some());
            let (manifest, checks_settings) = match contain_sync(describe) {
                Ok(described) => described,
                Err(message) => {
                    error!(
                        message,
                        "a built-in plugin panicked while describing itself"
                    );
                    continue;
                }
            };
            let id = manifest.id;
            let described = Described {
                kind: PluginKind::PLUGIN_KIND_BUILT_IN,
                make: Ok(Arc::new(make)),
                version: manifest.version,
                settings_schema: manifest.settings,
                checks_settings,
            };
            registry.write(|state| state.known.insert(id.clone(), described));
            let config = config.get(id.as_str()).cloned().unwrap_or_default();
            if !config.enabled {
                info!(plugin = %id, "plugin disabled in its settings");
                continue;
            }
            if let Err(err) = registry.enable_with_settings(plugin, Settings::new(config.settings))
            {
                log_not_enabled(&id, &err);
            }
        }
        registry
    }
}
