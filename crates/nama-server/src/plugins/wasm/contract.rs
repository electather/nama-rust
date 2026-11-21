//! The Plugin contract's setting messages as the Kernel's, and where a Manifest names what its
//! Fulfillers need.

use nama_plugin_api::{
    fulfiller::{Movies, Shows},
    settings::{Choice, SettingKind, SettingValue},
};
use nama_plugin_proto::nama::plugin::v1::{self as wire, setting_value};

use super::services::{
    MOVIE_FULFILL, MOVIE_PROGRESS, MOVIE_STOP_MONITORING, Method, SHOW_FULFILL, SHOW_PROGRESS,
    SHOW_STOP_MONITORING, SHOW_UNFOLLOW,
};

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
        wire::SettingKind::SETTING_KIND_LIBRARIES => SettingKind::Libraries,
        wire::SettingKind::SETTING_KIND_CHOICES => SettingKind::Choices,
    })
}

/// The value `value` holds, or `None` if it holds none.
pub(super) fn setting_value(value: &wire::SettingValue) -> Option<SettingValue> {
    Some(match value.value.as_ref()? {
        setting_value::Value::TextValue(text) => SettingValue::Text(text.clone()),
        setting_value::Value::BoolValue(on) => SettingValue::Bool(*on),
        setting_value::Value::IntegerValue(number) => SettingValue::Integer(*number),
        setting_value::Value::LibraryIds(libraries) => {
            SettingValue::Libraries(libraries.ids.clone())
        }
        setting_value::Value::Texts(texts) => SettingValue::Texts(texts.values.clone()),
    })
}

pub(super) fn choice(choice: wire::Choice) -> Choice {
    Choice {
        value: choice.value,
        label: choice.label,
    }
}

/// A kind of title a Fulfiller is registered under, with where a Manifest names its needs and
/// its service's rows.
pub(super) trait FulfillerKind {
    const FULFILL: &'static Method;
    const STOP_MONITORING: &'static Method;
    const PROGRESS: &'static Method;
    /// `None` for a service without the method.
    const UNFOLLOW: Option<&'static Method>;

    /// The id namespaces `manifest` says its Fulfiller of this kind needs, as written: one that
    /// isn't well formed never matches a title.
    fn needs(manifest: &wire::Manifest) -> Vec<String>;
}

impl FulfillerKind for Movies {
    const FULFILL: &'static Method = &MOVIE_FULFILL;
    const STOP_MONITORING: &'static Method = &MOVIE_STOP_MONITORING;
    const PROGRESS: &'static Method = &MOVIE_PROGRESS;
    const UNFOLLOW: Option<&'static Method> = None;

    fn needs(manifest: &wire::Manifest) -> Vec<String> {
        manifest.movie_id_namespaces.clone()
    }
}

impl FulfillerKind for Shows {
    const FULFILL: &'static Method = &SHOW_FULFILL;
    const STOP_MONITORING: &'static Method = &SHOW_STOP_MONITORING;
    const PROGRESS: &'static Method = &SHOW_PROGRESS;
    const UNFOLLOW: Option<&'static Method> = Some(&SHOW_UNFOLLOW);

    fn needs(manifest: &wire::Manifest) -> Vec<String> {
        manifest.show_id_namespaces.clone()
    }
}

#[cfg(test)]
#[path = "contract_tests.rs"]
mod tests;
