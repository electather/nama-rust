//! Plugin Permissions over the API: those a Plugin's Manifest requests are listed, and an Admin
//! grants some of them when enabling it, kept across a restart (ADR 0049).

#[path = "support/refused.rs"]
mod refused;
#[path = "support/server.rs"]
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
mod server;
#[path = "support/test_plugin.rs"]
mod test_plugin;

use buffa::EnumValue;
use connectrpc::{ErrorCode, client::HttpClient};
use nama_proto::nama::v1::{
    EnablePluginRequest, ListPluginsRequest, PluginPermission, PluginServiceClient, PluginSummary,
    RefusalReason,
};
use nama_server::plugins::Registry;
use refused::assert_refused;
use server::TestServer;

const STORE: PluginPermission = PluginPermission::PLUGIN_PERMISSION_STORE;
const READ_LIBRARIES: PluginPermission = PluginPermission::PLUGIN_PERMISSION_READ_LIBRARIES;
const DELETE_MEDIA: PluginPermission = PluginPermission::PLUGIN_PERMISSION_DELETE_MEDIA;

/// A Server with the test Plugin, built to request the Store and Library Permissions, dropped in.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn with_automation_plugin() -> TestServer {
    let server = TestServer::builder()
        .plugins(Registry::with_builtins())
        .start()
        .await;
    let dir = server.data_dir().join("plugins");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("test.wasm"), test_plugin::build("automation")).unwrap();
    server
}

fn client(server: &TestServer) -> PluginServiceClient<HttpClient> {
    PluginServiceClient::new(HttpClient::plaintext(), server.client_config())
}

/// The test Plugin as `ListPlugins` lists it, which also reads `plugins/`.
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
    let server = with_automation_plugin().await;

    let summary = listed(&server).await;

    assert_eq!(
        summary.requested_permissions,
        values(&[STORE, READ_LIBRARIES])
    );
    assert_eq!(summary.granted_permissions, values(&[]));
}

#[tokio::test]
async fn enabling_with_a_requested_permission_grants_it() {
    let server = with_automation_plugin().await;
    listed(&server).await;

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
    let server = with_automation_plugin().await;
    listed(&server).await;

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
    let server = with_automation_plugin().await;
    listed(&server).await;
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
    let server = with_automation_plugin().await;
    listed(&server).await;
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
async fn changing_the_grants_of_a_running_plugin_reaches_the_kernel() {
    let server = with_automation_plugin().await;
    listed(&server).await;
    client(&server)
        .enable_plugin(enable_granting(&[STORE]))
        .await
        .unwrap();
    let refused = server.tick("test-plugin").await.unwrap_err();
    assert!(
        refused.contains("Read Libraries Permission"),
        "the tick failed for another reason: {refused}"
    );

    client(&server)
        .enable_plugin(enable_granting(&[STORE, READ_LIBRARIES]))
        .await
        .unwrap();

    server.tick("test-plugin").await.unwrap();
}
