use nama_plugin_api::settings::{SettingKind, SettingValue};
use nama_proto::nama::plugin::v1::{self as wire, setting_value::Value};

use super::{setting_kind, setting_value};

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
