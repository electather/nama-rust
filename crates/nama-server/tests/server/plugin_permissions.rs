//! Plugin Permissions over the API: those a Plugin's Manifest requests are listed, and an Admin
//! grants some of them when enabling it, kept across a restart (ADR 0049), while a Built-in holds
//! them all (ADR 0079), and they alone decide which Kernel functions any Plugin may call (ADR
//! 0071).

#[path = "support/installed_test_plugin.rs"]
#[expect(
    dead_code,
    reason = "this binary enables the test Plugin itself, granting Permissions"
)]
mod installed_test_plugin;
#[path = "support/log.rs"]
mod log;
#[path = "support/refused.rs"]
mod refused;
#[path = "support/server.rs"]
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
mod server;
#[path = "support/test_plugin.rs"]
mod test_plugin;

use std::time::{Duration, Instant};

use buffa::EnumValue;
use connectrpc::{ErrorCode, client::HttpClient};
use log::Log;
use nama_proto::nama::v1::{
    DisablePluginRequest, EnablePluginRequest, ExtensionPoint, GetPluginRequest, LibraryKind,
    ListPluginsRequest, PluginPermission, PluginServiceClient, PluginSummary, RefusalReason,
};
use nama_server::plugins::Registry;
use refused::assert_refused;
use server::{TestServer, WAIT};

const STORE: PluginPermission = PluginPermission::PLUGIN_PERMISSION_STORE;
const READ_LIBRARIES: PluginPermission = PluginPermission::PLUGIN_PERMISSION_READ_LIBRARIES;
const DELETE_MEDIA: PluginPermission = PluginPermission::PLUGIN_PERMISSION_DELETE_MEDIA;

/// A Server with the test Plugin, built with `features`, installed. With `automation` it requests
/// the Store and Library Permissions, with `home-rows` the Store alone.
async fn with_test_plugin(features: &str) -> TestServer {
    let server = TestServer::builder()
        .plugins(Registry::with_builtins())
        .start()
        .await;
    server.install_test_plugin(features).await;
    server
}

fn client(server: &TestServer) -> PluginServiceClient<HttpClient> {
    PluginServiceClient::new(HttpClient::plaintext(), server.client_config())
}

/// The test Plugin as `ListPlugins` lists it.
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
        .find(|summary| summary.id == "test-plugin")
        .unwrap()
}

