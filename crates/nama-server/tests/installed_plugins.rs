//! Installed Plugins on a running Server: listed once their `.wasm` is dropped into
//! `<data_dir>/plugins/`, enabled and disabled without a restart, shown with why if refused,
//! reaching only the hosts they are allowed, and disabled when they keep crashing.

#[path = "support/log.rs"]
mod log;
#[path = "support/refused.rs"]
mod refused;
#[path = "support/server.rs"]
#[expect(dead_code, reason = "the harness's plain `start` has no use here")]
mod server;
#[path = "support/test_plugin.rs"]
mod test_plugin;

use std::{
    collections::BTreeMap,
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};

use connectrpc::{ConnectError, ErrorCode, client::HttpClient};
use log::Log;
use nama_plugin_api::PluginId;
use nama_proto::nama::v1::{
    CreateLibraryRequest, DisablePluginRequest, EnablePluginRequest, ExtensionPoint,
    GetPluginRequest, LibraryKind, LibraryServiceClient, ListPluginsRequest, Plugin, PluginKind,
    PluginServiceClient, PluginSummary, RefusalReason, ScanLibraryRequest, SettingValue,
    TestPluginSettingsRequest, TestPluginSettingsResponse, UpdatePluginSettingsRequest,
    UpdatePluginSettingsResponse, setting_value::Value,
};
use nama_server::{config::Config, db::Database, plugin_settings, plugins::Registry};
use refused::assert_refused;
use server::TestServer;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

/// How long a test waits for the Server to disable a crashing Plugin or save that.
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

    /// Saves the test Plugin's settings as the Plugins page does, each one text.
    async fn save_settings(
        &self,
        settings: &[(&str, &str)],
    ) -> Result<UpdatePluginSettingsResponse, ConnectError> {
        let request = UpdatePluginSettingsRequest {
            id: "test-plugin".to_owned(),
            settings: text_values(settings),
            ..Default::default()
        };
        let response = self.client().update_plugin_settings(request).await?;
        Ok(response.into_owned())
    }

    /// Tests the test Plugin's unsaved `settings` as the Plugins page's "Test" does, each one text.
    async fn test_settings(
        &self,
        settings: &[(&str, &str)],
    ) -> Result<TestPluginSettingsResponse, ConnectError> {
        let request = TestPluginSettingsRequest {
            id: "test-plugin".to_owned(),
            settings: text_values(settings),
            ..Default::default()
        };
        let response = self.client().test_plugin_settings(request).await?;
        Ok(response.into_owned())
    }

    /// Drops in the test Plugin with its Observer set to `observer` (`log`, `loop` or `panic`),
    /// enables it, and returns a Library to scan so it is told about Events.
    async fn observing(&self, observer: &str) -> (i64, tempfile::TempDir) {
        self.drop_in("test.wasm", "");
        // Listing is what reads `plugins/`.
        self.listed("test-plugin").await.unwrap();
        self.save_settings(&[("observer", observer)]).await.unwrap();
        self.client()
            .enable_plugin(enable("test-plugin"))
            .await
            .unwrap();
        let media = tempfile::tempdir().unwrap();
        std::fs::write(media.path().join("Heat (1995).mkv"), b"video").unwrap();
        (self.library(media.path()).await, media)
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

    /// The Plugin `id` as the Plugins page lists it.
    async fn listed(&self, id: &str) -> Option<PluginSummary> {
        let listed = self
            .client()
            .list_plugins(ListPluginsRequest::default())
            .await
            .unwrap()
            .into_owned();
        listed.summaries.into_iter().find(|plugin| plugin.id == id)
    }

    /// The Plugin `id` with its settings, as its details page gets it.
    async fn got(&self, id: &str) -> Plugin {
        let request = GetPluginRequest {
            id: id.to_owned(),
            ..Default::default()
        };
        let got = self.client().get_plugin(request).await.unwrap();
        got.into_owned().plugin.into_option().unwrap()
    }
}

/// Starts an HTTP server on 127.0.0.1 that answers every request `204 No Content`, and returns its
/// URL.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn http_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            // A GET's head comes in one read, and its answer doesn't depend on it.
            let mut head = [0; 4096];
            let _read = stream.read(&mut head).await;
            let reply = b"HTTP/1.1 204 No Content\r\nconnection: close\r\n\r\n";
            let _written = stream.write_all(reply).await;
        }
    });
    url
}

