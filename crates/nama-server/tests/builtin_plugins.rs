//! The Built-in Plugins: which ship in the Server, how config turns them on and off at startup,
//! and what they may depend on.

use std::{collections::BTreeMap, path::Path, process::Command, sync::Arc};

use figment::{
    Figment,
    providers::{Format, Serialized, Toml},
};
use nama_plugin_api::{
    Plugin, PluginId, Registrar, Registration,
    auth::Authenticator,
    settings::{SettingValue, Settings},
};
use nama_plugin_local_auth::LocalAuth;
use nama_plugin_log_events::LogEvents;
use nama_plugin_test_choices::TestChoices;
use nama_plugin_tmdb::Tmdb;
use nama_server::{
    config::Config, db::Database, plugin_settings::PluginSettings, plugins::Registry,
};

/// The Built-in Plugins started as a Server starts them, with the `[plugins.*]` tables in `toml`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn started(toml: &str) -> Vec<PluginId> {
    let data_dir = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data_dir.path()).await.unwrap());
    let config: Config = Figment::from(Serialized::defaults(Config::default()))
        .merge(Toml::string(toml))
        .merge(Serialized::default("data_dir", data_dir.path()))
        .extract()
        .unwrap();
    let plugins = PluginSettings::start(Arc::clone(&db), Registry::with_builtins(), &config)
        .await
        .unwrap();
    plugins.registry().enabled()
}

#[tokio::test]
async fn every_builtin_plugin_is_enabled_by_default_once_it_has_the_settings_it_requires() {
    let without_settings = started("").await;
    let with_settings = started("[plugins.tmdb]\napi_key = \"secret\"").await;

    // TMDB can't run without an API key.
    assert_eq!(
        without_settings,
        [
            PluginId::from_static("local-auth"),
            PluginId::from_static("log-events")
        ]
    );
    assert_eq!(
        with_settings,
        [
            PluginId::from_static("local-auth"),
            PluginId::from_static("log-events"),
            PluginId::from_static("tmdb")
        ]
    );
}

#[tokio::test]
async fn config_can_disable_a_builtin_plugin() {
    let enabled = started("[plugins.log-events]\nenabled = false").await;

    assert_eq!(enabled, [PluginId::from_static("local-auth")]);
}

#[tokio::test]
async fn config_for_a_plugin_the_server_does_not_have_is_ignored() {
    let enabled = started("[plugins.no-such-plugin]").await;

    assert_eq!(
        enabled,
        [
            PluginId::from_static("local-auth"),
            PluginId::from_static("log-events")
        ]
    );
}

#[test]
fn a_plugin_that_isnt_running_may_be_disabled_only_while_another_provides_an_authenticator() {
    let without_local_auth = Registry::with_builtins();
    let with_local_auth = Registry::new();
    with_local_auth.enable(Arc::new(LocalAuth)).unwrap();

    // As after `local-auth` panicked: it may be what would provide one after a restart.
    let refused =
        without_local_auth.last_provider::<dyn Authenticator>(&PluginId::from_static("local-auth"));
    let allowed =
        with_local_auth.last_provider::<dyn Authenticator>(&PluginId::from_static("tmdb"));

    assert!(refused.is_some(), "{refused:?}");
    assert!(allowed.is_none(), "{allowed:?}");
}

#[test]
fn each_builtin_plugin_registers_at_exactly_the_extension_points_its_manifest_names() {
    let plugins: [Arc<dyn Plugin>; 4] = [
        Arc::new(LocalAuth),
        Arc::new(LogEvents),
        Arc::new(TestChoices),
        Arc::new(Tmdb::new()),
    ];
    // So a Built-in Plugin added later can't miss this test.
    let builtins: Vec<_> = Registry::with_builtins()
        .plugins()
        .into_iter()
        .map(|plugin| plugin.id)
        .collect();
    let ids: Vec<_> = plugins.iter().map(|plugin| plugin.manifest().id).collect();
    assert_eq!(ids, builtins);

    for plugin in plugins {
        let manifest = plugin.manifest();
        let api_key = BTreeMap::from([(
            "api_key".to_owned(),
            SettingValue::Text(nama_plugin_test_choices::API_KEY.to_owned()),
        )]);
        let mut registrar = Registrar::with_settings(Settings::new(api_key));
        plugin.register(&mut registrar);

        let registered: Vec<_> = registrar
            .into_registrations()
            .iter()
            .map(Registration::point)
            .collect();
        assert_eq!(registered, manifest.extension_points, "{}", manifest.id);
    }
}

/// ADR 0006: the workspace crates a crate in `plugins/` may depend on, as `plugins/AGENTS.md` says.
#[test]
fn plugin_crates_depend_only_on_the_workspace_crates_they_may() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new(env!("CARGO"))
        .args(["metadata", "--format-version=1", "--no-deps", "--offline"])
        .current_dir(&workspace)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    // Build the plugins directory from Cargo's own `workspace_root` so it has the same form as
    // every `manifest_path`. `canonicalize` would give a `\\?\` verbatim path on Windows, which
    // never prefixes Cargo's plain `D:\...` paths.
    let plugins_dir = Path::new(metadata["workspace_root"].as_str().unwrap()).join("plugins");

    let in_plugins: Vec<_> = metadata["packages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|package| {
            Path::new(package["manifest_path"].as_str().unwrap()).starts_with(&plugins_dir)
        })
        .collect();
    let shared = |package: &serde_json::Value| {
        !package["name"]
            .as_str()
            .unwrap()
            .starts_with("nama-plugin-")
    };
    let mut pdk_and_shared = vec!["nama-pdk"];
    pdk_and_shared.extend(
        in_plugins
            .iter()
            .filter(|package| shared(package))
            .map(|package| package["name"].as_str().unwrap()),
    );

    for package in &in_plugins {
        let installed = package["targets"].as_array().unwrap().iter().any(|target| {
            target["crate_types"]
                .as_array()
                .unwrap()
                .contains(&"cdylib".into())
        });
        assert!(
            !(installed && shared(package)),
            "{} is an Installed Plugin, so its name starts with nama-plugin-",
            package["name"]
        );
        let installed_or_shared = installed || shared(package);
        let allowed: &[&str] = if installed_or_shared {
            &pdk_and_shared
        } else {
            &["nama-plugin-api", "nama-proto"]
        };
        for dependency in package["dependencies"].as_array().unwrap() {
            let name = dependency["name"].as_str().unwrap();
            let in_workspace = dependency.get("path").is_some_and(|path| !path.is_null());
            // A crate may depend on itself, to turn on its test helpers in its own tests.
            let itself = package["name"].as_str() == Some(name);
            assert!(
                !in_workspace || itself || allowed.contains(&name),
                "{} depends on {name}",
                package["name"]
            );
        }
    }
    assert!(
        !in_plugins.is_empty(),
        "found no plugin crates in {}",
        plugins_dir.display()
    );
}
