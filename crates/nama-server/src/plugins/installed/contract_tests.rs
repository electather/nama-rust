use buffa::EnumValue;
use nama_plugin_api::{
    fulfiller::{Download, DownloadStatus},
    settings::{SettingKind, SettingValue},
};
use nama_proto::nama::plugin::v1::{self as wire, setting_value::Value};

use super::{download, setting_kind, setting_value};

#[test]
fn each_kind_becomes_its_twin_and_unspecified_none() {
    let kinds = [
        wire::SettingKind::SETTING_KIND_UNSPECIFIED,
        wire::SettingKind::SETTING_KIND_TEXT,
        wire::SettingKind::SETTING_KIND_SECRET,
        wire::SettingKind::SETTING_KIND_BOOL,
        wire::SettingKind::SETTING_KIND_INTEGER,
        wire::SettingKind::SETTING_KIND_CHOICE,
        wire::SettingKind::SETTING_KIND_URL,
    ];

    assert_eq!(
        kinds.map(setting_kind),
        [
            None,
            Some(SettingKind::Text),
            Some(SettingKind::Secret),
            Some(SettingKind::Bool),
            Some(SettingKind::Integer),
            Some(SettingKind::Choice),
            Some(SettingKind::Url),
        ]
    );
}

#[test]
fn each_value_comes_back_as_it_went_and_an_empty_one_as_none() {
    let values = [
        Some(Value::TextValue("en".to_owned())),
        Some(Value::BoolValue(true)),
        Some(Value::IntegerValue(-3)),
        None,
    ];

    let back = values.map(|value| {
        setting_value(&wire::SettingValue {
            value,
            ..Default::default()
        })
    });

    assert_eq!(
        back,
        [
            Some(SettingValue::Text("en".to_owned())),
            Some(SettingValue::Bool(true)),
            Some(SettingValue::Integer(-3)),
            None,
        ]
    );
}

#[test]
fn a_download_keeps_its_fields_and_an_unknown_status_becomes_unspecified() {
    let reported = |status| wire::Download {
        title: "Dune".to_owned(),
        percent: 42.5,
        size_bytes: Some(1_000),
        eta_seconds: Some(90),
        status,
        attention_reason: Some("stalled".to_owned()),
        ..Default::default()
    };
    let kept = |status: DownloadStatus| Download {
        title: "Dune".to_owned(),
        percent: 42.5,
        size_bytes: Some(1_000),
        eta_seconds: Some(90),
        status: status.into(),
        attention_reason: Some("stalled".to_owned()),
        ..Default::default()
    };
    let statuses = [
        (
            EnumValue::Unknown(99),
            DownloadStatus::DOWNLOAD_STATUS_UNSPECIFIED,
        ),
        (
            wire::DownloadStatus::DOWNLOAD_STATUS_UNSPECIFIED.into(),
            DownloadStatus::DOWNLOAD_STATUS_UNSPECIFIED,
        ),
        (
            wire::DownloadStatus::DOWNLOAD_STATUS_QUEUED.into(),
            DownloadStatus::DOWNLOAD_STATUS_QUEUED,
        ),
        (
            wire::DownloadStatus::DOWNLOAD_STATUS_DOWNLOADING.into(),
            DownloadStatus::DOWNLOAD_STATUS_DOWNLOADING,
        ),
        (
            wire::DownloadStatus::DOWNLOAD_STATUS_IMPORTING.into(),
            DownloadStatus::DOWNLOAD_STATUS_IMPORTING,
        ),
        (
            wire::DownloadStatus::DOWNLOAD_STATUS_NEEDS_ATTENTION.into(),
            DownloadStatus::DOWNLOAD_STATUS_NEEDS_ATTENTION,
        ),
    ];

    for (status, expected) in statuses {
        assert_eq!(download(reported(status)), kept(expected));
    }
}
