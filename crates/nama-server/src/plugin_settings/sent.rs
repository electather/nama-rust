//! Fitting the settings an Admin sends for a Plugin to its schema before they are saved.

use std::collections::BTreeMap;

use nama_plugin_api::settings::{SettingKind, SettingValue, SettingsSchema};

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
