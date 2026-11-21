use std::{collections::BTreeMap, sync::Arc};

use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, Registrar,
    settings::{
        Choice, PersonalSettingsCheck, SettingField, SettingKind, SettingValue, Settings,
        SettingsSchema,
    },
};

use nama_proto::nama::v1::PluginKind;
use tracing::info;

use super::{Described, Registry, refuse_flawed};
use crate::{
    log_capture::CapturedLog,
    plugins::{CheckError, EnableError},
};

const ID: PluginId = PluginId::from_static("echoing");

/// A Plugin whose Personal settings check fails quoting its `api_key` and the User's `token`,
/// both secrets.
struct Echoing;

fn secret(key: &str) -> SettingsSchema {
    SettingsSchema {
        fields: vec![SettingField {
            key: key.to_owned(),
            label: key.to_owned(),
            description: String::new(),
            kind: SettingKind::Secret,
            required: false,
            default: None,
            min: None,
            max: None,
            choices: Vec::new(),
        }],
    }
}

impl Plugin for Echoing {
    fn manifest(&self) -> Manifest {
        Manifest {
            version: "0.0.0".to_owned(),
            settings: secret("api_key"),
            personal_settings: secret("token"),
            ..Manifest::new(ID)
        }
    }

    fn personal_settings_check(&self) -> Option<&dyn PersonalSettingsCheck> {
        Some(self)
    }

    fn register(&self, _registrar: &mut Registrar) -> Result<(), PluginError> {
        Ok(())
    }
}

impl PersonalSettingsCheck for Echoing {
    fn check<'a>(
        &'a self,
        settings: &'a Settings,
        personal: &'a BTreeMap<String, String>,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        let api_key = settings.text("api_key").unwrap_or_default();
        let token = personal.get("token").map_or("", String::as_str);
        let message = format!("{api_key} rejects {token}");
        Box::pin(async move { Err(message.into()) })
    }
}

#[tokio::test]
async fn a_personal_settings_check_failure_shows_neither_the_plugins_nor_the_users_secret() {
    let registry = Registry::new();
    registry.describe_builtin(|| Arc::new(Echoing));
    let settings = Settings::new(BTreeMap::from([(
        "api_key".to_owned(),
        SettingValue::Text("key_secret".to_owned()),
    )]));
    let personal = BTreeMap::from([("token".to_owned(), "tk_secret".to_owned())]);

    let checked = registry
        .check_personal_settings(&ID, settings, &personal)
        .await;

    let Err(CheckError::Failed(message)) = checked else {
        panic!("expected the check to fail, got {checked:?}");
    };
    assert_eq!(message, "… rejects …");
}

const LOGGING: PluginId = PluginId::from_static("logging");

/// A Plugin that logs with `tracing` as it registers and is dropped, as a Built-in does.
struct Logging;

impl Plugin for Logging {
    fn manifest(&self) -> Manifest {
        Manifest {
            version: "0.0.0".to_owned(),
            ..Manifest::new(LOGGING)
        }
    }

    fn register(&self, _: &mut Registrar) -> Result<(), PluginError> {
        info!("logging registered");
        Ok(())
    }
}

impl Drop for Logging {
    fn drop(&mut self) {
        info!("logging dropped");
    }
}

/// What enabling [`Logging`] and disabling it log.
async fn logged_by_logging(registry: &Registry) -> String {
    let log = CapturedLog::start();
    registry.enable(Arc::new(Logging)).unwrap();
    assert!(registry.disable(&LOGGING).await);
    log.finish()
}

#[tokio::test(start_paused = true)]
async fn a_builtins_lines_as_it_registers_and_drops_are_in_a_span_naming_it() {
    let logged = logged_by_logging(&Registry::new()).await;

    for what in ["registered", "dropped"] {
        let line = logged
            .lines()
            .find(|line| line.ends_with(&format!("logging {what}")))
            .unwrap_or_else(|| panic!("nothing {what}: {logged}"));
        assert!(line.contains(" plugin{plugin=logging}:"), "{line}");
    }
}

#[tokio::test(start_paused = true)]
async fn an_installed_plugins_lines_get_no_span_naming_it() {
    let registry = Registry::new();
    let as_installed = Described {
        kind: PluginKind::PLUGIN_KIND_INSTALLED,
        plugin: Err("only its kind is read".to_owned()),
        checks_settings: false,
    };
    registry.write(|state| state.known.insert(LOGGING, as_installed));

    let logged = logged_by_logging(&registry).await;

    assert!(logged.contains("logging registered"), "{logged}");
    assert!(!logged.contains(" plugin{"), "{logged}");
}

#[test]
fn a_builtins_drop_after_describing_it_is_in_a_span_naming_it() {
    let log = CapturedLog::start();
    Registry::new().describe_builtin(|| Arc::new(Logging));
    let logged = log.finish();

    let line = logged
        .lines()
        .find(|line| line.ends_with("logging dropped"))
        .unwrap_or_else(|| panic!("nothing dropped: {logged}"));
    assert!(line.contains(" plugin{plugin=logging}:"), "{line}");
}

