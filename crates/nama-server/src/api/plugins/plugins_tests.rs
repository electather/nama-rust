use nama_plugin_api::settings::{SettingField, SettingKind, SettingValue};
use nama_proto::nama::v1 as proto;

use super::{
    points::public_points, setting_to_proto, setting_value_from_proto, setting_value_to_proto,
};
use crate::plugins::installed_points;

#[test]
fn a_secrets_default_and_stored_value_are_never_sent() {
    let token = SettingField {
        key: "token".to_owned(),
        label: "Token".to_owned(),
        description: String::new(),
        kind: SettingKind::Secret,
        required: false,
        default: Some(SettingValue::Text("default-token".to_owned())),
        min: None,
        max: None,
        choices: Vec::new(),
    };
    let stored = SettingValue::Text("stored-token".to_owned());

    let shown = setting_to_proto(&token, Some(&stored));

    assert!(shown.default_value.is_unset(), "{shown:?}");
    assert!(shown.value.is_unset(), "{shown:?}");
    assert!(shown.secret_set);
}

#[test]
fn each_value_comes_back_from_the_schema_as_it_went() {
    let values = [
        SettingValue::Text("en".to_owned()),
        SettingValue::Bool(true),
        SettingValue::Integer(-3),
        SettingValue::Libraries(vec![7, 2]),
    ];

    let values_back = values
        .each_ref()
        .map(|value| setting_value_from_proto(&setting_value_to_proto(value)));

    assert_eq!(values_back, values.map(Some));
    assert_eq!(
        setting_value_from_proto(&proto::SettingValue::default()),
        None
    );
}

#[test]
fn every_point_an_installed_plugin_can_serve_shows_on_the_plugins_page() {
    let shown = public_points().map(|(key, _)| key);

    for (service, key) in installed_points() {
        assert!(
            shown.contains(&key),
            "{service} has no entry in public_points(), so the Plugins page hides it"
        );
    }
}
