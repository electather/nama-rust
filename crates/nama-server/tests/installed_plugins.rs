//! Installed Plugins on a running Server: listed once their `.wasm` is dropped into
//! `<data_dir>/plugins/`, enabled and disabled without a restart, shown with why if refused, and
//! disabled when they keep crashing.

#[path = "support/refused.rs"]
mod refused;
#[path = "support/server.rs"]
#[expect(dead_code, reason = "the harness's plain `start` has no use here")]
mod server;
#[path = "support/test_plugin.rs"]
mod test_plugin;

use std::{
    collections::BTreeMap,
    io,
    path::Path,
    sync::{Arc, Mutex, OnceLock, PoisonError},
    time::{Duration, Instant},
};

use connectrpc::{ErrorCode, client::HttpClient};
use nama_plugin_api::PluginId;
use nama_proto::nama::v1::{
    CreateLibraryRequest, DisablePluginRequest, EnablePluginRequest, LibraryKind,
    LibraryServiceClient, ListPluginsRequest, Plugin, PluginKind, PluginServiceClient,
    RefusalReason, ScanLibraryRequest, SettingValue, UpdatePluginSettingsRequest,
    setting_value::Value,
};
use nama_server::{config::Config, db::Database, plugin_settings, plugins::Registry};
use refused::assert_refused;
use server::TestServer;

const WAIT: Duration = Duration::from_secs(10);

/// A running Server with no Installed Plugins yet.
struct Started {
    server: TestServer,
    db: Arc<Database>,
    plugins: Registry,
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
impl Started {
    async fn new() -> Self {
        let plugins = Registry::with_builtins(&BTreeMap::new());
        let (server, db) = TestServer::start_with_db(Config::default(), plugins.clone()).await;
        // As on a Server's first start.
        plugin_settings::import(&db, &BTreeMap::new())
            .await
            .unwrap();
        Self {
            server,
            db,
            plugins,
        }
    }

    /// Drops the test Plugin, built with `features`, into the Server's `plugins/` as `file`.
    fn drop_in(&self, file: &str, features: &str) {
        let dir = self.server.data_dir().join("plugins");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(file), test_plugin::build(features)).unwrap();
    }

    fn client(&self) -> PluginServiceClient<HttpClient> {
        PluginServiceClient::new(HttpClient::plaintext(), self.server.client_config())
    }

    /// Creates a Movies Library of `folder`, and returns its id.
    async fn library(&self, folder: &Path) -> i64 {
        self.libraries()
            .create_library(CreateLibraryRequest {
                name: "Movies".to_owned(),
                kind: LibraryKind::LIBRARY_KIND_MOVIES.into(),
                folders: vec![folder.to_str().unwrap().to_owned()],
                ..Default::default()
            })
            .await
            .unwrap()
            .into_owned()
            .library
            .into_option()
            .unwrap()
            .id
    }

    /// Starts a scan of the Library `id`.
    async fn scan(&self, id: i64) {
        self.libraries()
            .scan_library(ScanLibraryRequest {
                id,
                ..Default::default()
            })
            .await
            .unwrap();
    }

    fn libraries(&self) -> LibraryServiceClient<HttpClient> {
        LibraryServiceClient::new(HttpClient::plaintext(), self.server.client_config())
    }

    /// Drops in the test Plugin with its Observer set to `observer` (`log`, `loop` or `panic`),
    /// enables it, and returns a Library to scan so it is told about Events.
    async fn observing(&self, observer: &str) -> (i64, tempfile::TempDir) {
        self.drop_in("test.wasm", "");
        // Listing is what reads `plugins/`.
        self.listed("test-plugin").await.unwrap();
        let observer = SettingValue {
            value: Some(Value::TextValue(observer.to_owned())),
            ..Default::default()
        };
        self.client()
            .update_plugin_settings(UpdatePluginSettingsRequest {
                id: "test-plugin".to_owned(),
                settings: std::iter::once(("observer".to_owned(), observer)).collect(),
                ..Default::default()
            })
            .await
            .unwrap();
        self.client()
            .enable_plugin(enable("test-plugin"))
            .await
            .unwrap();
        let media = tempfile::tempdir().unwrap();
        std::fs::write(media.path().join("Heat (1995).mkv"), b"video").unwrap();
        (self.library(media.path()).await, media)
    }

