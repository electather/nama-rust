//! Saving a Plugin setting that names Libraries by id, over the API (ADR 0053).

use std::sync::Arc;

use connectrpc::{ConnectError, ErrorCode, client::HttpClient};
use nama_plugin_api::{
    Manifest, Plugin, PluginError, PluginId, Registrar,
    settings::{SettingField, SettingKind, SettingsSchema},
};
use nama_proto::nama::v1::{
    GetPluginRequest, LibraryIds, LibraryKind, PluginServiceClient, RefusalReason, SettingValue,
    UpdatePluginSettingsRequest, UpdatePluginSettingsResponse, setting_value::Value,
};
use nama_server::plugins::Registry;

use crate::{refused::assert_refused, server::TestServer};

/// The Built-in Plugin `keeper`, with the Libraries setting `keep`.
struct Keeper;

impl Plugin for Keeper {
    fn manifest(&self) -> Manifest {
        let keep = SettingField {
            key: "keep".to_owned(),
            label: "Keep".to_owned(),
            description: String::new(),
            kind: SettingKind::Libraries,
            required: false,
            default: None,
        };
        Manifest {
            version: "0.0.0".to_owned(),
            settings: SettingsSchema { fields: vec![keep] },
            ..Manifest::new(PluginId::from_static("keeper"))
        }
    }

    fn register(&self, _registrar: &mut Registrar) -> Result<(), PluginError> {
        Ok(())
    }
}

async fn with_keeper() -> TestServer {
    let plugins = Registry::new();
    plugins.describe_builtin(|| Arc::new(Keeper));
    TestServer::builder().plugins(plugins).start().await
}

/// Sets `keeper`'s `keep` to the Libraries `ids`, as the Admin.
async fn keep(
    server: &TestServer,
    ids: &[i64],
) -> Result<UpdatePluginSettingsResponse, ConnectError> {
    let keep = SettingValue {
        value: Some(Value::LibraryIds(Box::new(LibraryIds {
            ids: ids.to_vec(),
            ..Default::default()
        }))),
        ..Default::default()
    };
    let request = UpdatePluginSettingsRequest {
        id: "keeper".to_owned(),
        settings: std::iter::once(("keep".to_owned(), keep)).collect(),
        ..Default::default()
    };
    PluginServiceClient::new(HttpClient::plaintext(), server.client_config())
        .update_plugin_settings(request)
        .await
        .map(connectrpc::UnaryResponse::into_owned)
}

/// The Library ids `keeper`'s `keep` holds, as `GetPlugin` gets them as the Admin; none if it
/// holds nothing.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn kept(server: &TestServer) -> Option<Vec<i64>> {
    let request = GetPluginRequest {
        id: "keeper".to_owned(),
        ..Default::default()
    };
    let response = PluginServiceClient::new(HttpClient::plaintext(), server.client_config())
        .get_plugin(request)
        .await
        .unwrap()
        .into_owned();
    let plugin = response.plugin.as_option()?;
    let keep = plugin
        .settings
        .iter()
        .find(|setting| setting.key == "keep")?;
    match keep.value.as_option()?.value.as_ref()? {
        Value::LibraryIds(libraries) => Some(libraries.ids.clone()),
        Value::TextValue(_) | Value::BoolValue(_) | Value::IntegerValue(_) => None,
    }
}

#[tokio::test]
async fn a_libraries_setting_saves_ids_every_library_has() {
    let server = with_keeper().await;
    let movies = server
        .create_library(LibraryKind::LIBRARY_KIND_MOVIES, &[])
        .await;

    keep(&server, &[movies.id]).await.unwrap();

    assert_eq!(kept(&server).await, Some(vec![movies.id]));
}

#[tokio::test]
async fn a_libraries_setting_naming_an_id_no_library_has_is_refused_and_saves_nothing() {
    let server = with_keeper().await;
    let movies = server
        .create_library(LibraryKind::LIBRARY_KIND_MOVIES, &[])
        .await;
    let unknown = movies.id + 1;

    let refused = assert_refused(
        keep(&server, &[movies.id, unknown]).await,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_LIBRARY_NOT_FOUND,
    );

    assert_eq!(refused.field, "settings.keep");
    assert_eq!(
        refused.params.get("libraryId").map(String::as_str),
        Some(unknown.to_string().as_str())
    );
    assert_eq!(kept(&server).await, None);
}
