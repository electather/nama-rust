use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, Registrar,
    automation::{
        Automation, DeleteAsked, DeleteStarted, Kernel, Libraries, LibrariesAsked, Permission,
        Refused, StoreEntry, StoreKey, StoreValue, Stored, Tick, Titles, TitlesAsked, WatchStates,
        WatchStatesAsked,
    },
    settings::Settings,
};

use super::super::{
    Installed,
    load::{load, write_cache_config},
};
use crate::plugins::Registry;

#[path = "../../../tests/support/test_plugin.rs"]
mod test_plugin;

const NOT_GRANTED: &str = "the Admin didn't grant this Plugin PERMISSION_READ_LIBRARIES";

/// A Kernel that records each Store call with its key and bytes, and refuses everything else.
#[derive(Default)]
struct Fake {
    calls: Mutex<Vec<(&'static str, String, Vec<u8>)>>,
}

impl Fake {
    fn record(&self, function: &'static str, key: &str, value: &[u8]) {
        self.calls
            .lock()
            .unwrap()
            .push((function, key.to_owned(), value.to_vec()));
    }
}

fn refuse<'a, T: Send + 'a>(words: &str) -> BoxFuture<'a, Result<T, Refused>> {
    let refused = Refused(words.to_owned());
    Box::pin(async move { Err(refused) })
}

impl Kernel for Fake {
    fn store_get<'a>(&'a self, key: &'a StoreKey) -> BoxFuture<'a, Result<StoreValue, Refused>> {
        self.record("store_get", &key.key, &[]);
        let value = StoreValue {
            value: Some(b"kept".to_vec()),
            ..StoreValue::default()
        };
        Box::pin(async move { Ok(value) })
    }

    fn store_put<'a>(&'a self, entry: &'a StoreEntry) -> BoxFuture<'a, Result<Stored, Refused>> {
        self.record("store_put", &entry.key, &entry.value);
        Box::pin(async { Ok(Stored::default()) })
    }

    fn store_delete<'a>(&'a self, key: &'a StoreKey) -> BoxFuture<'a, Result<Stored, Refused>> {
        self.record("store_delete", &key.key, &[]);
        Box::pin(async { Ok(Stored::default()) })
    }

    fn list_libraries<'a>(
        &'a self,
        _: &'a LibrariesAsked,
    ) -> BoxFuture<'a, Result<Libraries, Refused>> {
        refuse(NOT_GRANTED)
    }

    fn list_titles<'a>(&'a self, _: &'a TitlesAsked) -> BoxFuture<'a, Result<Titles, Refused>> {
        refuse("not faked")
    }

    fn list_watch_states<'a>(
        &'a self,
        _: &'a WatchStatesAsked,
    ) -> BoxFuture<'a, Result<WatchStates, Refused>> {
        refuse("not faked")
    }

    fn delete_media<'a>(
        &'a self,
        _: &'a DeleteAsked,
    ) -> BoxFuture<'a, Result<DeleteStarted, Refused>> {
        refuse("not faked")
    }
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
    let fake = Arc::new(Fake::default());
    let kernel: Arc<dyn Kernel> = Arc::<Fake>::clone(&fake);
    let automation = automation(&automation_plugin(&data_dir), Some(kernel));

    // A crash every time would disable the Plugin by the third.
    for _ in 0..3 {
        let failed = automation.tick(&Tick::default()).await.unwrap_err();
        assert!(failed.to_string().contains(NOT_GRANTED), "{failed}");
    }

    let calls = fake.calls.lock().unwrap();
    assert_eq!(
        calls[..2],
        [
            ("store_put", "ticked".to_owned(), b"yes".to_vec()),
            ("store_get", "ticked".to_owned(), Vec::new()),
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
    let key = StoreKey::default();

    let before = kernel.store_get(&key).await.unwrap_err();
    let made = Arc::new(Mutex::new(Vec::new()));
    let making = Arc::clone(&made);
    registry.serve_kernel(Arc::new(move |plugin, permissions| {
        making.lock().unwrap().push((plugin, permissions));
        Arc::new(Fake::default())
    }));
    kernel.store_get(&key).await.unwrap();

    assert_eq!(before.0, "the Server isn't serving yet");
    assert_eq!(
        *made.lock().unwrap(),
        [(id, HashSet::from([Permission::PERMISSION_STORE]))]
    );
}
