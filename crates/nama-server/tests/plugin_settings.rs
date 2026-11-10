//! Starting the Server's Plugins with the settings stored in the database, which come from the
//! config's `[plugins.*]` tables once, on the first start, and changing them in the order
//! ADR 0028 sets, against a real database and fake Plugins, without HTTP.

#[path = "support/test_plugin.rs"]
mod test_plugin;

#[path = "support/log.rs"]
#[expect(dead_code, reason = "the harness's `wait_for` has no use here")]
mod log;

use std::{
    collections::{BTreeMap, HashMap},
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};

use figment::{
    Figment,
    providers::{Format, Serialized, Toml},
};
use log::Log;
use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    fulfiller::{Download, Fulfilled, Fulfiller, Movies},
    settings::{SettingField, SettingKind, SettingValue, SettingsSchema},
};
use nama_plugin_local_auth::LocalAuth;
use nama_proto::nama::v1::Request;
use nama_server::{
    config::{Config, PluginConfig},
    db::Database,
    plugin_settings::{self, ChangeError, PluginSettings, StartError},
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

/// A Built-in Movie Fulfiller that takes no Request.
struct BuiltinFulfiller;

impl Plugin for BuiltinFulfiller {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: PluginId::from_static("zz-fulfiller"),
            name: String::new(),
            description: String::new(),
            version: "0.0.0".to_owned(),
            settings: SettingsSchema::default(),
            extension_points: vec![PointKey::provide::<dyn Fulfiller<Movies>>()],
            logo_svg: None,
        }
    }

    fn register(&self, registrar: &mut Registrar) {
        registrar.provide::<dyn Fulfiller<Movies>>(Priority::DEFAULT, Arc::new(Self));
    }
}

impl Fulfiller<Movies> for BuiltinFulfiller {
    fn fulfill<'a>(&'a self, _: &'a Request) -> BoxFuture<'a, Result<Fulfilled, PluginError>> {
        Box::pin(async { Err("takes no Request".into()) })
    }

    fn progress<'a>(
        &'a self,
        _: &'a [String],
    ) -> BoxFuture<'a, Result<HashMap<String, Vec<Download>>, PluginError>> {
        Box::pin(async { Ok(HashMap::new()) })
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
async fn builtin_plugins_enable_before_installed_ones_so_a_builtin_fulfiller_keeps_its_kind() {
    let (data_dir, db) = open().await;
    let registry = registry();
    // Its id sorts after `radarr`'s, so only enabling Built-in Plugins first lets it win.
    registry.describe_builtin(|| Arc::new(BuiltinFulfiller));
    let config = config(
        data_dir.path(),
        r#"
        [plugins.radarr]
        url = "http://localhost:7878"
        api_key = "key"
        quality_profile = "1"
        root_folder = "/movies"
        "#,
    );

    let plugins = PluginSettings::start(db, registry, &config).await.unwrap();

    let enabled = plugins.registry().enabled();
    assert!(
        enabled.contains(&PluginId::from_static("zz-fulfiller")),
        "{enabled:?}"
    );
    assert!(
        !enabled.contains(&PluginId::from_static("radarr")),
        "{enabled:?}"
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

/// Starts the Plugins of [`registry`] with `never-enables` beside them, and the config's
/// `[plugins.*]` tables in `toml`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn with_one_that_never_enables(toml: &str) -> (TempDir, PluginSettings) {
    let (data_dir, db) = open().await;
    let registry = registry();
    registry.describe_builtin(|| {
        Arc::new(Fake {
            id: "never-enables",
            panics: true,
        })
    });
    let config = config(data_dir.path(), toml);
    let plugins = PluginSettings::start(db, registry, &config).await.unwrap();
    (data_dir, plugins)
}

fn never_enables() -> PluginId {
    PluginId::from_static("never-enables")
}

/// Whether the Plugin `id` is stored as enabled.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn stored_enabled(plugins: &PluginSettings, id: &PluginId) -> bool {
    plugins.get(id).await.unwrap().unwrap().stored.enabled
}

#[tokio::test]
async fn a_refused_enable_saves_nothing() {
    let (_data_dir, plugins) =
        with_one_that_never_enables("[plugins.never-enables]\nenabled = false").await;

    let refused = plugins.enable(&never_enables()).await;

    assert!(
        matches!(refused, Err(ChangeError::Enable(_))),
        "{refused:?}"
    );
    assert!(!stored_enabled(&plugins, &never_enables()).await);
    assert!(!plugins.registry().enabled().contains(&never_enables()));
}

