//! The Built-in Plugins: every Plugin compiled into the Server, listed here and nowhere else.

use std::sync::Arc;

use nama_plugin_api::{Plugin, PluginId};
use nama_plugin_hub::{Hub, KernelLink};
use nama_plugin_local_auth::LocalAuth;
use nama_plugin_log_events::LogEvents;
use nama_plugin_tmdb::Tmdb;
use nama_plugin_upkeep::Upkeep;
use nama_proto::nama::v1::PluginKind;

use super::{Registry, drop_flawed_presentation, kept_logo, registry::Described, what_it_says};

/// The Built-in Plugins that stay off until an Admin enables one; every other starts enabled.
/// The Hub Plugin, since a Server that isn't Linked doesn't enable it (ADR 0094).
pub(crate) const OFF_UNTIL_ENABLED: [PluginId; 1] = [Hub::ID];

fn all() -> Vec<fn() -> Arc<dyn Plugin>> {
    vec![
        || Arc::new(Hub::default()),
        || Arc::new(LocalAuth),
        || Arc::new(LogEvents),
        || Arc::new(Tmdb::new()),
        || Arc::new(Upkeep),
        #[cfg(feature = "testing")]
        || Arc::new(nama_plugin_test_choices::TestChoices),
    ]
}

impl Registry {
    /// Has the Hub Plugin, if this registry knows it, call the Hub through `link`, the Kernel's
    /// Server Link (ADR 0105). Without it, as when the Server's config names no Hub, it won't
    /// start.
    pub(crate) fn link_hub(&self, link: &Arc<dyn KernelLink>) {
        if self.plugin(&Hub::ID).is_some() {
            let link = Arc::clone(link);
            self.describe_builtin(move || Arc::new(Hub::through(Arc::clone(&link))));
        }
    }

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
        manifest.logo_svg = logo.and_then(|logo| kept_logo(&manifest.id, &logo));
        drop_flawed_presentation(&mut manifest);
        let id = manifest.id.clone();
        let described = Described {
            kind: PluginKind::PLUGIN_KIND_BUILT_IN,
            plugin: Ok((Arc::new(make), manifest)),
            checks_settings,
        };
        self.write(|state| state.known.insert(id, described));
    }
}
