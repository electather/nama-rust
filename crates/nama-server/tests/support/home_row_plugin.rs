//! The test Plugin built as a Home row source, installed, enabled and given a User's `handle`
//! through the API.

use connectrpc::client::HttpClient;
use nama_proto::nama::v1::{
    DisablePluginRequest, EnablePluginRequest, ListPluginsRequest, PluginServiceClient,
    SettingValue, UpdatePersonalSettingsRequest, UserServiceClient, setting_value::Value,
};

use crate::{
    server::{SignedIn, TestServer},
    test_plugin,
};

/// The test Plugin's id.
pub(crate) const PLUGIN: &str = "test-plugin";

fn plugins(server: &TestServer) -> PluginServiceClient<HttpClient> {
    PluginServiceClient::new(HttpClient::plaintext(), server.client_config())
}

/// Installs the test Plugin, built as a Home row source, on `server`, not yet enabled.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
pub(crate) async fn install(server: &TestServer) {
    let dir = server.data_dir().join("plugins");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("test.wasm"), test_plugin::build("home-rows")).unwrap();
    // Listing is what reads `plugins/`.
    (plugins(server))
        .list_plugins(ListPluginsRequest::default())
        .await
        .unwrap();
}

/// Enables the test Plugin [`install`] installed.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
pub(crate) async fn enable(server: &TestServer) {
    (plugins(server))
        .enable_plugin(EnablePluginRequest {
            id: PLUGIN.to_owned(),
            ..Default::default()
        })
        .await
        .unwrap();
}

/// Disables the test Plugin.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
pub(crate) async fn disable(server: &TestServer) {
    (plugins(server))
        .disable_plugin(DisablePluginRequest {
            id: PLUGIN.to_owned(),
            ..Default::default()
        })
        .await
        .unwrap();
}

/// A text setting's value, `text`.
pub(crate) fn text(text: &str) -> SettingValue {
    SettingValue {
        value: Some(Value::TextValue(text.to_owned())),
        ..Default::default()
    }
}

/// Saves `handle` as `user`'s `handle` Personal setting for the test Plugin, which gives them the
/// row `for-<handle>` and suggests titles for their Featured.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
pub(crate) async fn set_handle(user: &SignedIn, handle: &str) {
    UserServiceClient::new(HttpClient::plaintext(), user.client_config())
        .update_personal_settings(UpdatePersonalSettingsRequest {
            plugin_id: PLUGIN.to_owned(),
            settings: std::iter::once(("handle".to_owned(), text(handle))).collect(),
            ..Default::default()
        })
        .await
        .unwrap();
}
