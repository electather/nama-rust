//! A Fulfiller's Qualities, checked against the profiles its settings check offers.

use nama_plugin_api::{PluginId, settings::Choices};
use nama_proto::nama::v1::Quality;

use super::{ChangeError, PluginSettings, StoredPlugin};
use crate::requests::{QualitiesInvalid, check_qualities, list_qualities, replace_qualities};

impl PluginSettings {
    /// Runs the Plugin `id`'s settings check on its stored settings, and returns the choices it
    /// offers.
    async fn check_stored(&self, id: &PluginId) -> Result<Choices, ChangeError> {
        let (_, StoredPlugin { stored, .. }) = self.listed(id).await?;
        self.run_check(id, stored.settings, None).await
    }

    /// The Plugin `id`'s Qualities, in the order the admin listed them.
    pub(crate) async fn qualities(&self, id: &PluginId) -> Result<Vec<Quality>, ChangeError> {
        self.listed(id).await?;
        let mut conn = self.db.reader().acquire().await?;
        Ok(list_qualities(&mut conn, id).await?)
    }

    /// Replaces the Plugin `id`'s Qualities with `listed` and returns them as saved, or why
    /// [`check_qualities`] refused them against the profiles its stored settings' check offers.
    pub(crate) async fn replace_qualities(
        &self,
        id: &PluginId,
        listed: Vec<Quality>,
    ) -> Result<Result<Vec<Quality>, QualitiesInvalid>, ChangeError> {
        // Only a list with Qualities asks the settings check, so a Fulfiller that's down can still
        // have its list cleared.
        let offered = if listed.is_empty() {
            self.listed(id).await?;
            Choices::new()
        } else {
            self.check_stored(id).await?
        };
        if let Err(invalid) = check_qualities(&listed, &offered) {
            return Ok(Err(invalid));
        }
        Ok(Ok(replace_qualities(&self.db, id.clone(), listed).await?))
    }
}