#[tokio::test]
async fn a_settings_change_that_fails_to_enable_still_saves_them_and_keeps_it_stored_as_enabled() {
    let (_data_dir, plugins) = with_one_that_never_enables("").await;
    let hello = BTreeMap::from([(
        "greeting".to_owned(),
        SettingValue::Text("hello".to_owned()),
    )]);

    let updated = plugins
        .update(&never_enables(), hello.clone())
        .await
        .unwrap();

    assert!(!updated.plugin.enabled);
    let stored = plugins.get(&never_enables()).await.unwrap().unwrap().stored;
    assert_eq!(
        stored,
        PluginConfig {
            enabled: true,
            settings: hello
        }
    );
    assert!(!plugins.registry().enabled().contains(&never_enables()));
}

#[tokio::test]
async fn disabling_the_last_authenticator_is_refused_and_saves_nothing() {
    let (data_dir, db) = open().await;
    let plugins = PluginSettings::start(db, registry(), &config(data_dir.path(), ""))
        .await
        .unwrap();
    let local_auth = PluginId::from_static("local-auth");

    let refused = plugins.disable(&local_auth).await;

    assert!(
        matches!(refused, Err(ChangeError::LastAuthenticator(_))),
        "{refused:?}"
    );
    assert!(stored_enabled(&plugins, &local_auth).await);
    assert!(plugins.registry().enabled().contains(&local_auth));
}

#[tokio::test]
async fn a_disable_is_saved_and_applied() {
    let (data_dir, db) = open().await;
    let plugins = PluginSettings::start(db, registry(), &config(data_dir.path(), ""))
        .await
        .unwrap();

    plugins.disable(&greeter()).await.unwrap();

    assert!(!stored_enabled(&plugins, &greeter()).await);
    assert!(!plugins.registry().enabled().contains(&greeter()));
}

// Several workers, so the changes really run at once and race between saving and applying.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn changes_at_once_leave_the_stored_state_as_what_is_running() {
    let (data_dir, db) = open().await;
    let plugins = PluginSettings::start(db, registry(), &config(data_dir.path(), ""))
        .await
        .unwrap();
    let hello = BTreeMap::from([(
        "greeting".to_owned(),
        SettingValue::Text("hello".to_owned()),
    )]);
    let greeter = greeter();

    for _ in 0..50 {
        let (disabled, enabled, updated) = tokio::join!(
            plugins.disable(&greeter),
            plugins.enable(&greeter),
            plugins.update(&greeter, hello.clone()),
        );
        disabled.unwrap();
        enabled.unwrap();
        updated.unwrap();

        let running = plugins.registry().enabled().contains(&greeter);
        assert_eq!(stored_enabled(&plugins, &greeter).await, running);
    }
}

/// Calls the enabled Movie Fulfiller until the test Plugin's, set to panic, has crashed enough
/// times in a row to be disabled.
async fn crash_test_plugin(plugins: &PluginSettings) {
    let test_plugin = PluginId::from_static("test-plugin");
    while plugins.registry().enabled().contains(&test_plugin) {
        plugins
            .registry()
            .call_provider(|fulfiller: Arc<dyn Fulfiller<Movies>>| async move {
                let _crashed: Result<Fulfilled, PluginError> =
                    fulfiller.fulfill(&Request::default()).await;
            })
            .await;
    }
}

#[tokio::test]
async fn a_crashed_plugin_is_saved_as_disabled_unless_an_admin_enabled_it_again() {
    let (data_dir, db) = open().await;
    let dir = data_dir.path().join("plugins");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("test.wasm"), test_plugin::build("")).unwrap();
    let config = config(
        data_dir.path(),
        "[plugins.test-plugin]\nfulfiller = \"panic\"",
    );
    let plugins = PluginSettings::start(db, registry(), &config)
        .await
        .unwrap();
    let test_plugin = PluginId::from_static("test-plugin");

    crash_test_plugin(&plugins).await;
    let crashed_at = Instant::now();
    while stored_enabled(&plugins, &test_plugin).await {
        assert!(
            crashed_at.elapsed() < Duration::from_secs(10),
            "its disable was never saved"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    plugins.enable(&test_plugin).await.unwrap();
    crash_test_plugin(&plugins).await;
    plugins.enable(&test_plugin).await.unwrap();
    // Listing takes the lock after the crash's save, which waited for the enable.
    plugins.list().await.unwrap();

    assert!(plugins.registry().enabled().contains(&test_plugin));
    assert!(stored_enabled(&plugins, &test_plugin).await);
}
