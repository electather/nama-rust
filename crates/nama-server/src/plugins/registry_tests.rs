use std::{collections::BTreeMap, sync::Arc};

use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, Registrar,
    settings::{
        PersonalSettingsCheck, SettingField, SettingKind, SettingValue, Settings, SettingsSchema,
    },
};

use super::Registry;
use crate::plugins::CheckError;

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
