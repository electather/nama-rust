//! What each Plugin, enabled or not, says of its title source, for judging a Library's order.

use nama_plugin_api::{PluginId, metadata::SourceManifest};

use super::PluginSettings;

impl PluginSettings {
    /// What every Plugin the Server has, enabled or not, says of its title source in its Manifest.
    /// Doesn't read the Installed Plugins again as [`PluginSettings::list`] does.
    #[must_use]
    pub(crate) fn title_sources(&self) -> Vec<SourceManifest> {
        (self.title_sources_by_plugin().into_iter())
            .map(|(_, source)| source)
            .collect()
    }

    /// [`PluginSettings::title_sources`], each with the id of the Plugin that says it.
    #[must_use]
    pub(crate) fn title_sources_by_plugin(&self) -> Vec<(PluginId, SourceManifest)> {
        (self.registry.plugins().into_iter())
            .filter_map(|plugin| Some((plugin.id, plugin.manifest.ok()?.title_source?)))
            .collect()
    }
}
