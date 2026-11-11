//! The Built-in Plugins: every Plugin compiled into the Server, listed here and nowhere else.

use std::sync::Arc;

use nama_plugin_api::{Manifest, Plugin};
use nama_plugin_local_auth::LocalAuth;
use nama_plugin_log_events::LogEvents;
use nama_plugin_tmdb::Tmdb;
use tracing::error;

use nama_proto::nama::v1::PluginKind;

use super::{Described, Registry, contain_sync, kept_logo};

fn all() -> Vec<fn() -> Arc<dyn Plugin>> {
    vec![
        || Arc::new(LocalAuth),
        || Arc::new(LogEvents),
        || Arc::new(Tmdb::new()),
        #[cfg(feature = "testing")]
        || Arc::new(nama_plugin_test_choices::TestChoices),
    ]
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
        let Some((mut manifest, checks_settings)) = what_it_says(&make) else {
            return;
        };
        let logo = manifest.logo_svg.take();
        manifest.logo_svg = logo.and_then(|logo| kept_logo(&manifest.id, logo));
        let id = manifest.id.clone();
        let described = Described {
            kind: PluginKind::PLUGIN_KIND_BUILT_IN,
            plugin: Ok((Arc::new(make), manifest)),
            checks_settings,
        };
        self.write(|state| state.known.insert(id, described));
    }
}

/// What the Plugin `make` makes says about itself, as [`asked`] gets it, or `None`, logged, if it
/// panics being made.
pub(super) fn what_it_says(make: &dyn Fn() -> Arc<dyn Plugin>) -> Option<(Manifest, bool)> {
    let plugin = contain_sync(make)
        .inspect_err(|message| error!(message, "a plugin panicked while being made"))
        .ok()?;
    let said = asked(&*plugin);
    // A panic dropping it is moot: what it said about itself is already read.
    let _dropped = contain_sync(|| drop(plugin));
    said
}

/// `plugin`'s Manifest and whether it has a settings check, or `None`, logged, if it panics
/// giving them: both are Plugin code, so a panic in them is contained.
fn asked(plugin: &dyn Plugin) -> Option<(Manifest, bool)> {
    let manifest = contain_sync(|| plugin.manifest())
        .inspect_err(|message| {
            error!(message, "a plugin panicked while giving its manifest");
        })
        .ok()?;
    let checks_settings = contain_sync(|| plugin.settings_check().is_some())
        .inspect_err(|message| {
            error!(plugin = %manifest.id, message, "a plugin panicked while describing itself");
        })
        .ok()?;
    Some((manifest, checks_settings))
}
