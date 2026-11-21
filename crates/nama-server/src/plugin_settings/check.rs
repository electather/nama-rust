//! Running a Plugin's settings check, and refusing settings whose choices aren't offered.

use std::{collections::BTreeMap, slice};

use nama_plugin_api::{
    Manifest, PluginId,
    settings::{Choices, SettingField, SettingKind, SettingValue, Settings, in_listed_order},
};

use super::{ChangeError, PluginSettings, StoredPlugin, sent::with_stored_secrets};

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

    /// Runs the Plugin `id`'s settings check on `settings`, naming the choice `setting` a failed
    /// check refuses, if any.
    pub(super) async fn run_check(
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

    /// Refuses `settings` for the Plugin `manifest` describes unless each value a choice or
    /// Choices setting holds is offered: among the choices its settings check offers for it, or
    /// else a Choices setting's own. Keeps each Choices setting's values once, in the order its
    /// choices are listed. The check runs only when a choice setting has a value, or the Plugin
    /// `checks_settings` and a Choices setting holds some.
    pub(super) async fn check_choices(
        &self,
        manifest: &Manifest,
        checks_settings: bool,
        settings: &mut BTreeMap<String, SettingValue>,
    ) -> Result<(), ChangeError> {
        let picked = picked(&manifest.settings.fields, settings);
        let needs_check =
            (picked.iter()).any(|(field, _)| checks_settings || field.kind == SettingKind::Choice);
        let offered = match picked.first() {
            Some((first, _)) if needs_check => {
                self.run_check(&manifest.id, settings.clone(), Some(&first.key))
                    .await?
            }
            Some(_) | None => Choices::new(),
        };
        let mut ordered = Vec::new();
        for (field, values) in picked {
            let listed = offered.get(&field.key).unwrap_or(&field.choices);
            let unoffered =
                (values.iter()).find(|value| !listed.iter().any(|choice| choice.value == **value));
            if let Some(value) = unoffered {
                return Err(ChangeError::NotOffered {
                    plugin: manifest.id.clone(),
                    key: field.key.clone(),
                    value: value.clone(),
                });
            }
            if field.kind == SettingKind::Choices {
                let kept = SettingValue::Texts(in_listed_order(listed, values));
                ordered.push((field.key.clone(), kept));
            }
        }
        settings.extend(ordered);
        Ok(())
    }
}

/// Each choice or Choices setting in `fields` that `settings` gives a value, with the values it
/// holds: a choice setting's one, or a Choices setting's, if it holds any.
fn picked<'a>(
    fields: &'a [SettingField],
    settings: &'a BTreeMap<String, SettingValue>,
) -> Vec<(&'a SettingField, &'a [String])> {
    fields
        .iter()
        .filter_map(|field| {
            let values = match (field.kind, settings.get(&field.key)?) {
                (SettingKind::Choice, SettingValue::Text(value)) => slice::from_ref(value),
                (SettingKind::Choices, SettingValue::Texts(values)) if !values.is_empty() => values,
                _ => return None,
            };
            Some((field, values))
        })
        .collect()
}