    /// The Plugin `id` as the Plugins page lists it.
    async fn listed(&self, id: &str) -> Option<Plugin> {
        let listed = self
            .client()
            .list_plugins(ListPluginsRequest::default())
            .await
            .unwrap()
            .into_owned();
        listed.plugins.into_iter().find(|plugin| plugin.id == id)
    }
}

fn enable(id: &str) -> EnablePluginRequest {
    EnablePluginRequest {
        id: id.to_owned(),
        ..Default::default()
    }
}

/// Everything the Server logs, to this test binary's one global subscriber: a thread-local one
/// wouldn't see what Observers log on the runtime's other threads.
#[derive(Clone, Default)]
struct Log(Arc<Mutex<Vec<u8>>>);

impl Log {
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    fn global() -> &'static Self {
        static LOG: OnceLock<Log> = OnceLock::new();
        LOG.get_or_init(|| {
            let log = Self::default();
            let writer = log.clone();
            let subscriber = tracing_subscriber::fmt()
                .with_max_level(tracing::Level::INFO)
                // A test's Server runs on its thread, which is named after it.
                .with_thread_names(true)
                .with_writer(move || writer.clone())
                .finish();
            tracing::subscriber::set_global_default(subscriber).unwrap();
            log
        })
    }

    fn contents(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap_or_else(PoisonError::into_inner)).into_owned()
    }

    /// How many logged lines contain every one of `needles`.
    fn count(&self, needles: &[&str]) -> usize {
        self.contents()
            .lines()
            .filter(|line| needles.iter().all(|needle| line.contains(needle)))
            .count()
    }

    /// Waits until `times` lines containing every one of `needles` are logged.
    async fn wait_for(&self, needles: &[&str], times: usize) {
        let started = Instant::now();
        while self.count(needles) < times {
            assert!(
                started.elapsed() < WAIT,
                "nothing logged {needles:?}: {}",
                self.contents()
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
}

impl io::Write for Log {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[tokio::test]
async fn a_dropped_plugin_is_listed_disabled_observes_scans_only_while_enabled_and_stays_enabled() {
    let log = Log::global();
    let started = Started::new().await;
    let test_plugin = PluginId::from_static("test-plugin");
    assert_eq!(started.listed("test-plugin").await, None);

    started.drop_in("test.wasm", "");
    let listed = started.listed("test-plugin").await.unwrap();
    assert_eq!(listed.kind, PluginKind::PLUGIN_KIND_INSTALLED);
    assert_eq!(listed.version, "0.1.0");
    assert!(!listed.enabled);
    assert_eq!(listed.refused_reason, "");
    // Saving its settings doesn't enable it either.
    let updated = started
        .client()
        .update_plugin_settings(UpdatePluginSettingsRequest {
            id: "test-plugin".to_owned(),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned();
    assert!(!updated.plugin.unwrap().enabled);

    started
        .client()
        .enable_plugin(enable("test-plugin"))
        .await
        .unwrap();
    assert!(started.listed("test-plugin").await.unwrap().enabled);
    let media = tempfile::tempdir().unwrap();
    std::fs::write(media.path().join("Heat (1995).mkv"), b"video").unwrap();
    let library = started.library(media.path()).await;
    started.scan(library).await;
    log.wait_for(&["test-plugin observed", "ScanFinished"], 1)
        .await;

    // A restart enables what the database holds, as `main` does.
    let stored = plugin_settings::stored(&started.db).await.unwrap().unwrap();
    let restarted = Registry::with_builtins(&stored);
    restarted.load_installed(started.server.data_dir(), &stored);
    assert!(restarted.enabled().contains(&test_plugin));

    started
        .client()
        .disable_plugin(DisablePluginRequest {
            id: "test-plugin".to_owned(),
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(!started.plugins.enabled().contains(&test_plugin));
    assert!(!started.listed("test-plugin").await.unwrap().enabled);

    // Disabled, it is told nothing: of the next two scans it observes only the one after it is
    // enabled again.
    std::fs::write(media.path().join("Alien (1979).mkv"), b"video").unwrap();
    started.scan(library).await;
    let this_test = std::thread::current().name().unwrap().to_owned();
    log.wait_for(&["scanned a Library", &this_test], 2).await;
    started
        .client()
        .enable_plugin(enable("test-plugin"))
        .await
        .unwrap();
    std::fs::write(media.path().join("Brazil (1985).mkv"), b"video").unwrap();
    started.scan(library).await;
    log.wait_for(&["test-plugin observed", "Brazil"], 1).await;
    assert_eq!(log.count(&["test-plugin observed", "Alien"]), 0);
}

#[tokio::test]
async fn a_refused_plugin_is_listed_with_why_and_enabling_it_is_refused() {
    let started = Started::new().await;

    started.drop_in("out-of-range.wasm", "out-of-range");
    let listed = started.listed("out-of-range.wasm").await.unwrap();
    assert_eq!(listed.kind, PluginKind::PLUGIN_KIND_INSTALLED);
    assert!(!listed.enabled);
    assert!(
        listed
            .refused_reason
            .contains("it wants nama-api <0.1, and this Server has 0.1.0"),
        "{listed:?}"
    );

    let refusal = assert_refused(
        started
            .client()
            .enable_plugin(enable("out-of-range.wasm"))
            .await,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_PLUGIN_REFUSED,
    );
    assert_eq!(refusal.field, "id");
    assert_eq!(refusal.params["pluginId"], "out-of-range.wasm");
    assert_eq!(refusal.params["reason"], listed.refused_reason);
    assert!(!started.listed("out-of-range.wasm").await.unwrap().enabled);
    let update = UpdatePluginSettingsRequest {
        id: "out-of-range.wasm".to_owned(),
        ..Default::default()
    };
    assert_refused(
        started.client().update_plugin_settings(update).await,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_PLUGIN_REFUSED,
    );
    assert_eq!(
        plugin_settings::stored(&started.db).await.unwrap(),
        Some(BTreeMap::new())
    );
}

#[tokio::test]
async fn a_second_file_with_a_plugins_id_is_refused() {
    let started = Started::new().await;

    started.drop_in("a.wasm", "");
    started.drop_in("b.wasm", "");

    let first = started.listed("test-plugin").await.unwrap();
    assert_eq!(first.refused_reason, "");
    let second = started.listed("b.wasm").await.unwrap();
    assert_eq!(
        second.refused_reason,
        "its id test-plugin is another Plugin's"
    );
}

#[tokio::test]
async fn an_observer_that_loops_forever_is_stopped_by_the_timeout_and_the_server_carries_on() {
    let log = Log::global();
    let started = Started::new().await;
    let (library, _media) = started.observing("loop").await;

    started.scan(library).await;
    log.wait_for(&["test-plugin is looping"], 1).await;
    // Served while its call loops, which the timeout stops only after 5 seconds.
    assert!(started.listed("test-plugin").await.is_some());
    assert_eq!(log.count(&["test-plugin", "observer crashed: timeout"]), 0);
    log.wait_for(&["test-plugin", "observer crashed: timeout"], 1)
        .await;

    assert!(started.listed("test-plugin").await.is_some());
    started.scan(library).await;
}

#[tokio::test]
async fn a_plugin_that_keeps_panicking_logs_why_is_disabled_and_saved_so_and_can_be_enabled_again()
{
    let log = Log::global();
    let started = Started::new().await;
    let test_plugin = PluginId::from_static("test-plugin");
    let (library, _media) = started.observing("panic").await;

    let disabled_at = Instant::now();
    while started.plugins.enabled().contains(&test_plugin) {
        assert!(disabled_at.elapsed() < WAIT, "it was never disabled");
        started.scan(library).await;
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    log.wait_for(
        &["test-plugin", "the test Plugin's Observer was set to panic"],
        3,
    )
    .await;
    log.wait_for(
        &["test-plugin", "crashed 3 times in a row", "disabling it"],
        1,
    )
    .await;
    // Saved like an admin's disable, which the Server shows and a restart keeps.
    assert!(!started.listed("test-plugin").await.unwrap().enabled);
    let saved_at = Instant::now();
    while plugin_settings::stored(&started.db).await.unwrap().unwrap()["test-plugin"].enabled {
        assert!(saved_at.elapsed() < WAIT, "its disable was never saved");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    started
        .client()
        .enable_plugin(enable("test-plugin"))
        .await
        .unwrap();
    assert!(started.listed("test-plugin").await.unwrap().enabled);
}
