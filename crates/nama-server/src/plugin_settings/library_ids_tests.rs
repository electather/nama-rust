use std::collections::{BTreeMap, HashSet};

use nama_plugin_api::settings::{SettingField, SettingKind, SettingValue, SettingsSchema};

use super::unknown_library;

fn schema(fields: &[(&str, SettingKind)]) -> SettingsSchema {
    SettingsSchema {
        fields: fields
            .iter()
            .map(|&(key, kind)| SettingField {
                key: key.to_owned(),
                label: key.to_owned(),
                description: String::new(),
                kind,
                required: false,
                default: None,
                min: None,
                max: None,
            })
            .collect(),
    }
}

fn settings(values: &[(&str, SettingValue)]) -> BTreeMap<String, SettingValue> {
    values
        .iter()
        .map(|(key, value)| ((*key).to_owned(), value.clone()))
        .collect()
}

#[test]
fn ids_every_library_has_are_taken() {
    let schema = schema(&[("keep", SettingKind::Libraries)]);
    let known = HashSet::from([1, 2, 3]);

    let named = settings(&[("keep", SettingValue::Libraries(vec![3, 1]))]);
    let none = settings(&[("keep", SettingValue::Libraries(Vec::new()))]);

    assert_eq!(unknown_library(&schema, &named, &known), None);
    assert_eq!(unknown_library(&schema, &none, &known), None);
    assert_eq!(unknown_library(&schema, &settings(&[]), &known), None);
}

#[test]
fn the_first_id_no_library_has_is_named_with_its_setting() {
    let schema = schema(&[
        ("keep", SettingKind::Libraries),
        ("also", SettingKind::Libraries),
    ]);
    let known = HashSet::from([1]);

    let refused = unknown_library(
        &schema,
        &settings(&[
            ("also", SettingValue::Libraries(vec![9])),
            ("keep", SettingValue::Libraries(vec![1, 7, 8])),
        ]),
        &known,
    );

    assert_eq!(refused, Some(("keep".to_owned(), 7)));
}

#[test]
fn a_setting_of_another_kind_is_never_read_as_library_ids() {
    let schema = schema(&[("count", SettingKind::Integer)]);

    let refused = unknown_library(
        &schema,
        &settings(&[("count", SettingValue::Integer(7))]),
        &HashSet::new(),
    );

    assert_eq!(refused, None);
}
