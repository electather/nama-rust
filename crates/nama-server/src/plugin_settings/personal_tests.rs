use std::{collections::BTreeMap, sync::Arc};

use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, Registrar,
    settings::{
        PersonalSettingsCheck, SettingField, SettingKind, SettingValue, Settings, SettingsError,
        SettingsSchema,
    },
};
use nama_plugin_local_auth::LocalAuth;
use tempfile::TempDir;

use crate::{
    config::Config,
    db::Database,
    plugin_settings::{ChangeError, PluginSettings},
    plugins::{CheckError, Registry},
    users::personal_settings_recipients,
};

const ID: PluginId = PluginId::from_static("pager");
const PLAIN: PluginId = PluginId::from_static("plain");

/// A Plugin declaring a required `handle`, a secret `token`, a `loud` bool and a `retries`
/// number as Personal settings, whose check refuses the handle `taken` and fails quoting the
/// token for the handle `echo`.
struct Pager;

fn field(key: &str, kind: SettingKind, required: bool) -> SettingField {
    SettingField {
        key: key.to_owned(),
        label: key.to_owned(),
        description: String::new(),
        kind,
        required,
        default: None,
        min: None,
        max: None,
        choices: Vec::new(),
    }
}

impl Plugin for Pager {
    fn manifest(&self) -> Manifest {
        Manifest {
            version: "0.0.0".to_owned(),
            personal_settings: SettingsSchema {
                fields: vec![
                    field("handle", SettingKind::Text, true),
                    field("token", SettingKind::Secret, false),
                    field("loud", SettingKind::Bool, false),
                    field("retries", SettingKind::Integer, false),
                ],
            },
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

impl PersonalSettingsCheck for Pager {
    fn check<'a>(
        &'a self,
        _settings: &'a Settings,
        personal: &'a BTreeMap<String, String>,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        let value = |key: &str| personal.get(key).map_or("", String::as_str);
        let checked = match value("handle") {
            "taken" => Err("the handle taken is someone else's".into()),
            "echo" => Err(format!("the token {} doesn't open echo", value("token")).into()),
            _ => Ok(()),
        };
        Box::pin(async move { checked })
    }
}

/// A Plugin that declares no Personal settings.
struct Plain;

impl Plugin for Plain {
    fn manifest(&self) -> Manifest {
        Manifest {
            version: "0.0.0".to_owned(),
            ..Manifest::new(PLAIN)
        }
    }

    fn register(&self, _registrar: &mut Registrar) -> Result<(), PluginError> {
        Ok(())
    }
}

/// Plugin settings with `Pager` and `Plain` enabled, and a User's id.
async fn started() -> (TempDir, PluginSettings, i64) {
    let data_dir = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data_dir.path()).await.unwrap());
    let registry = Registry::new();
    registry.describe_builtin(|| Arc::new(LocalAuth));
    registry.describe_builtin(|| Arc::new(Pager));
    registry.describe_builtin(|| Arc::new(Plain));
    let config = Config {
        data_dir: data_dir.path().to_owned(),
        ..Config::default()
    };
    let plugins = PluginSettings::start(Arc::clone(&db), registry, &config)
        .await
        .unwrap();
    let user = insert_user(&db).await;
    (data_dir, plugins, user)
}

async fn insert_user(db: &Database) -> i64 {
    db.write(|tx| {
        Box::pin(async move {
            sqlx::query_scalar(
                "INSERT INTO users (username, display_username, permissions, created_at,
                                    updated_at)
                 VALUES ('ada', 'ada', 0, 0, 0) RETURNING id",
            )
            .fetch_one(&mut **tx)
            .await
        })
    })
    .await
    .unwrap()
}

fn text(value: &str) -> SettingValue {
    SettingValue::Text(value.to_owned())
}

fn sent(values: &[(&str, SettingValue)]) -> BTreeMap<String, SettingValue> {
    values
        .iter()
        .map(|(key, value)| ((*key).to_owned(), value.clone()))
        .collect()
}

/// What a Notifier is handed for the User of `plugins`' Personal settings for `Pager`.
async fn handed(plugins: &PluginSettings) -> BTreeMap<String, String> {
    let recipients = personal_settings_recipients(&plugins.db, &ID)
        .await
        .unwrap();
    let Some((_, values, _)) = recipients.into_iter().next() else {
        return BTreeMap::new();
    };
    values
}

/// `Pager`'s values for `user` as `plugins` lists them.
async fn listed(plugins: &PluginSettings, user: i64) -> BTreeMap<String, SettingValue> {
    let listed = plugins.list_personal(user).await.unwrap();
    let ids: Vec<_> = listed.iter().map(|it| it.plugin.id.clone()).collect();
    assert_eq!(ids, [ID], "only Pager declares Personal settings");
    listed.into_iter().next().unwrap().values
}

