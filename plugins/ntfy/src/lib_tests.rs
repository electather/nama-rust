use nama_pdk::nama_plugin_proto::nama::plugin::v1::{
    PersonalSettingsCheck, SettingKind, setting_value::Value,
};

use super::{TOPIC, check_personal_settings, manifest, topic};

#[test]
fn a_topic_of_letters_digits_underscores_and_dashes_up_to_64_long_is_taken() {
    let longest = "a".repeat(64);

    for taken in ["a", "nama_ada-7Xq", longest.as_str()] {
        assert_eq!(topic(Some(taken)).ok(), Some(taken));
    }
}

#[test]
fn an_empty_too_long_or_oddly_lettered_topic_is_refused_naming_the_setting() {
    let too_long = "a".repeat(65);

    for refused in [
        "",
        too_long.as_str(),
        "with space",
        "a/b",
        "ünïcode",
        "dot.ted",
    ] {
        let error = topic(Some(refused)).map(drop).unwrap_err().to_string();
        assert!(
            error.contains("Personal setting topic"),
            "{refused}: {error}"
        );
    }
}

fn asked(topic: &str) -> PersonalSettingsCheck {
    PersonalSettingsCheck {
        personal_settings: std::iter::once((TOPIC.to_owned(), topic.to_owned())).collect(),
        ..PersonalSettingsCheck::default()
    }
}

#[test]
fn the_personal_settings_check_passes_a_good_topic_and_refuses_a_bad_one_in_its_words() {
    let passed = check_personal_settings(&asked("nama_ada-7Xq"));
    let refused = check_personal_settings(&asked("bad topic/../!"));

    assert!(passed.is_ok(), "{passed:?}");
    assert_eq!(
        refused.unwrap_err().to_string(),
        "the Personal setting topic must be 1 to 64 letters, digits, _ or -"
    );
}

#[test]
fn a_missing_topic_is_refused_naming_the_setting() {
    let error = topic(None).map(drop).unwrap_err().to_string();

    assert!(error.contains("Personal setting topic"), "{error}");
}

#[test]
fn manifest_declares_the_ntfy_server_defaulting_to_ntfy_sh() {
    let settings = manifest().settings;
    let settings: Vec<_> = settings
        .iter()
        .map(|field| {
            let default = field.default_value.as_option();
            (
                field.key.as_str(),
                field.kind.as_known(),
                default.and_then(|default| default.value.clone()),
            )
        })
        .collect();

    assert_eq!(
        settings,
        [(
            "server_url",
            Some(SettingKind::SETTING_KIND_URL),
            Some(Value::TextValue("https://ntfy.sh".to_owned())),
        )]
    );
}
