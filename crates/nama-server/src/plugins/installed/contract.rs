//! The Kernel's values as the Plugin contract's, and back.

use buffa::EnumValue;
use nama_plugin_api::{
    fulfiller::{Download, DownloadStatus},
    settings::{SettingKind, SettingValue},
};
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

/// The download `download` reports, as the Kernel keeps it on a Request.
pub(super) fn download(download: wire::Download) -> Download {
    let status = match download.status {
        EnumValue::Known(wire::DownloadStatus::DOWNLOAD_STATUS_QUEUED) => {
            DownloadStatus::DOWNLOAD_STATUS_QUEUED
        }
        EnumValue::Known(wire::DownloadStatus::DOWNLOAD_STATUS_DOWNLOADING) => {
            DownloadStatus::DOWNLOAD_STATUS_DOWNLOADING
        }
        EnumValue::Known(wire::DownloadStatus::DOWNLOAD_STATUS_IMPORTING) => {
            DownloadStatus::DOWNLOAD_STATUS_IMPORTING
        }
        EnumValue::Known(wire::DownloadStatus::DOWNLOAD_STATUS_NEEDS_ATTENTION) => {
            DownloadStatus::DOWNLOAD_STATUS_NEEDS_ATTENTION
        }
        EnumValue::Known(wire::DownloadStatus::DOWNLOAD_STATUS_UNSPECIFIED)
        | EnumValue::Unknown(_) => DownloadStatus::DOWNLOAD_STATUS_UNSPECIFIED,
    };
    Download {
        title: download.title,
        percent: download.percent,
        size_bytes: download.size_bytes,
        eta_seconds: download.eta_seconds,
        status: status.into(),
        attention_reason: download.attention_reason,
        ..Default::default()
    }
}

#[cfg(test)]
#[path = "contract_tests.rs"]
mod tests;