/// `settings` as the Plugins page sends them, each one text.
fn text_values<M: FromIterator<(String, SettingValue)>>(settings: &[(&str, &str)]) -> M {
    settings
        .iter()
        .map(|(key, text)| {
            let value = SettingValue {
                value: Some(Value::TextValue((*text).to_owned())),
                ..Default::default()
            };
            ((*key).to_owned(), value)
        })
        .collect()
}

/// The labels of the choices `tested` offers for the setting `key`.
fn choice_labels<'a>(tested: &'a TestPluginSettingsResponse, key: &str) -> Vec<&'a str> {
    tested.choices.get(key).map_or_else(Vec::new, |choices| {
        choices
            .choices
            .iter()
            .map(|choice| choice.label.as_str())
            .collect()
    })
}

fn enable(id: &str) -> EnablePluginRequest {
    EnablePluginRequest {
        id: id.to_owned(),
        ..Default::default()
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
    // Its required settings each have a default.
    assert!(!listed.settings_missing);
    // As its Manifest's `providers`, then `observers`, name them, though it isn't enabled.
    assert_eq!(
        listed.extension_points,
        [
            ExtensionPoint::EXTENSION_POINT_MOVIE_FULFILLER,
            ExtensionPoint::EXTENSION_POINT_NOTIFIER,
            ExtensionPoint::EXTENSION_POINT_EVENTS_OBSERVER
        ]
    );
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
    // Only this scan finds one file unchanged: other tests here scan too.
    log.wait_for(&["scanned a Library", "unchanged: 1"], 1)
        .await;
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
async fn a_plugins_name_and_description_come_from_its_manifest() {
    let started = Started::new().await;

    started.drop_in("test.wasm", "");

    let listed = started.listed("test-plugin").await.unwrap();
    assert_eq!(listed.name, "Test Plugin");
    assert_eq!(
        listed.description,
        "Stands in for an Installed Plugin in the Server's tests."
    );
    let got = started.got("test-plugin").await;
    assert_eq!(
        (got.name, got.description),
        (listed.name, listed.description)
    );
}

#[tokio::test]
async fn a_plugin_whose_manifest_has_no_name_or_description_still_loads_and_enables() {
    let started = Started::new().await;

    started.drop_in("unnamed.wasm", "unnamed");

    let listed = started.listed("test-plugin").await.unwrap();
    assert_eq!(listed.refused_reason, "");
    assert_eq!(
        (listed.name.as_str(), listed.description.as_str()),
        ("", "")
    );
    let got = started.got("test-plugin").await;
    assert_eq!((got.name.as_str(), got.description.as_str()), ("", ""));
    started
        .client()
        .enable_plugin(enable("test-plugin"))
        .await
        .unwrap();
    assert!(started.listed("test-plugin").await.unwrap().enabled);
}

#[tokio::test]
async fn a_refused_plugin_is_listed_and_shown_with_why_and_enabling_it_is_refused() {
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
    let got = started.got("out-of-range.wasm").await;
    assert_eq!(got.refused_reason, listed.refused_reason);
    assert!(got.settings.is_empty(), "{got:?}");
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
async fn a_plugin_reaches_a_host_once_a_url_setting_names_it_without_a_restart() {
    let log = Log::global();
    let server = http_server().await;
    let started = Started::new().await;
    started.drop_in("test.wasm", "");
    // Listing the Plugins, as opening the Plugins page does, reads it.
    assert!(started.listed("test-plugin").await.is_some());
    let media = tempfile::tempdir().unwrap();
    let library = started.library(media.path()).await;

    // Neither its Manifest nor a URL setting names the host it requests.
    started.save_settings(&[("fetch", &server)]).await.unwrap();
    started
        .client()
        .enable_plugin(enable("test-plugin"))
        .await
        .unwrap();
    started.scan(library).await;
    let refused = format!("HTTP request to {server} is not allowed");
    log.wait_for(&["observer failed", &refused], 1).await;
    assert!(started.listed("test-plugin").await.unwrap().enabled);

    // Named on the Plugins page, the host is reachable at once.
    started
        .save_settings(&[("url", &server), ("fetch", &server)])
        .await
        .unwrap();
    started.scan(library).await;
    log.wait_for(&[&format!("test-plugin fetched {server}: 204")], 1)
        .await;

    // Without the URL setting, it is refused again.
    started.save_settings(&[("fetch", &server)]).await.unwrap();
    started.scan(library).await;
    log.wait_for(&["observer failed", &refused], 2).await;
}

#[tokio::test]
async fn a_url_setting_that_isnt_a_url_is_refused_naming_it() {
    let started = Started::new().await;
    started.drop_in("test.wasm", "");
    assert!(started.listed("test-plugin").await.is_some());

    let refusal = assert_refused(
        started.save_settings(&[("url", "sonarr.lan:8989")]).await,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_SETTING_WRONG_KIND,
    );

    assert_eq!(refusal.field, "settings.url");
    assert_eq!(refusal.params["settingKind"], "SETTING_KIND_URL");
    assert_eq!(
        plugin_settings::stored(&started.db).await.unwrap(),
        Some(BTreeMap::new())
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

#[tokio::test]
async fn testing_offers_the_checks_choices_and_only_an_offered_one_is_saved() {
    let started = Started::new().await;
    started.drop_in("test.wasm", "");
    // Only listing reads `plugins/`, so finds the dropped `test.wasm`.
    assert!(started.listed("test-plugin").await.is_some());
    assert!(started.got("test-plugin").await.checks_settings);

    let tested = started.test_settings(&[]).await.unwrap();

    assert_eq!(choice_labels(&tested, "profile"), ["HD", "Ultra HD"]);
    assert_eq!(tested.choices["profile"].choices[1].value, "2");
    started.save_settings(&[("profile", "2")]).await.unwrap();
    let refusal = assert_refused(
        started.save_settings(&[("profile", "3")]).await,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_CHOICE_NOT_OFFERED,
    );
    assert_eq!(refusal.field, "settings.profile");
    let stored = plugin_settings::stored(&started.db).await.unwrap().unwrap();
    assert_eq!(
        stored["test-plugin"].settings.get("profile"),
        Some(&nama_plugin_api::settings::SettingValue::Text(
            "2".to_owned()
        ))
    );
}

#[tokio::test]
async fn testing_an_unsaved_url_reaches_its_host_for_that_test_alone() {
    let log = Log::global();
    let server = http_server().await;
    let started = Started::new().await;
    started.drop_in("test.wasm", "");
    assert!(started.listed("test-plugin").await.is_some());

    let tested = started.test_settings(&[("url", &server)]).await.unwrap();

    assert_eq!(choice_labels(&tested, "profile"), ["HD", "Ultra HD"]);
    // The Plugin enabled afterwards, whose saved settings don't name the host, may not reach it.
    let media = tempfile::tempdir().unwrap();
    let library = started.library(media.path()).await;
    started.save_settings(&[("fetch", &server)]).await.unwrap();
    started
        .client()
        .enable_plugin(enable("test-plugin"))
        .await
        .unwrap();
    started.scan(library).await;
    let refused = format!("HTTP request to {server} is not allowed");
    log.wait_for(&["observer failed", &refused], 1).await;
}

#[tokio::test]
async fn a_check_that_loops_forever_or_panics_fails_in_plain_words_and_the_server_carries_on() {
    let started = Started::new().await;
    started.drop_in("test.wasm", "");
    assert!(started.listed("test-plugin").await.is_some());

    let looped = started.test_settings(&[("fail_with", "loop")]).await;
    let panicked = started.test_settings(&[("fail_with", "panic")]).await;

    for (failed, message, timeout_seconds) in [
        (
            looped,
            "test-plugin didn't finish checking its settings within 5 seconds",
            Some("5"),
        ),
        (
            panicked,
            "test-plugin failed while checking its settings",
            None,
        ),
    ] {
        let err = failed.clone().unwrap_err();
        let refusal = assert_refused(
            failed,
            ErrorCode::FailedPrecondition,
            RefusalReason::REFUSAL_REASON_SETTINGS_CHECK_FAILED,
        );
        assert_eq!(err.message.as_deref(), Some(message));
        let timeout = refusal.params.get("timeoutSeconds").map(String::as_str);
        assert_eq!(timeout, timeout_seconds);
    }
    let retested = started.test_settings(&[]).await.unwrap();
    assert_eq!(choice_labels(&retested, "profile"), ["HD", "Ultra HD"]);
}

#[tokio::test]
async fn a_check_that_fails_is_refused_with_the_plugins_error() {
    let started = Started::new().await;
    started.drop_in("test.wasm", "");
    assert!(started.listed("test-plugin").await.is_some());

    let failed = started
        .test_settings(&[("fail_with", "the service refused the API key")])
        .await;

    assert_eq!(
        failed.unwrap_err().message.as_deref(),
        Some("the service refused the API key")
    );
}
