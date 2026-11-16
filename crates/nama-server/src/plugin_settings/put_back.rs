//! Putting a Plugin back as it ran before a change to its grants or settings failed, so what runs
//! agrees with what is stored (ADR 0028, 0049).

use std::fmt::Display;

use nama_plugin_api::PluginId;
use tracing::error;

use super::{PluginSettings, running::Ran, stored::set_enabled};

impl PluginSettings {
    /// Puts the Plugin `id` back as it ran before a change that failed with `failed`: running as
    /// `ran`, or not running if that is `None`. Should it not enable again, saves it as disabled,
    /// so it is stored as it runs, and logs both errors. Returns whether it saved it disabled, so a
    /// caller answering with what is stored doesn't say enabled.
    pub(super) async fn put_back(
        &self,
        id: &PluginId,
        ran: Option<Ran>,
        failed: &impl Display,
    ) -> bool {
        self.registry.disable(id).await;
        let Some(Ran { settings, granted }) = ran else {
            return false;
        };
        match self.enable_with(id, settings, granted) {
            Ok(()) => {
                self.queue_came_up(id);
                false
            }
            Err(refused) => {
                error!(
                    plugin = %id,
                    "could not change the plugin: {failed}; nor enable it again as it ran: \
                     {refused}"
                );
                match set_enabled(&self.db, id.as_str(), false).await {
                    Ok(()) => true,
                    Err(err) => {
                        error!(plugin = %id, "could not save that the plugin is disabled: {err}");
                        false
                    }
                }
            }
        }
    }
}
