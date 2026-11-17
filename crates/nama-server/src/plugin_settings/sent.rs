//! Fitting the settings an Admin sends for a Plugin to its schema before they are saved.

use std::collections::BTreeMap;

use nama_plugin_api::settings::{
    SettingKind, SettingValue, Settings, SettingsError, SettingsSchema,
};

/// The settings `sent` for a Plugin with `schema`: the ones it names, with a secret left out
/// keeping its `stored` value, so a form can replace a secret without showing it.
pub(super) fn with_stored_secrets(
    schema: &SettingsSchema,
    mut sent: BTreeMap<String, SettingValue>,
    stored: &BTreeMap<String, SettingValue>,
) -> BTreeMap<String, SettingValue> {
    schema
        .fields
        .iter()
        .filter_map(|field| {
            let kept = || {
                let secret = field.kind == SettingKind::Secret;
                stored.get(&field.key).filter(|_| secret).cloned()
            };
            Some((field.key.clone(), sent.remove(&field.key).or_else(kept)?))
        })
        .collect()
}

/// Refuses `settings` unless they fit `schema`. A required setting left out is refused only for a
/// Plugin stored as enabled, which saving enables again; enabling a disabled one checks it then.
pub(super) fn check_fits(
    schema: &SettingsSchema,
    settings: &BTreeMap<String, SettingValue>,
    stored_enabled: bool,
) -> Result<(), SettingsError> {
    let mut schema = schema.clone();
    if !stored_enabled {
        // Not by ignoring `Missing` afterwards: `resolve` returns it before checking any kind.
        for field in &mut schema.fields {
            field.required = false;
        }
    }
    Settings::new(settings.clone()).resolve(&schema).map(drop)
}
