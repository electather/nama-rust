//! Changing a running Plugin's grants over the API leaves what runs and what is stored agreeing,
//! even when the Plugin won't enable with the new grants (ADR 0028, 0049).

#[path = "support/server.rs"]
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
mod server;

use std::sync::{
    Arc, Mutex, PoisonError,
    atomic::{AtomicUsize, Ordering},
};

use buffa::EnumValue;
use connectrpc::client::HttpClient;
use nama_plugin_api::{
    Manifest, Plugin, PluginError, PluginId, Registrar,
    automation::{Kernel, Permission, UsersAsked},
    settings::{SettingField, SettingKind, SettingsSchema},
};
use nama_proto::nama::v1::{
    EnablePluginRequest, ListPluginsRequest, PluginPermission, PluginServiceClient, PluginSummary,
    SettingValue, UpdatePluginSettingsRequest, setting_value::Value,
};
use nama_server::plugins::Registry;
use server::TestServer;

const STORE: PluginPermission = PluginPermission::PLUGIN_PERMISSION_STORE;
const READ_USERS: PluginPermission = PluginPermission::PLUGIN_PERMISSION_READ_USERS;

/// What one test shares with its [`Grantee`]: how many more times its `register` fails, and the
/// Kernel's functions its last enable handed it, so the test can call them with its grants.
struct Shared {
    failures: AtomicUsize,
    kernel: Mutex<Option<Arc<dyn Kernel>>>,
}

impl Shared {
    const fn new() -> Self {
        Self {
            failures: AtomicUsize::new(0),
            kernel: Mutex::new(None),
        }
    }

    /// Has the next `enables` fail, and forgets the Kernel's functions enabled before.
    fn fail_next(&self, enables: usize) {
        self.failures.store(enables, Ordering::SeqCst);
        *self.kernel.lock().unwrap_or_else(PoisonError::into_inner) = None;
    }

    #[expect(
        clippy::expect_used,
        reason = "a test helper fails its test by panicking"
    )]
    fn kernel(&self) -> Arc<dyn Kernel> {
        let kernel = self.kernel.lock().unwrap_or_else(PoisonError::into_inner);
        let kernel = kernel.as_ref().expect("enabled since `fail_next`");
        Arc::clone(kernel)
    }
}

/// The value of the Grantee's `mood` setting it refuses to start with.
const SULKY: &str = "sulky";

/// An in-process Plugin listed as Installed that requests the Store and Users Permissions,
/// registers nothing, and refuses to start with the `mood` [`SULKY`].
struct Grantee(&'static Shared);

impl Plugin for Grantee {
    fn manifest(&self) -> Manifest {
        let mood = SettingField {
            key: "mood".to_owned(),
            label: "Mood".to_owned(),
            description: String::new(),
            kind: SettingKind::Text,
            required: false,
            default: None,
        };
        Manifest {
            version: "0.0.0".to_owned(),
            permissions: vec![
                Permission::PERMISSION_STORE,
                Permission::PERMISSION_READ_USERS,
            ],
            settings: SettingsSchema { fields: vec![mood] },
            ..Manifest::new(PluginId::from_static("grantee"))
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        if registrar.settings().text("mood") == Some(SULKY) {
            return Err("too sulky to start".into());
        }
        let failing = self
            .0
            .failures
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |left| {
                left.checked_sub(1)
            });
        if failing.is_ok() {
            return Err("told to fail".into());
        }
        *self.0.kernel.lock().unwrap_or_else(PoisonError::into_inner) = registrar.kernel();
        Ok(())
    }
}

static FAILS_ONCE: Shared = Shared::new();
static FAILS_TWICE: Shared = Shared::new();

fn fails_once() -> Registry {
    let registry = Registry::new();
    registry.list_as_installed(vec![|| Arc::new(Grantee(&FAILS_ONCE))]);
    registry
}

fn fails_twice() -> Registry {
    let registry = Registry::new();
    registry.list_as_installed(vec![|| Arc::new(Grantee(&FAILS_TWICE))]);
    registry
}

static REFUSES_A_SETTING: Shared = Shared::new();

fn refuses_a_setting() -> Registry {
    let registry = Registry::new();
    registry.list_as_installed(vec![|| Arc::new(Grantee(&REFUSES_A_SETTING))]);
    registry
}

fn client(server: &TestServer) -> PluginServiceClient<HttpClient> {
    PluginServiceClient::new(HttpClient::plaintext(), server.client_config())
}

/// The Grantee as `ListPlugins` lists it, which also reads the Installed Plugins.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn listed(server: &TestServer) -> PluginSummary {
    client(server)
        .list_plugins(ListPluginsRequest::default())
        .await
        .unwrap()
        .into_owned()
        .summaries
        .into_iter()
        .find(|summary| summary.id == "grantee")
        .unwrap()
}

