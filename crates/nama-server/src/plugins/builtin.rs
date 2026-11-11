//! The Built-in Plugins: every Plugin compiled into the Server, listed here and nowhere else.

use std::sync::Arc;

use nama_plugin_api::Plugin;
use nama_plugin_local_auth::LocalAuth;
use nama_plugin_log_events::LogEvents;
use nama_plugin_tmdb::Tmdb;
use nama_proto::nama::v1::PluginKind;

use super::{Described, Registry, kept_logo, what_it_says};

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
