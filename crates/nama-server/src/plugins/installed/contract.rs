//! The Kernel's values as the Plugin contract's, and back.

use nama_plugin_api::settings::{SettingKind, SettingValue};
use nama_proto::nama::plugin::v1::{self as wire, setting_value};

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

#[cfg(test)]
#[path = "contract_tests.rs"]
mod tests;
