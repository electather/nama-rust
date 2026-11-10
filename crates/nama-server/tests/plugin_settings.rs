//! Starting the Server's Plugins with the settings stored in the database, which come from the
//! config's `[plugins.*]` tables once, on the first start.

#[path = "support/log.rs"]
#[expect(dead_code, reason = "the harness's `wait_for` has no use here")]
mod log;

use std::{collections::BTreeMap, path::Path, sync::Arc};

use figment::{
    Figment,
    providers::{Format, Serialized, Toml},
};
use log::Log;
use nama_plugin_api::{
    Manifest, Plugin, PluginId, Registrar,
    settings::{SettingField, SettingKind, SettingValue, SettingsSchema},
};
use nama_plugin_local_auth::LocalAuth;
use nama_server::{
    config::{Config, PluginConfig},
    db::Database,
    plugin_settings::{self, PluginSettings, StartError},
    plugins::Registry,
};
use tempfile::TempDir;

/// A Plugin that registers nothing, with an optional `greeting` setting, and panics in `register`
/// if it `panics`.
struct Fake {
    id: &'static str,
    panics: bool,
}

impl Plugin for Fake {
    fn manifest(&self) -> Manifest {
        let greeting = SettingField {
            key: "greeting".to_owned(),
            label: "Greeting".to_owned(),
            description: String::new(),
            kind: SettingKind::Text,
            required: false,
            default: None,
        };
        Manifest {
            id: PluginId::from_static(self.id),
            name: String::new(),
            description: String::new(),
            version: "0.0.0".to_owned(),
            settings: SettingsSchema {
                fields: vec![greeting],
            },
            extension_points: Vec::new(),
            logo_svg: None,
        }
    }

    fn register(&self, _registrar: &mut Registrar) {
        assert!(!self.panics, "register bug");
    }
}

/// A registry that knows `local-auth` and the Fake Built-in Plugin `greeter`.
fn registry() -> Registry {
    let registry = Registry::new();
    registry.describe_builtin(|| Arc::new(LocalAuth));
    registry.describe_builtin(|| {
        Arc::new(Fake {
            id: "greeter",
            panics: false,
        })
    });
    registry
}

/// A config in `data_dir` with the `[plugins.*]` tables in `toml`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn config(data_dir: &Path, toml: &str) -> Config {
    Figment::from(Serialized::defaults(Config::default()))
        .merge(Toml::string(toml))
        .merge(Serialized::default("data_dir", data_dir))
        .extract()
        .unwrap()
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn open() -> (TempDir, Arc<Database>) {
    let data_dir = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data_dir.path()).await.unwrap());
    (data_dir, db)
}

fn greeter() -> PluginId {
    PluginId::from_static("greeter")
}

#[tokio::test]
async fn the_first_start_imports_the_configs_tables_and_one_without_enabled_imports_enabled() {
    let (data_dir, db) = open().await;
    let config = config(
        data_dir.path(),
        r#"
        [plugins.greeter]
        greeting = "hello"

        [plugins.log-events]
        enabled = false
        "#,
    );

    let plugins = PluginSettings::start(Arc::clone(&db), registry(), &config)
        .await
        .unwrap();

    let greeting = BTreeMap::from([(
        "greeting".to_owned(),
        SettingValue::Text("hello".to_owned()),
    )]);
    let imported = BTreeMap::from([
        (
            "greeter".to_owned(),
            PluginConfig {
                enabled: true,
                settings: greeting,
            },
        ),
        (
            "log-events".to_owned(),
            PluginConfig {
                enabled: false,
                settings: BTreeMap::new(),
            },
        ),
    ]);
    assert_eq!(plugin_settings::stored(&db).await.unwrap(), Some(imported));
    assert!(plugins.registry().enabled().contains(&greeter()));
}

#[tokio::test]
async fn the_first_start_without_tables_still_records_the_import() {
    let (data_dir, db) = open().await;

    PluginSettings::start(Arc::clone(&db), registry(), &config(data_dir.path(), ""))
        .await
        .unwrap();

    assert_eq!(
        plugin_settings::stored(&db).await.unwrap(),
        Some(BTreeMap::new())
    );
}

#[tokio::test]
async fn the_stored_settings_win_over_the_configs_tables_once_imported() {
    let (data_dir, db) = open().await;
    let disabled = config(data_dir.path(), "[plugins.greeter]\nenabled = false");
    PluginSettings::start(Arc::clone(&db), registry(), &disabled)
        .await
        .unwrap();

    let enabled = config(data_dir.path(), "[plugins.greeter]\nenabled = true");
    let restarted = PluginSettings::start(Arc::clone(&db), registry(), &enabled)
        .await
        .unwrap();

    assert!(!restarted.registry().enabled().contains(&greeter()));
    assert!(!plugin_settings::stored(&db).await.unwrap().unwrap()["greeter"].enabled);
}

#[tokio::test]
async fn with_nothing_stored_a_builtin_plugin_starts_enabled_and_an_installed_one_disabled() {
    let (data_dir, db) = open().await;

    let plugins = PluginSettings::start(Arc::clone(&db), registry(), &config(data_dir.path(), ""))
        .await
        .unwrap();

    // Radarr is an Installed Plugin the Server bundles.
    let radarr = PluginId::from_static("radarr");
    assert_eq!(
        plugins.registry().enabled(),
        [greeter(), PluginId::from_static("local-auth")]
    );
    assert!(
        plugins
            .get(&greeter())
            .await
            .unwrap()
            .unwrap()
            .stored
            .enabled
    );
    assert!(!plugins.get(&radarr).await.unwrap().unwrap().stored.enabled);
    let listed: Vec<_> = plugins
        .list()
        .await
        .unwrap()
        .into_iter()
        .map(|got| (got.plugin.id, got.stored.enabled))
        .collect();
    assert!(listed.contains(&(greeter(), true)), "{listed:?}");
    assert!(listed.contains(&(radarr, false)), "{listed:?}");
}

#[tokio::test]
async fn a_plugin_that_fails_to_enable_is_logged_and_left_off_while_the_rest_start() {
    let log = Log::global();
    let (data_dir, db) = open().await;
    let registry = registry();
    registry.describe_builtin(|| {
        Arc::new(Fake {
            id: "breaks-on-start",
            panics: true,
        })
    });

    let plugins = PluginSettings::start(db, registry, &config(data_dir.path(), ""))
        .await
        .unwrap();

    assert_eq!(
        plugins.registry().enabled(),
        [greeter(), PluginId::from_static("local-auth")]
    );
    assert_eq!(
        log.count(&["breaks-on-start", "panicked while being enabled"]),
        1,
        "{}",
        log.contents()
    );
}

#[tokio::test]
async fn without_an_authenticator_the_plugins_refuse_to_start_and_import_nothing() {
    let (data_dir, db) = open().await;
    let config = config(data_dir.path(), "[plugins.local-auth]\nenabled = false");

    let refused = PluginSettings::start(Arc::clone(&db), registry(), &config)
        .await
        .err();

    assert!(
        matches!(refused, Some(StartError::NoAuthenticator)),
        "{refused:?}"
    );
    assert_eq!(plugin_settings::stored(&db).await.unwrap(), None);
}
