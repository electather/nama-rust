use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use buffa::Message;
use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Registrar,
    automation::{
        Automation, Kernel, KernelRefused, Permission, StoreEntry, StoreKey, StoreValue, Tick,
    },
    settings::Settings,
};

use super::super::{
    Installed,
    listing::{listing, until_failing},
    load::{load, write_cache_config},
};
use crate::{log_capture::CapturedLog, plugins::Registry};

#[path = "../../../tests/support/test_plugin.rs"]
mod test_plugin;

const NOT_GRANTED: &str = "the Admin didn't grant this Plugin PERMISSION_READ_LIBRARIES";

/// The function and request of each call a [`recording`] Kernel was asked.
type Calls = Arc<Mutex<Vec<(String, Vec<u8>)>>>;

/// A Kernel that records each call on `calls`, answers the Store's functions, `store_get` with
/// "kept", and refuses everything else as not granted.
fn recording(calls: Calls) -> Arc<dyn Kernel> {
    Arc::new(
        move |function: &str, request| -> BoxFuture<'static, Result<Vec<u8>, KernelRefused>> {
            calls.lock().unwrap().push((function.to_owned(), request));
            let answer = match function {
                "store_get" => Ok(StoreValue {
                    value: Some(b"kept".to_vec()),
                    ..StoreValue::default()
                }
                .encode_to_vec()),
                "store_put" | "store_delete" => Ok(Vec::new()),
                _ => Err(KernelRefused(NOT_GRANTED.to_owned())),
            };
            Box::pin(async move { answer })
        },
    )
}

/// The test Plugin built with its `automation` feature, loaded as the Server loads it.
fn automation_plugin(data_dir: &tempfile::TempDir) -> Installed {
    let cache_config = write_cache_config(data_dir.path()).unwrap();
    load(test_plugin::build("automation"), &cache_config).unwrap()
}

/// `plugin`'s Automation, enabled with `kernel` if given.
fn automation(plugin: &Installed, kernel: Option<Arc<dyn Kernel>>) -> Arc<dyn Automation> {
    let mut registrar = Registrar::with_settings(Settings::default());
    if let Some(kernel) = kernel {
        registrar = registrar.with_kernel(kernel);
    }
    plugin.register(&mut registrar).unwrap();
    registrar
        .into_registrations()
        .iter()
        .find_map(nama_plugin_api::Registration::provider::<dyn Automation>)
        .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_tick_reaches_the_kernels_store_and_gets_its_refusals_back_without_crashing() {
    let data_dir = tempfile::tempdir().unwrap();
    let calls = Calls::default();
    let kernel = recording(Arc::clone(&calls));
    let automation = automation(&automation_plugin(&data_dir), Some(kernel));

    // A crash every time would disable the Plugin by the third.
    for _ in 0..3 {
        let failed = automation.tick(&Tick::default()).await.unwrap_err();
        assert!(failed.to_string().contains(NOT_GRANTED), "{failed}");
    }

    let put = StoreEntry {
        key: "ticked".to_owned(),
        value: b"yes".to_vec(),
        ..StoreEntry::default()
    };
    let get = StoreKey {
        key: "ticked".to_owned(),
        ..StoreKey::default()
    };
    assert_eq!(
        calls.lock().unwrap()[..2],
        [
            ("store_put".to_owned(), put.encode_to_vec()),
            ("store_get".to_owned(), get.encode_to_vec()),
        ]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_plugin_calling_the_kernel_loads_without_one_and_is_refused_by_it() {
    let data_dir = tempfile::tempdir().unwrap();

    let plugin = automation_plugin(&data_dir);
    let failed = automation(&plugin, None)
        .tick(&Tick::default())
        .await
        .unwrap_err();

    assert_eq!(
        plugin.manifest.permissions,
        [
            Permission::PERMISSION_STORE,
            Permission::PERMISSION_READ_LIBRARIES
        ]
    );
    assert!(
        failed
            .to_string()
            .contains("the Kernel's functions answer only an enabled Plugin"),
        "{failed}"
    );
}

/// A Built-in Plugin requesting the Store Permission, which keeps the Kernel's functions it is
/// handed.
struct Keeps(Arc<Mutex<Option<Arc<dyn Kernel>>>>);

impl Plugin for Keeps {
    fn manifest(&self) -> Manifest {
        Manifest {
            permissions: vec![Permission::PERMISSION_STORE],
            ..Manifest::new(PluginId::from("keeps".to_owned()))
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        *self.0.lock().unwrap() = registrar.kernel();
        Ok(())
    }
}

#[tokio::test]
async fn an_enabled_plugin_is_granted_only_what_its_manifest_requests_once_the_kernel_serves() {
    let registry = Registry::new();
    let kept = Arc::new(Mutex::new(None));
    let handed = Arc::clone(&kept);
    registry.describe_builtin(move || Arc::new(Keeps(Arc::clone(&handed))));
    let id = PluginId::from("keeps".to_owned());
    let granted = HashSet::from([
        Permission::PERMISSION_STORE,
        Permission::PERMISSION_DELETE_MEDIA,
    ]);
    registry
        .enable_by_id_granting(&id, Settings::default(), granted)
        .unwrap();
    let kernel = kept.lock().unwrap().clone().unwrap();
    let before = kernel.call("store_get", Vec::new()).await.unwrap_err();
    let made = Arc::new(Mutex::new(Vec::new()));
    let making = Arc::clone(&made);
    registry.serve_kernel(Arc::new(move |plugin, permissions| {
        making.lock().unwrap().push((plugin, permissions));
        recording(Calls::default())
    }));
    kernel.call("store_get", Vec::new()).await.unwrap();

    assert_eq!(before.0, "the Server isn't serving yet");
    assert_eq!(
        *made.lock().unwrap(),
        [(id, HashSet::from([Permission::PERMISSION_STORE]))]
    );
}

#[tokio::test]
async fn an_installed_automations_failed_tick_is_logged_once() {
    let data_dir = tempfile::tempdir().unwrap();
    let registry = listing(automation_plugin(&data_dir));
    registry.serve_kernel(Arc::new(|_, _| Arc::new(Fake::default())));
    let id = PluginId::from_static("test-plugin");
    let points = registry.extension_points();

    // The test's runtime has one thread, so the tick loop logs to it.
    let log = CapturedLog::start();
    registry.enable_by_id(&id, Settings::default()).unwrap();
    until_failing(&points, &id, PointKey::provide::<dyn Automation>()).await;
    let logged = log.finish();

    let failed: Vec<&str> = logged
        .lines()
        .filter(|line| line.contains(NOT_GRANTED))
        .collect();
    assert_eq!(failed.len(), 1, "{logged}");
    assert!(
        failed[0].contains("automation tick failed plugin=test-plugin"),
        "{logged}"
    );
}