#[tokio::test]
async fn a_builtins_lines_while_its_settings_are_checked_are_in_a_check_span_naming_it() {
    let registry = Registry::new();
    registry.describe_builtin(|| Arc::new(Logging));
    let log = CapturedLog::start();

    registry
        .check_personal_settings(&LOGGING, Settings::default(), &BTreeMap::new())
        .await
        .unwrap();

    let logged = log.finish();
    let line = logged
        .lines()
        .find(|line| line.ends_with("logging dropped"))
        .unwrap_or_else(|| panic!("nothing dropped: {logged}"));
    assert!(line.contains(" check{plugin=logging}:"), "{line}");
}

fn ratings(choices: &[&str], default: Option<&[&str]>) -> SettingField {
    let texts = |values: &[&str]| values.iter().map(|value| (*value).to_owned()).collect();
    SettingField {
        key: "ratings".to_owned(),
        label: "Ratings".to_owned(),
        kind: SettingKind::Choices,
        choices: choices
            .iter()
            .map(|value| Choice {
                value: (*value).to_owned(),
                label: (*value).to_owned(),
            })
            .collect(),
        default: default.map(|values| SettingValue::Texts(texts(values))),
        ..days(None, None, None)
    }
}

fn days(min: Option<i64>, max: Option<i64>, default: Option<i64>) -> SettingField {
    SettingField {
        key: "days".to_owned(),
        label: "Days".to_owned(),
        description: String::new(),
        kind: SettingKind::Integer,
        required: false,
        default: default.map(SettingValue::Integer),
        min,
        max,
        choices: Vec::new(),
    }
}

/// Why `refuse_flawed` refuses a Built-in whose one Plugin-wide setting is `field`, when the
/// Plugin has a settings check if `offers_choices`.
fn refused(field: SettingField, offers_choices: bool) -> Option<String> {
    let manifest = Manifest {
        settings: SettingsSchema {
            fields: vec![field],
        },
        ..Manifest::new(ID)
    };
    match refuse_flawed(&manifest, offers_choices) {
        Ok(()) => None,
        Err(EnableError::Refused { reason, .. }) => Some(reason),
        Err(other) => panic!("refused as {other:?}"),
    }
}

#[test]
fn a_builtin_with_bounds_on_another_kind_is_refused() {
    let field = SettingField {
        kind: SettingKind::Text,
        ..days(Some(1), None, None)
    };

    assert_eq!(
        refused(field, false).as_deref(),
        Some("its setting days has min or max, which only an Integer setting may")
    );
}

#[test]
fn a_builtin_with_a_min_over_its_max_is_refused() {
    assert_eq!(
        refused(days(Some(7), Some(1), None), false).as_deref(),
        Some("its setting days's min 7 is over its max 1")
    );
}

#[test]
fn a_builtin_with_a_default_outside_its_bounds_is_refused() {
    assert_eq!(
        refused(days(Some(1), Some(7), Some(30)), false).as_deref(),
        Some("its setting days's default is outside its bounds")
    );
}

#[test]
fn a_builtin_with_choices_on_another_kind_is_refused() {
    let field = SettingField {
        kind: SettingKind::Text,
        ..ratings(&["G"], None)
    };

    assert_eq!(
        refused(field, false).as_deref(),
        Some("its setting ratings has choices, which only a Choices setting may")
    );
}

#[test]
fn a_builtin_listing_a_choice_twice_is_refused() {
    assert_eq!(
        refused(ratings(&["G", "G"], None), false).as_deref(),
        Some(r#"its setting ratings lists the choice "G" twice"#)
    );
}

#[test]
fn a_builtin_with_a_default_not_among_its_choices_is_refused() {
    assert_eq!(
        refused(ratings(&["G"], Some(&["R"])), false).as_deref(),
        Some("its setting ratings's default holds a value that isn't among its choices")
    );
}

#[test]
fn a_builtin_with_a_choices_setting_without_choices_and_no_settings_check_is_refused() {
    assert_eq!(
        refused(ratings(&[], None), false).as_deref(),
        Some(
            "its setting ratings is Choices with no choices, and it has no settings check to offer \
             some"
        )
    );
}

#[test]
fn a_builtin_with_a_choices_setting_without_choices_and_a_settings_check_is_not_refused() {
    assert_eq!(refused(ratings(&[], None), true), None);
}

#[test]
fn a_builtin_with_a_flawed_personal_setting_is_refused() {
    let manifest = Manifest {
        personal_settings: SettingsSchema {
            fields: vec![days(Some(7), Some(1), None)],
        },
        ..Manifest::new(ID)
    };

    assert!(matches!(
        refuse_flawed(&manifest, false),
        Err(EnableError::Refused { reason, .. })
            if reason == "its setting days's min 7 is over its max 1"
    ));
}
