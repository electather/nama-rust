//! The Built-in Plugins: which ship in the Server, how config turns them on and off, and what
//! they may depend on.

use std::{collections::BTreeMap, path::Path, process::Command};

use nama_plugin_api::PluginId;
use nama_server::{
    config::{Config, PluginConfig},
    plugins::Registry,
};

#[test]
fn every_builtin_plugin_is_enabled_by_default() {
    let registry = Registry::with_builtins(&Config::default().plugins);

    assert_eq!(registry.enabled(), [PluginId::from_static("log-events")]);
}

#[test]
fn config_can_disable_a_builtin_plugin() {
    let plugins = BTreeMap::from([("log-events".to_owned(), PluginConfig { enabled: false })]);

    let registry = Registry::with_builtins(&plugins);

    assert_eq!(registry.enabled(), Vec::<PluginId>::new());
}

#[test]
fn config_for_a_plugin_the_server_does_not_have_is_ignored() {
    let plugins = BTreeMap::from([("no-such-plugin".to_owned(), PluginConfig { enabled: true })]);

    let registry = Registry::with_builtins(&plugins);

    assert_eq!(registry.enabled(), [PluginId::from_static("log-events")]);
}

/// ADR 0006: Built-in Plugins are separate crates so the compiler keeps them out of Kernel
/// internals. Of this workspace's crates they may use only the plugin API and the protobuf types.
#[test]
fn builtin_plugin_crates_depend_on_no_workspace_crate_but_the_plugin_api_and_proto() {
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
    let plugins_dir = workspace.join("plugins").canonicalize().unwrap();

    let mut checked = 0;
    for package in metadata["packages"].as_array().unwrap() {
        let manifest = Path::new(package["manifest_path"].as_str().unwrap());
        if !manifest.starts_with(&plugins_dir) {
            continue;
        }
        checked += 1;
        for dependency in package["dependencies"].as_array().unwrap() {
            let name = dependency["name"].as_str().unwrap();
            let in_workspace = dependency.get("path").is_some_and(|path| !path.is_null());
            assert!(
                !in_workspace || ["nama-plugin-api", "nama-proto"].contains(&name),
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
