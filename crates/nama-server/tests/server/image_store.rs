//! Where the stored images live: local disk by default, and the Admin's switch to another store
//! with `SwitchImageStore` (ADR 0052), over a real socket.

use crate::support::{refused, server};

use connectrpc::{ConnectError, ErrorCode, client::ClientConfig, client::HttpClient};
use nama_proto::nama::v1::{
    GetImageSettingsRequest, GetImageSettingsResponse, Permission, RefusalReason,
    ServerServiceClient, SwitchImageStoreRequest, SwitchImageStoreResponse,
};
use refused::assert_refused;
use server::TestServer;

fn client(who: ClientConfig) -> ServerServiceClient<HttpClient> {
    ServerServiceClient::new(HttpClient::plaintext(), who)
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn image_settings(server: &TestServer) -> GetImageSettingsResponse {
    client(server.client_config())
        .get_image_settings(GetImageSettingsRequest::default())
        .await
        .unwrap()
        .into_owned()
}

async fn switch_as(
    who: ClientConfig,
    plugin_id: &str,
) -> Result<SwitchImageStoreResponse, ConnectError> {
    let request = SwitchImageStoreRequest {
        plugin_id: plugin_id.to_owned(),
        ..Default::default()
    };
    let response = client(who).switch_image_store(request).await?;
    Ok(response.into_owned())
}

#[tokio::test]
async fn the_images_are_on_local_disk_with_no_switch_until_one_is_asked_for() {
    let server = TestServer::builder().start().await;

    let settings = image_settings(&server).await;

    assert_eq!(settings.settings.store_plugin_id, "");
    assert!(settings.switching.is_unset(), "{settings:?}");
}

#[tokio::test]
async fn switching_to_a_plugin_that_keeps_no_blobs_is_refused_naming_it() {
    let server = TestServer::builder().start().await;

    // An id no Plugin has, and `local-auth`, enabled but serving no blob store.
    for id in ["no-such-plugin", "local-auth"] {
        let refused = switch_as(server.client_config(), id).await;

        let refusal = assert_refused(
            refused,
            ErrorCode::FailedPrecondition,
            RefusalReason::REFUSAL_REASON_NOT_A_BLOB_STORE,
        );
        assert_eq!(refusal.field, "plugin_id");
        assert_eq!(refusal.params.get("pluginId").map(String::as_str), Some(id));
    }
    let settings = image_settings(&server).await;
    assert_eq!(settings.settings.store_plugin_id, "");
    assert!(settings.switching.is_unset(), "{settings:?}");
}

#[tokio::test]
async fn switching_to_local_disk_while_on_it_is_refused_as_nothing_to_do() {
    let server = TestServer::builder().start().await;

    let refused = switch_as(server.client_config(), "").await;

    let refusal = assert_refused(
        refused,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_IMAGE_STORE_IN_USE,
    );
    assert_eq!(refusal.field, "plugin_id");
    assert!(image_settings(&server).await.switching.is_unset());
}

#[tokio::test]
async fn switching_the_image_store_needs_manage_settings() {
    let server = TestServer::builder().start().await;
    let grace = server.user("Grace", &[Permission::ManageLibraries]).await;
    let ada = server.user("Ada", &[Permission::ManageSettings]).await;

    let refused = switch_as(grace.client_config(), "").await;
    let allowed = switch_as(ada.client_config(), "").await;

    let refusal = assert_refused(
        refused,
        ErrorCode::PermissionDenied,
        RefusalReason::REFUSAL_REASON_PERMISSION_REQUIRED,
    );
    assert_eq!(
        refusal.params.get("permission").map(String::as_str),
        Some("PERMISSION_MANAGE_SETTINGS")
    );
    // Past the access check, to the handler's own refusal.
    assert_refused(
        allowed,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_IMAGE_STORE_IN_USE,
    );
}
