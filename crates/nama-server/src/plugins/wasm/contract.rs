//! The Plugin contract's setting messages as the Kernel's, and where a Manifest names what its
//! Fulfillers need.

use nama_plugin_api::{
    fulfiller::{Movies, Shows},
    settings::{SettingKind, SettingValue},
};
use nama_plugin_proto::nama::plugin::v1::{self as wire, setting_value};

/// The kind `kind` names, or `None` for `SETTING_KIND_UNSPECIFIED`.
pub(super) const fn setting_kind(kind: wire::SettingKind) -> Option<SettingKind> {
    Some(match kind {
        wire::SettingKind::SETTING_KIND_UNSPECIFIED => return None,
        wire::SettingKind::SETTING_KIND_TEXT => SettingKind::Text,
        wire::SettingKind::SETTING_KIND_SECRET => SettingKind::Secret,
        wire::SettingKind::SETTING_KIND_BOOL => SettingKind::Bool,
        wire::SettingKind::SETTING_KIND_INTEGER => SettingKind::Integer,
        wire::SettingKind::SETTING_KIND_CHOICE => SettingKind::Choice,
        wire::SettingKind::SETTING_KIND_URL => SettingKind::Url,
    })
}

/// The value `value` holds, or `None` if it holds none.
pub(super) fn setting_value(value: &wire::SettingValue) -> Option<SettingValue> {
    Some(match value.value.as_ref()? {
        setting_value::Value::TextValue(text) => SettingValue::Text(text.clone()),
        setting_value::Value::BoolValue(on) => SettingValue::Bool(*on),
        setting_value::Value::IntegerValue(number) => SettingValue::Integer(*number),
    })
}

/// A kind of title a Fulfiller is registered under, with where a Manifest names its needs.
pub(super) trait FulfillerKind {
    /// The id namespaces `manifest` says its Fulfiller of this kind needs, as written: one that
    /// isn't well formed never matches a title.
    fn needs(manifest: &wire::Manifest) -> Vec<String>;
}

impl FulfillerKind for Movies {
    fn needs(manifest: &wire::Manifest) -> Vec<String> {
        manifest.movie_id_namespaces.clone()
    }
}

impl FulfillerKind for Shows {
    fn needs(manifest: &wire::Manifest) -> Vec<String> {
        manifest.show_id_namespaces.clone()
    }
}

#[cfg(test)]
#[path = "contract_tests.rs"]
mod tests;