/// Why the test Plugin's Job `kernel` last failed, as `GetPlugin` shows it, once a Run has. Each
/// look moves the clock a minute, so the Job, due every minute, fires while it waits.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn job_failure(server: &TestServer) -> String {
    let request = GetPluginRequest {
        id: "test-plugin".to_owned(),
        ..Default::default()
    };
    let automation = EnumValue::from(ExtensionPoint::EXTENSION_POINT_AUTOMATION);
    let waiting = Instant::now();
    loop {
        server.advance_clock(Duration::from_mins(1));
        let got = client(server).get_plugin(request.clone()).await.unwrap();
        let plugin = got.into_owned().plugin.into_option().unwrap();
        if let Some(failure) = plugin
            .failures
            .into_iter()
            .find(|it| it.point == automation)
        {
            return failure.last_error;
        }
        assert!(
            waiting.elapsed() < WAIT,
            "the test Plugin's Job never failed"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// Moves the clock a minute at a time, as [`job_failure`] does, until `log` has `needle`.
async fn job_logged(server: &TestServer, log: &Log, needle: &str) {
    let waiting = Instant::now();
    while log.count(&[needle]) == 0 {
        assert!(
            waiting.elapsed() < WAIT,
            "the test Plugin never logged {needle:?}"
        );
        server.advance_clock(Duration::from_mins(1));
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

fn enable_granting(granted: &[PluginPermission]) -> EnablePluginRequest {
    EnablePluginRequest {
        id: "test-plugin".to_owned(),
        granted_permissions: granted.iter().map(|&it| it.into()).collect(),
        ..Default::default()
    }
}

fn values(permissions: &[PluginPermission]) -> Vec<EnumValue<PluginPermission>> {
    permissions.iter().map(|&it| it.into()).collect()
}

#[tokio::test]
async fn listing_shows_the_permissions_a_plugin_requests_and_none_granted() {
    let server = with_test_plugin("automation").await;

    let summary = listed(&server).await;

    assert_eq!(
        summary.requested_permissions,
        values(&[STORE, READ_LIBRARIES])
    );
    assert_eq!(summary.granted_permissions, values(&[]));
}

#[tokio::test]
async fn an_automation_plugin_is_listed_at_the_automation_point() {
    let server = with_test_plugin("automation").await;

    let summary = listed(&server).await;

    let automation = EnumValue::from(ExtensionPoint::EXTENSION_POINT_AUTOMATION);
    assert!(
        summary.extension_points.contains(&automation),
        "{:?}",
        summary.extension_points
    );
}

#[tokio::test]
async fn enabling_with_a_requested_permission_grants_it() {
    let server = with_test_plugin("automation").await;

    client(&server)
        .enable_plugin(enable_granting(&[STORE]))
        .await
        .unwrap();

    let summary = listed(&server).await;
    assert!(summary.enabled);
    assert_eq!(summary.granted_permissions, values(&[STORE]));
}

#[tokio::test]
async fn granting_a_permission_the_plugin_does_not_request_is_refused() {
    let server = with_test_plugin("automation").await;

    let refusal = assert_refused(
        client(&server)
            .enable_plugin(enable_granting(&[STORE, DELETE_MEDIA]))
            .await,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_PERMISSION_NOT_REQUESTED,
    );

    assert_eq!(refusal.field, "granted_permissions");
    assert_eq!(
        refusal.params.get("permission").map(String::as_str),
        Some("PLUGIN_PERMISSION_DELETE_MEDIA")
    );
    let summary = listed(&server).await;
    assert!(!summary.enabled);
    assert_eq!(summary.granted_permissions, values(&[]));
}

#[tokio::test]
async fn a_restart_keeps_the_grants() {
    let server = with_test_plugin("automation").await;
    client(&server)
        .enable_plugin(enable_granting(&[STORE, READ_LIBRARIES]))
        .await
        .unwrap();

    let server = TestServer::builder()
        .plugins(Registry::with_builtins())
        .restart(server)
        .await;

    let summary = listed(&server).await;
    assert!(summary.enabled);
    assert_eq!(
        summary.granted_permissions,
        values(&[STORE, READ_LIBRARIES])
    );
}

#[tokio::test]
async fn enabling_again_with_none_clears_the_grants() {
    let server = with_test_plugin("automation").await;
    client(&server)
        .enable_plugin(enable_granting(&[STORE]))
        .await
        .unwrap();

    client(&server)
        .enable_plugin(enable_granting(&[]))
        .await
        .unwrap();

    let summary = listed(&server).await;
    assert!(summary.enabled);
    assert_eq!(summary.granted_permissions, values(&[]));
}

#[tokio::test]
async fn enabling_a_builtin_plugin_granting_none_still_grants_all_it_requests() {
    let server = TestServer::builder()
        .plugins(Registry::with_builtins())
        .start()
        .await;
    let disable = DisablePluginRequest {
        id: "upkeep".to_owned(),
        ..Default::default()
    };
    client(&server).disable_plugin(disable).await.unwrap();

    let enable = EnablePluginRequest {
        id: "upkeep".to_owned(),
        ..Default::default()
    };
    client(&server).enable_plugin(enable).await.unwrap();

    let summaries = client(&server)
        .list_plugins(ListPluginsRequest::default())
        .await
        .unwrap()
        .into_owned()
        .summaries;
    let upkeep = summaries.iter().find(|it| it.id == "upkeep").unwrap();
    assert!(upkeep.enabled);
    assert_ne!(upkeep.requested_permissions, values(&[]));
    assert_eq!(upkeep.granted_permissions, upkeep.requested_permissions);
}

#[tokio::test]
async fn changing_the_grants_of_a_running_plugin_reaches_the_kernel() {
    let log = Log::global();
    let server = TestServer::builder()
        .plugins(Registry::with_builtins())
        .moves_clock()
        .start()
        .await;
    server.install_test_plugin("automation").await;
    // The log is the binary's, and only this test's Server has a Library for its Plugin to see.
    server
        .create_library(LibraryKind::LIBRARY_KIND_MOVIES, &[])
        .await;
    client(&server)
        .enable_plugin(enable_granting(&[STORE]))
        .await
        .unwrap();
    let refused = job_failure(&server).await;
    assert!(
        refused.contains("Read Libraries Permission"),
        "the Job failed for another reason: {refused}"
    );

    client(&server)
        .enable_plugin(enable_granting(&[STORE, READ_LIBRARIES]))
        .await
        .unwrap();

    job_logged(&server, log, "test-plugin sees 1 Libraries").await;
}

#[tokio::test]
async fn a_plugin_not_naming_the_automation_point_reads_back_what_it_put_in_its_granted_store() {
    let log = Log::global();
    let server = with_test_plugin("home-rows").await;

    // Enabling asks the Home row source for its shared rows, which go through its Store.
    client(&server)
        .enable_plugin(enable_granting(&[STORE]))
        .await
        .unwrap();

    log.wait_for(&["test-plugin's Store keeps Some(\"rows\")"], 1)
        .await;
}

#[tokio::test]
async fn a_plugin_not_naming_the_automation_point_is_refused_a_store_not_granted() {
    let log = Log::global();
    let server = with_test_plugin("home-rows").await;

    client(&server)
        .enable_plugin(enable_granting(&[]))
        .await
        .unwrap();

    log.wait_for(
        &[
            "test-plugin's Store refused",
            "hasn't granted test-plugin the Store Permission",
        ],
        1,
    )
    .await;
}
