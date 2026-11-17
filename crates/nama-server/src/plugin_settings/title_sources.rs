//! What each Plugin, enabled or not, says of its title source, for judging a Library's order.

use nama_plugin_api::metadata::SourceManifest;

use super::PluginSettings;

impl PluginSettings {
    /// What every Plugin the Server has, enabled or not, says of its title source in its Manifest.
    /// Doesn't read the Installed Plugins again as [`PluginSettings::list`] does.
    #[must_use]
    pub(crate) fn title_sources(&self) -> Vec<SourceManifest> {
        (self.registry.plugins().into_iter())
            .filter_map(|plugin| plugin.manifest.ok()?.title_source)
            .collect()
    }
}