async fn enable_granting(
    server: &TestServer,
    granted: &[PluginPermission],
) -> Result<(), connectrpc::ConnectError> {
    let request = EnablePluginRequest {
        id: "grantee".to_owned(),
        granted_permissions: values(granted),
        ..Default::default()
    };
    client(server).enable_plugin(request).await.map(drop)
}

/// Saves `mood` as the Grantee's mood.
async fn update_mood(server: &TestServer, mood: &str) -> Result<(), connectrpc::ConnectError> {
    let mood = SettingValue {
        value: Some(Value::TextValue(mood.to_owned())),
        ..Default::default()
    };
    let request = UpdatePluginSettingsRequest {
        id: "grantee".to_owned(),
        settings: std::iter::once(("mood".to_owned(), mood)).collect(),
        ..Default::default()
    };
    client(server)
        .update_plugin_settings(request)
        .await
        .map(drop)
}

fn values(permissions: &[PluginPermission]) -> Vec<EnumValue<PluginPermission>> {
    permissions.iter().map(|&it| it.into()).collect()
}

/// A Server with the Grantee running, granted the Store Permission only.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn granted_the_store(registry: Registry) -> TestServer {
    let server = TestServer::builder().plugins(registry).start().await;
    listed(&server).await;
    enable_granting(&server, &[STORE]).await.unwrap();
    server
}

#[tokio::test]
async fn a_plugin_that_wont_enable_with_new_grants_runs_on_with_its_old_ones() {
    let server = granted_the_store(fails_once()).await;
    FAILS_ONCE.fail_next(1);

    let changed = enable_granting(&server, &[STORE, READ_USERS]).await;

    assert!(changed.is_err(), "{changed:?}");
    let summary = listed(&server).await;
    assert!(summary.enabled);
    assert_eq!(summary.granted_permissions, values(&[STORE]));
    let refused = FAILS_ONCE
        .kernel()
        .list_users(&UsersAsked::default())
        .await
        .expect_err("the Users Permission was never granted");
    assert!(refused.0.contains("Read Users"), "{refused}");
    let server = TestServer::builder()
        .plugins(fails_once())
        .restart(server)
        .await;
    let restarted = listed(&server).await;
    assert!(restarted.enabled);
    assert_eq!(restarted.granted_permissions, values(&[STORE]));
}

#[tokio::test]
async fn a_plugin_that_wont_enable_with_new_grants_nor_its_old_ones_is_saved_disabled() {
    let server = granted_the_store(fails_twice()).await;
    FAILS_TWICE.fail_next(2);

    let changed = enable_granting(&server, &[STORE, READ_USERS]).await;

    assert!(changed.is_err(), "{changed:?}");
    assert!(!listed(&server).await.enabled);
    // So only what is stored keeps it disabled after the restart.
    FAILS_TWICE.fail_next(0);
    let server = TestServer::builder()
        .plugins(fails_twice())
        .restart(server)
        .await;
    assert!(!listed(&server).await.enabled);
}

#[tokio::test]
async fn new_grants_that_fail_after_refused_settings_put_it_back_with_the_settings_it_ran_with() {
    let server = granted_the_store(refuses_a_setting()).await;
    update_mood(&server, SULKY).await.unwrap();
    assert!(listed(&server).await.enabled);

    // Enabled with the sulky mood saved, which it refuses, and put back with no mood, as it ran.
    let changed = enable_granting(&server, &[STORE, READ_USERS]).await;

    assert!(changed.is_err(), "{changed:?}");
    let summary = listed(&server).await;
    assert!(summary.enabled);
    assert_eq!(summary.granted_permissions, values(&[STORE]));
}
