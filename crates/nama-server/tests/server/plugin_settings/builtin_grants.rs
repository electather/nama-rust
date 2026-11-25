//! Each start grants a Built-in Plugin exactly the Plugin Permissions its Manifest requests,
//! whatever was stored, while an Installed Plugin keeps only what an Admin granted (ADR 0049,
//! 0079).

use std::{
    collections::{BTreeMap, HashSet},
    path::Path,
    sync::Arc,
};

use nama_plugin_api::{
    Manifest, Plugin, PluginError, PluginId, Registrar, automation::Permission,
    settings::SettingValue,
};
use nama_server::{db::Database, plugin_settings::PluginSettings, plugins::Registry};

use super::{Fake, config, open, registry};

const STORE: Permission = Permission::PERMISSION_STORE;
const READ_LIBRARIES: Permission = Permission::PERMISSION_READ_LIBRARIES;

/// A Plugin with [`Fake`]'s settings, `id`, requesting `permissions`.
struct Requesting {
    id: &'static str,
    permissions: &'static [Permission],
}

impl Plugin for Requesting {
    fn manifest(&self) -> Manifest {
        let fake = Fake {
            id: self.id,
            panics: false,
        };
        Manifest {
            permissions: self.permissions.to_vec(),
            ..fake.manifest()
        }
    }

    fn register(&self, _registrar: &mut Registrar) -> Result<(), PluginError> {
        Ok(())
    }
}

fn requesting() -> PluginId {
    PluginId::from_static("requesting")
}

/// Starts the Plugins of [`registry`] on `db`, with the Built-in `requesting` requesting
/// `permissions` beside them, and the config's `[plugins.*]` tables in `toml`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn start(
    db: &Arc<Database>,
    data_dir: &Path,
    permissions: &'static [Permission],
    toml: &str,
) -> PluginSettings {
    let registry = registry();
    registry.describe_builtin(move || {
        Arc::new(Requesting {
            id: "requesting",
            permissions,
        })
    });
    let config = config(data_dir, toml);
    PluginSettings::start(Arc::clone(db), registry, &config)
        .await
        .unwrap()
}

/// The Plugin Permissions `plugins` stores as granted the Plugin `id`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn held(plugins: &PluginSettings, id: &PluginId) -> HashSet<Permission> {
    plugins.get(id).await.unwrap().unwrap().granted
}

#[tokio::test]
async fn a_builtin_plugin_holds_a_permission_it_newly_requests_after_a_restart() {
    let (data_dir, db) = open().await;
    let first = start(&db, data_dir.path(), &[STORE], "").await;
    assert_eq!(held(&first, &requesting()).await, HashSet::from([STORE]));
    first.shutdown().await;

    let restarted = start(&db, data_dir.path(), &[STORE, READ_LIBRARIES], "").await;

    assert_eq!(
        held(&restarted, &requesting()).await,
        HashSet::from([STORE, READ_LIBRARIES])
    );
}

#[tokio::test]
async fn a_builtin_plugin_holds_a_permission_revoked_before_again_after_a_restart() {
    let (data_dir, db) = open().await;
    start(&db, data_dir.path(), &[STORE], "")
        .await
        .shutdown()
        .await;
    // As an Admin could revoke it before Built-ins always held what they request.
    db.write(|tx| {
        Box::pin(async move {
            sqlx::query("UPDATE plugin_settings SET granted_permissions = '[]'")
                .execute(&mut **tx)
                .await
        })
    })
    .await
    .unwrap();

    let restarted = start(&db, data_dir.path(), &[STORE], "").await;

    assert_eq!(
        held(&restarted, &requesting()).await,
        HashSet::from([STORE])
    );
}

#[tokio::test]
async fn a_builtin_plugin_no_longer_holds_a_permission_it_stops_requesting_after_a_restart() {
    let (data_dir, db) = open().await;
    let first = start(&db, data_dir.path(), &[STORE], "").await;
    first.shutdown().await;

    let restarted = start(&db, data_dir.path(), &[], "").await;

    assert_eq!(held(&restarted, &requesting()).await, HashSet::new());
}

#[tokio::test]
async fn a_builtin_plugins_new_grants_keep_whether_it_is_enabled_and_its_settings() {
    let (data_dir, db) = open().await;
    let toml = "[plugins.requesting]\nenabled = false\ngreeting = \"hello\"";
    start(&db, data_dir.path(), &[STORE], toml)
        .await
        .shutdown()
        .await;

    let restarted = start(&db, data_dir.path(), &[STORE, READ_LIBRARIES], "").await;

    let stored = restarted.get(&requesting()).await.unwrap().unwrap().stored;
    assert!(!stored.enabled);
    let greeting = SettingValue::Text("hello".to_owned());
    assert_eq!(
        stored.settings,
        BTreeMap::from([("greeting".to_owned(), greeting)])
    );
    assert_eq!(
        held(&restarted, &requesting()).await,
        HashSet::from([STORE, READ_LIBRARIES])
    );
}

/// [`registry`], listing `installed`, requesting two Plugin Permissions, as an Installed Plugin.
fn with_installed() -> Registry {
    let registry = registry();
    registry.list_as_installed(vec![|| {
        Arc::new(Requesting {
            id: "installed",
            permissions: &[STORE, READ_LIBRARIES],
        })
    }]);
    registry
}

#[tokio::test]
async fn an_installed_plugin_holds_only_what_an_admin_granted_after_a_restart() {
    let (data_dir, db) = open().await;
    let config = config(data_dir.path(), "");
    let id = PluginId::from_static("installed");
    let first = PluginSettings::start(Arc::clone(&db), with_installed(), &config)
        .await
        .unwrap();
    first
        .enable_granting(&id, HashSet::from([STORE]))
        .await
        .unwrap();
    first.shutdown().await;

    let restarted = PluginSettings::start(Arc::clone(&db), with_installed(), &config)
        .await
        .unwrap();

    assert_eq!(held(&restarted, &id).await, HashSet::from([STORE]));
}
