//! The Built-in Plugins: every Plugin compiled into the Server, listed here and nowhere else.

use std::sync::Arc;

use nama_plugin_api::Plugin;
use nama_plugin_local_auth::LocalAuth;
use nama_plugin_log_events::LogEvents;
use nama_plugin_tmdb::Tmdb;
use tracing::error;

use nama_proto::nama::v1::PluginKind;

use super::{Described, Registry, contain_sync, kept_logo};

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
    /// A registry that knows every Built-in Plugin and enables none.
    #[must_use]
    pub fn with_builtins() -> Self {
        let registry = Self::new();
        for make in all() {
            registry.describe_builtin(make);
        }
        registry
    }

    /// Makes the Plugin `make` makes known as a Built-in one, without enabling it, in place of any
    /// with its id. One that panics while being made or describing itself is logged and left out.
    pub fn describe_builtin(&self, make: impl Fn() -> Arc<dyn Plugin> + Send + Sync + 'static) {
        let plugin = match contain_sync(&make) {
            Ok(plugin) => plugin,
            Err(message) => {
                error!(message, "a built-in plugin panicked while being made");
                return;
            }
        };
        // A Plugin's `manifest` and `settings_check` are Plugin code, so a panic in them is
        // contained.
        let manifest = match contain_sync(|| plugin.manifest()) {
            Ok(manifest) => manifest,
            Err(message) => {
                error!(
                    message,
                    "a built-in plugin panicked while giving its manifest"
                );
                return;
            }
        };
        let id = manifest.id;
        let checks_settings = match contain_sync(|| plugin.settings_check().is_some()) {
            Ok(checks_settings) => checks_settings,
            Err(message) => {
                error!(plugin = %id, message, "a built-in plugin panicked while describing itself");
                return;
            }
        };
        let described = Described {
            kind: PluginKind::PLUGIN_KIND_BUILT_IN,
            make: Ok(Arc::new(make)),
            name: manifest.name,
            description: manifest.description,
            version: manifest.version,
            settings_schema: manifest.settings,
            checks_settings,
            extension_points: manifest.extension_points,
            logo_svg: manifest.logo_svg.and_then(|logo| kept_logo(&id, logo)),
        };
        self.write(|state| state.known.insert(id, described));
    }
}
