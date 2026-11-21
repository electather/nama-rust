//! Running a Plugin's settings check, and its Qualities, whose replacement must fit what it offers.

use std::collections::BTreeMap;

use nama_plugin_api::{
    Manifest, PluginId,
    settings::{Choices, SettingKind, SettingValue, Settings},
};
use nama_proto::nama::v1::Quality;

use super::{ChangeError, PluginSettings, StoredPlugin, sent::with_stored_secrets};
use crate::requests::{QualitiesInvalid, check_qualities, list_qualities, replace_qualities};

impl PluginSettings {
    /// Runs the Plugin `id`'s settings check on `sent`, a secret left out keeping its stored value,
    /// and returns the choices it offers. Saves nothing.
    ///
    /// # Errors
    ///
    /// If no Plugin has the id or the Server refused it, the settings don't fit, the check fails
    /// or times out, or the database can't be read.
    pub async fn check(
        &self,
        id: &PluginId,
        sent: BTreeMap<String, SettingValue>,
    ) -> Result<Choices, ChangeError> {
        let (manifest, StoredPlugin { stored, .. }) = self.listed(id).await?;
        let settings = with_stored_secrets(&manifest.settings, sent, &stored.settings);
        self.run_check(id, settings, None).await
    }

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

    /// Runs the Plugin `id`'s settings check on `settings`, naming the choice `setting` a failed
    /// check refuses, if any.
    async fn run_check(
        &self,
        id: &PluginId,
        settings: BTreeMap<String, SettingValue>,
        setting: Option<&str>,
    ) -> Result<Choices, ChangeError> {
        self.registry
            .check_settings(id, Settings::new(settings))
            .await
            .map_err(|error| ChangeError::Check {
                plugin: id.clone(),
                error,
                setting: setting.map(str::to_owned),
            })
    }

    /// The Plugin `id`'s Manifest, and the Plugin with what is stored for it, unless no Plugin
    /// has the id or the Server refused it.
    pub(super) async fn listed(
        &self,
        id: &PluginId,
    ) -> Result<(Manifest, StoredPlugin), ChangeError> {
        let found = self
            .get(id)
            .await?
            .ok_or_else(|| ChangeError::Unknown(id.clone()))?;
        match &found.plugin.manifest {
            Ok(manifest) => Ok((manifest.clone(), found)),
            Err(reason) => Err(ChangeError::Refused {
                plugin: id.clone(),
                reason: reason.clone(),
            }),
        }
    }

    /// Refuses `settings` for the Plugin `manifest` describes unless its settings check passes on
    /// them and offers each choice setting's value. The check runs only when some choice setting
    /// has a value.
    pub(super) async fn check_choices(
        &self,
        manifest: &Manifest,
        settings: &BTreeMap<String, SettingValue>,
    ) -> Result<(), ChangeError> {
        let chosen: Vec<_> = manifest
            .settings
            .fields
            .iter()
            .filter(|field| field.kind == SettingKind::Choice)
            .filter_map(|field| match settings.get(&field.key) {
                Some(SettingValue::Text(value)) => Some((field.key.as_str(), value)),
                _ => None,
            })
            .collect();
        let Some(&(first, _)) = chosen.first() else {
            return Ok(());
        };
        let offered = self
            .run_check(&manifest.id, settings.clone(), Some(first))
            .await?;
        for (key, value) in chosen {
            let is_offered = offered
                .get(key)
                .is_some_and(|choices| choices.iter().any(|choice| choice.value == *value));
            if !is_offered {
                return Err(ChangeError::NotOffered {
                    plugin: manifest.id.clone(),
                    key: key.to_owned(),
                    value: value.clone(),
                });
            }
        }
        Ok(())
    }
}