#[tokio::test]
async fn saved_values_read_back_as_their_kinds_and_reach_a_notifier_as_text() {
    let (_dir, plugins, user) = started().await;

    let saved = plugins
        .save_personal(
            user,
            &ID,
            sent(&[
                ("handle", text("ada")),
                ("loud", SettingValue::Bool(true)),
                ("retries", SettingValue::Integer(3)),
            ]),
        )
        .await
        .unwrap();

    let expected = sent(&[
        ("handle", text("ada")),
        ("loud", SettingValue::Bool(true)),
        ("retries", SettingValue::Integer(3)),
    ]);
    assert_eq!(saved.values, expected);
    assert_eq!(listed(&plugins, user).await, expected);
    assert_eq!(
        handed(&plugins).await,
        BTreeMap::from([
            ("handle".to_owned(), "ada".to_owned()),
            ("loud".to_owned(), "true".to_owned()),
            ("retries".to_owned(), "3".to_owned()),
        ])
    );
}

#[tokio::test]
async fn a_secret_left_out_keeps_its_stored_value_and_an_unknown_key_is_dropped() {
    let (_dir, plugins, user) = started().await;
    plugins
        .save_personal(
            user,
            &ID,
            sent(&[("handle", text("ada")), ("token", text("tk_secret"))]),
        )
        .await
        .unwrap();

    plugins
        .save_personal(
            user,
            &ID,
            sent(&[("handle", text("ada2")), ("unknown", text("x"))]),
        )
        .await
        .unwrap();

    assert_eq!(
        listed(&plugins, user).await,
        sent(&[("handle", text("ada2")), ("token", text("tk_secret"))])
    );
    assert_eq!(
        handed(&plugins).await,
        BTreeMap::from([
            ("handle".to_owned(), "ada2".to_owned()),
            ("token".to_owned(), "tk_secret".to_owned()),
        ])
    );
}

#[tokio::test]
async fn a_required_value_left_out_is_refused_naming_it() {
    let (_dir, plugins, user) = started().await;

    // A form's blank field arrives left out: the handler drops empty text.
    let refused = plugins
        .save_personal(user, &ID, sent(&[("token", text("tk"))]))
        .await;

    assert!(
        matches!(
            refused,
            Err(ChangeError::InvalidSettings(SettingsError::Missing(ref key))) if key == "handle"
        ),
        "{refused:?}"
    );
}

#[tokio::test]
async fn a_failed_check_keeps_the_earlier_values_and_blanks_the_secret_from_its_reason() {
    let (_dir, plugins, user) = started().await;
    let earlier = sent(&[("handle", text("ada")), ("token", text("tk_secret"))]);
    plugins
        .save_personal(user, &ID, earlier.clone())
        .await
        .unwrap();

    let taken = plugins
        .save_personal(user, &ID, sent(&[("handle", text("taken"))]))
        .await;
    let echoed = plugins
        .save_personal(user, &ID, sent(&[("handle", text("echo"))]))
        .await;

    let failed = |checked: Result<_, ChangeError>| match checked {
        Err(ChangeError::Check {
            plugin,
            error: CheckError::Failed(message),
            setting: None,
        }) if plugin == ID => message,
        other => panic!("expected the check to fail, got {other:?}"),
    };
    assert_eq!(failed(taken), "the handle taken is someone else's");
    assert_eq!(failed(echoed), "the token … doesn't open echo");
    assert_eq!(listed(&plugins, user).await, earlier);
}

#[tokio::test]
async fn a_disabled_unknown_or_non_declaring_plugin_is_refused_as_not_found() {
    let (_dir, plugins, user) = started().await;
    plugins.disable(&ID).await.unwrap();
    let handle = || sent(&[("handle", text("ada"))]);

    for id in [ID, PLAIN, PluginId::from_static("no-such-plugin")] {
        let refused = plugins.save_personal(user, &id, handle()).await;

        assert!(
            matches!(refused, Err(ChangeError::NoPersonalSettings(ref named)) if *named == id),
            "{id}: {refused:?}"
        );
    }
    assert!(plugins.list_personal(user).await.unwrap().is_empty());
}

#[tokio::test]
async fn deleting_forgets_the_values() {
    let (_dir, plugins, user) = started().await;
    plugins
        .save_personal(user, &ID, sent(&[("handle", text("ada"))]))
        .await
        .unwrap();

    plugins.delete_personal(user, &ID).await.unwrap();

    assert_eq!(listed(&plugins, user).await, BTreeMap::new());
    assert_eq!(handed(&plugins).await, BTreeMap::new());
}
