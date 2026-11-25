//! A Plugin's recent Plugin Log lines, as an Admin reads them on its Plugins page (ADR 0061).

#[path = "support/installed_test_plugin.rs"]
mod installed_test_plugin;
#[path = "support/log.rs"]
#[expect(
    dead_code,
    reason = "only installing its subscriber, which keeps Plugin Log lines"
)]
mod log;
#[path = "support/server.rs"]
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
mod server;
#[path = "support/test_plugin.rs"]
mod test_plugin;

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use connectrpc::{ConnectError, client::HttpClient};
use log::Log;
use nama_plugin_api::{Manifest, Plugin, PluginError, PluginId, Registrar};
use nama_proto::nama::v1::{
    DisablePluginRequest, LibraryKind, ListPluginLogLinesRequest, PluginLogLevel, PluginLogLine,
    PluginServiceClient,
};
use nama_server::plugins::Registry;
use server::{TestServer, WAIT};

/// A Built-in Plugin that logs as it starts. Its lines name the target its crate would have, as a
/// Built-in's package is `nama-plugin-<folder>`: the Plugins page keeps no other crate's.
struct Chatty;

impl Plugin for Chatty {
    fn manifest(&self) -> Manifest {
        Manifest::new(PluginId::from_static("chatty"))
    }

    fn register(&self, _registrar: &mut Registrar) -> Result<(), PluginError> {
        tracing::warn!(target: "nama_plugin_chatty", "chatty is starting");
        Ok(())
    }
}

/// A Built-in Plugin that logs as it starts, then refuses to.
struct Refusing;

impl Plugin for Refusing {
    fn manifest(&self) -> Manifest {
        Manifest::new(PluginId::from_static("refusing"))
    }

    fn register(&self, _registrar: &mut Registrar) -> Result<(), PluginError> {
        tracing::warn!(target: "nama_plugin_refusing", "refusing is starting");
        Err("no thanks".into())
    }
}

fn plugins(server: &TestServer) -> PluginServiceClient<HttpClient> {
    PluginServiceClient::new(HttpClient::plaintext(), server.client_config())
}

async fn log_lines(
    client: &PluginServiceClient<HttpClient>,
    id: &str,
) -> Result<Vec<PluginLogLine>, ConnectError> {
    let request = ListPluginLogLinesRequest {
        id: id.to_owned(),
        ..Default::default()
    };
    Ok(client
        .list_plugin_log_lines(request)
        .await?
        .into_owned()
        .lines)
}

fn texts(lines: &[PluginLogLine]) -> Vec<&str> {
    lines.iter().map(|line| line.text.as_str()).collect()
}

/// Waits until the Plugin `id` has logged a line containing `needle`, and returns its lines.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn wait_for_line(
    client: &PluginServiceClient<HttpClient>,
    id: &str,
    needle: &str,
) -> Vec<PluginLogLine> {
    let started = Instant::now();
    loop {
        let lines = log_lines(client, id).await.unwrap();
        if lines.iter().any(|line| line.text.contains(needle)) {
            return lines;
        }
        assert!(
            started.elapsed() < WAIT,
            "{id} never logged {needle:?}: {lines:?}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn an_admin_reads_a_builtin_plugins_lines_but_not_the_servers_own() {
    Log::global();
    let registry = Registry::new();
    registry.enable(Arc::new(Chatty)).unwrap();
    let server = TestServer::builder().plugins(registry).start().await;

    let lines = log_lines(&plugins(&server), "chatty").await.unwrap();

    assert_eq!(texts(&lines), ["chatty is starting"]);
    let line = lines.first().unwrap();
    assert_eq!(line.level, PluginLogLevel::PLUGIN_LOG_LEVEL_WARN);
    assert!(line.at_ms > 0, "{line:?}");
}

#[tokio::test]
async fn a_plugin_that_refused_to_start_has_no_lines() {
    Log::global();
    let registry = Registry::new();
    registry.enable(Arc::new(Refusing)).unwrap_err();
    let server = TestServer::builder().plugins(registry).start().await;

    assert_eq!(log_lines(&plugins(&server), "refusing").await.unwrap(), []);
}

#[tokio::test]
async fn an_installed_plugins_lines_come_newest_first_and_go_when_it_is_disabled() {
    Log::global();
    let server = TestServer::builder()
        .plugins(Registry::with_builtins())
        .start()
        .await;
    server.enable_test_plugin("").await;
    let client = plugins(&server);

    server
        .library(LibraryKind::LIBRARY_KIND_MOVIES, &["Heat (1995).mkv"])
        .await;
    // The two Scan events may reach the Plugin in either order (`ExtensionPoints::notify`).
    wait_for_line(&client, "test-plugin", "ScanStarted").await;
    let lines = wait_for_line(&client, "test-plugin", "ScanFinished").await;

    let observed: Vec<&str> = texts(&lines);
    assert!(
        observed
            .iter()
            .all(|text| text.starts_with("test-plugin observed")),
        "{observed:?}"
    );
    assert!(
        lines.windows(2).all(|pair| pair[0].at_ms >= pair[1].at_ms),
        "{lines:?}"
    );
    assert!(
        lines
            .iter()
            .all(|line| line.level == PluginLogLevel::PLUGIN_LOG_LEVEL_INFO)
    );

    let disable = DisablePluginRequest {
        id: "test-plugin".to_owned(),
        ..Default::default()
    };
    client.disable_plugin(disable).await.unwrap();
    assert_eq!(log_lines(&client, "test-plugin").await.unwrap(), []);
}
