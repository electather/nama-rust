use nama_pdk::nama_plugin_proto::nama::plugin::v1::{SettingKind, setting_value::Value};

use super::{manifest, topic};

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
