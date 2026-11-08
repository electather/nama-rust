//! A Built-in Plugin used only in tests (the Server's `testing` feature): its settings check offers
//! choices, and its settings can tell it to fail.

use std::future;

use nama_plugin_api::{
    BoxFuture, Plugin, PluginError, PluginId, Registrar,
    settings::{
        Choice, Choices, SettingField, SettingKind, Settings, SettingsCheck, SettingsSchema,
    },
};

/// The test Plugin's id.
pub const ID: &str = "test-choices";

/// The only API key its check accepts.
pub const API_KEY: &str = "test-choices-key-that-must-stay-on-the-server";

/// What `fail_with` is set to for the check to panic.
pub const PANIC: &str = "panic";

/// What `fail_with` is set to for the check never to finish.
pub const HANG: &str = "hang";

/// The test Plugin. It registers nothing.
#[derive(Debug)]
pub struct TestChoices;

impl Plugin for TestChoices {
    fn id(&self) -> PluginId {
        PluginId::from_static(ID)
    }

    fn version(&self) -> &'static str {
        "0.0.0"
    }

    fn settings_schema(&self) -> SettingsSchema {
        let field = |key, label, description, kind, required| SettingField {
            key,
            label,
            description,
            kind,
            required,
            default: None,
        };
        SettingsSchema {
            fields: vec![
                field(
                    "api_key",
                    "API key",
                    "The key the check accepts.",
                    SettingKind::Secret,
                    true,
                ),
                field(
                    "fail_with",
                    "Fail with",
                    "Makes the check fail with this error, or panic or hang for those words.",
                    SettingKind::Text,
                    false,
                ),
                field(
                    "profile",
                    "Profile",
                    "One of the profiles the check offers.",
                    SettingKind::Choice,
                    true,
                ),
            ],
        }
    }

    fn settings_check(&self) -> Option<&dyn SettingsCheck> {
        Some(self)
    }

    fn register(&self, _registrar: &mut Registrar) {}
}

impl SettingsCheck for TestChoices {
    fn check<'a>(&'a self, settings: &'a Settings) -> BoxFuture<'a, Result<Choices, PluginError>> {
        #[expect(
            clippy::panic,
            reason = "it panics when told to, to test that the Kernel contains it"
        )]
        Box::pin(async move {
            match settings.text("fail_with") {
                Some(PANIC) => panic!("the test Plugin was told to panic"),
                Some(HANG) => future::pending().await,
                Some(error) => return Err(error.into()),
                None => {}
            }
            if settings.text("api_key") != Some(API_KEY) {
                return Err("the service refused the API key".into());
            }
            let profile = |value: &str, label: &str| Choice {
                value: value.to_owned(),
                label: label.to_owned(),
            };
            Ok(Choices::from([(
                "profile".to_owned(),
                vec![profile("1", "HD"), profile("2", "Ultra HD")],
            )]))
        })
    }
}
