//! Saving each Plugin the registry disables for crashing as disabled.

use nama_plugin_api::PluginId;
use tokio::sync::mpsc;
use tracing::error;

use super::{PluginSettings, set_enabled};

impl PluginSettings {
    /// Saves each Plugin the registry disables for crashing as disabled, as an admin's disable is,
    /// so it stays disabled across a restart until an admin enables it again. Runs until
    /// [`PluginSettings::shutdown`] and the crashes queued are saved, or until the registry hands
    /// a later caller of [`Registry::crashed`](crate::plugins::Registry::crashed) the crashes
    /// instead.
    pub(super) async fn save_crashed(self, mut crashed: mpsc::UnboundedReceiver<PluginId>) {
        loop {
            tokio::select! {
                // Stopping first, so a stop always saves the queue through the one drain below.
                biased;
                () = self.stop_saving_crashed.cancelled() => break,
                id = crashed.recv() => match id {
                    Some(id) => self.save_crashed_plugin(&id).await,
                    None => return,
                },
            }
        }
        while let Ok(id) = crashed.try_recv() {
            self.save_crashed_plugin(&id).await;
        }
    }

    async fn save_crashed_plugin(&self, id: &PluginId) {
        let _changing = self.lock().await;
        // An admin may have enabled it again since.
        if self.registry.enabled().contains(id) {
            return;
        }
        if let Err(err) = set_enabled(&self.db, id.as_str(), false).await {
            error!(plugin = %id, "could not save that the crashed plugin is disabled: {err}");
        }
        self.forget_disabled(id).await;
    }
}
