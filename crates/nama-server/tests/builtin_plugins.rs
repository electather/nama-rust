//! The Built-in Plugins: which ship in the Server, how config turns them on and off, and what
//! they may depend on.

use std::{collections::BTreeMap, path::Path, process::Command};

use nama_plugin_api::{PluginId, auth::Authenticator, settings::SettingValue};
use nama_server::{config::PluginConfig, plugins::Registry};

#[test]
fn every_builtin_plugin_is_enabled_by_default_once_it_has_the_settings_it_requires() {
    let without_settings = Registry::with_builtins(&BTreeMap::new());
    let tmdb = PluginConfig {
        settings: BTreeMap::from([(
            "api_key".to_owned(),
            SettingValue::Text("secret".to_owned()),
        )]),
        ..PluginConfig::default()
    };
    let with_settings = Registry::with_builtins(&BTreeMap::from([("tmdb".to_owned(), tmdb)]));

    // TMDB can't run without an API key.
    assert_eq!(
        without_settings.enabled(),
        [
            PluginId::from_static("local-auth"),
            PluginId::from_static("log-events")
        ]
    );
    assert_eq!(
        with_settings.enabled(),
        [
            PluginId::from_static("local-auth"),
            PluginId::from_static("log-events"),
            PluginId::from_static("tmdb")
        ]
    );
}

#[test]
fn config_can_disable_a_builtin_plugin() {
    let plugins = BTreeMap::from([(
        "log-events".to_owned(),
        PluginConfig {
            enabled: false,
            ..PluginConfig::default()
        },
    )]);

    let registry = Registry::with_builtins(&plugins);

    assert_eq!(registry.enabled(), [PluginId::from_static("local-auth")]);
}

#[test]
fn config_for_a_plugin_the_server_does_not_have_is_ignored() {
    let plugins = BTreeMap::from([("no-such-plugin".to_owned(), PluginConfig::default())]);

    let registry = Registry::with_builtins(&plugins);

    assert_eq!(
        registry.enabled(),
        [
            PluginId::from_static("local-auth"),
            PluginId::from_static("log-events")
        ]
    );
}

#[test]
fn a_plugin_that_isnt_running_may_be_disabled_only_while_another_provides_an_authenticator() {
    let without_local_auth = Registry::with_builtins(&BTreeMap::from([(
        "local-auth".to_owned(),
        PluginConfig {
            enabled: false,
            ..PluginConfig::default()
        },
    )]));
    let with_local_auth = Registry::with_builtins(&BTreeMap::new());

    // As after `local-auth` panicked: it may be what would provide one after a restart.
    let refused = without_local_auth
        .disable_unless_last_provider::<dyn Authenticator>(&PluginId::from_static("local-auth"));
    let allowed = with_local_auth
        .disable_unless_last_provider::<dyn Authenticator>(&PluginId::from_static("tmdb"));

    assert!(refused.is_err(), "{refused:?}");
    assert!(matches!(allowed, Ok(false)), "{allowed:?}");
}

/// ADR 0006: Built-in Plugins are separate crates so the compiler keeps them out of Kernel
/// internals. Of this workspace's crates they may use only the plugin API and the protobuf types,
/// and an Installed Plugin in `plugins/` only `nama-pdk`.
#[test]
fn plugin_crates_depend_on_no_workspace_crate_but_the_plugin_api_and_proto_or_the_pdk() {
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

    let mut checked = 0;
    for package in metadata["packages"].as_array().unwrap() {
        let manifest = Path::new(package["manifest_path"].as_str().unwrap());
        if !manifest.starts_with(&plugins_dir) {
            continue;
        }
        checked += 1;
        let installed = package["targets"].as_array().unwrap().iter().any(|target| {
            target["crate_types"]
                .as_array()
                .unwrap()
                .contains(&"cdylib".into())
        });
        let allowed: &[&str] = if installed {
            &["nama-pdk"]
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
        checked > 0,
        "found no plugin crates in {}",
        plugins_dir.display()
    );
}
