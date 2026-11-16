use nama_pdk::nama_plugin_proto::nama::plugin::v1::{SettingKind, setting_value::Value};

use super::{manifest, webhook};

#[test]
fn the_id_and_token_after_api_webhooks_are_taken_whatever_the_host() {
    for (given, taken) in [
        (
            "https://discord.com/api/webhooks/123/tok_EN-9",
            "123/tok_EN-9",
        ),
        ("https://discordapp.com/api/webhooks/123/tok/", "123/tok"),
        (
            "https://ptb.discord.com/api/webhooks/123/tok?wait=true",
            "123/tok",
        ),
        ("http://127.0.0.1:9/api/webhooks/456/other", "456/other"),
    ] {
        assert_eq!(webhook(Some(given)).ok().as_deref(), Some(taken), "{given}");
    }
}

#[test]
fn a_webhook_without_an_id_and_token_is_refused_naming_the_setting_not_its_value() {
    for refused in [
        "",
        "https://discord.com/channels/123/456",
        "https://discord.com/api/webhooks/123",
        "https://discord.com/api/webhooks/123/",
        "https://discord.com/api/webhooks/abc/secret_token",
        "https://discord.com/api/webhooks/123/secret_token/github",
        "https://discord.com/api/webhooks/123/../secret_token",
    ] {
        let error = webhook(Some(refused)).unwrap_err().to_string();
        assert!(
            error.contains("Personal setting webhook"),
            "{refused}: {error}"
        );
        assert!(!error.contains("secret_token"), "{refused}: {error}");
    }
}

#[test]
fn a_missing_webhook_is_refused_naming_the_setting() {
    let error = webhook(None).unwrap_err().to_string();

    assert!(error.contains("Personal setting webhook"), "{error}");
}

#[test]
fn manifest_declares_the_discord_url_defaulting_to_discord_com() {
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
            "api_url",
            Some(SettingKind::SETTING_KIND_URL),
            Some(Value::TextValue("https://discord.com".to_owned())),
        )]
    );
}
