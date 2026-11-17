//! Clearing a disabled Plugin's required settings over the API: required-ness is checked when it
//! is enabled, not when its settings are saved.

use std::sync::Arc;

use connectrpc::{ConnectError, ErrorCode, client::HttpClient};
use nama_plugin_api::{
    Manifest, Plugin, PluginError, PluginId, Registrar,
    settings::{SettingField, SettingKind, SettingsSchema},
};
use nama_proto::nama::v1::{
    DisablePluginRequest, EnablePluginRequest, GetPluginRequest, ListPluginsRequest,
    PluginServiceClient, PluginSummary, RefusalReason, SettingValue, UpdatePluginSettingsRequest,
    UpdatePluginSettingsResponse, setting_value::Value,
};
use nama_server::plugins::Registry;

use crate::{refused::assert_refused, server::TestServer};

/// The Built-in Plugin `bucketer`, with the required text settings `endpoint` and `bucket`.
struct Bucketer;

impl Plugin for Bucketer {
    fn manifest(&self) -> Manifest {
        let required = |key: &str| SettingField {
            key: key.to_owned(),
            label: key.to_owned(),
            description: String::new(),
            kind: SettingKind::Text,
            required: true,
            default: None,
            min: None,
            max: None,
        };
        Manifest {
            version: "0.0.0".to_owned(),
            settings: SettingsSchema {
                fields: vec![required("endpoint"), required("bucket")],
            },
            ..Manifest::new(PluginId::from_static("bucketer"))
        }
    }

    fn register(&self, _registrar: &mut Registrar) -> Result<(), PluginError> {
        Ok(())
    }
}

fn client(server: &TestServer) -> PluginServiceClient<HttpClient> {
    PluginServiceClient::new(HttpClient::plaintext(), server.client_config())
}

/// A server with `bucketer` saved with both its settings, then disabled, as the Admin.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn with_bucketer_set_up_and_disabled() -> TestServer {
    let plugins = Registry::new();
    plugins.describe_builtin(|| Arc::new(Bucketer));
    let server = TestServer::builder().plugins(plugins).start().await;
    let set_up = [
        ("endpoint", text("https://s3.example")),
        ("bucket", text("media")),
    ];
    update(&server, &set_up).await.unwrap();
    let disable = DisablePluginRequest {
        id: "bucketer".to_owned(),
        ..Default::default()
    };
    client(&server).disable_plugin(disable).await.unwrap();
    server
}

fn text(value: &str) -> SettingValue {
    SettingValue {
        value: Some(Value::TextValue(value.to_owned())),
        ..Default::default()
    }
}

/// Sets `bucketer`'s settings to `settings`, as the Admin.
async fn update(
    server: &TestServer,
    settings: &[(&str, SettingValue)],
) -> Result<UpdatePluginSettingsResponse, ConnectError> {
    let request = UpdatePluginSettingsRequest {
        id: "bucketer".to_owned(),
        settings: (settings.iter())
            .map(|(key, value)| ((*key).to_owned(), value.clone()))
            .collect(),
        ..Default::default()
    };
    client(server)
        .update_plugin_settings(request)
        .await
        .map(connectrpc::UnaryResponse::into_owned)
}

/// The text values of `bucketer`'s settings that have one, as `GetPlugin` gets them.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn stored_texts(server: &TestServer) -> Vec<String> {
    let request = GetPluginRequest {
        id: "bucketer".to_owned(),
        ..Default::default()
    };
    let response = client(server)
        .get_plugin(request)
        .await
        .unwrap()
        .into_owned();
    let plugin = response.plugin.into_option().unwrap();
    (plugin.settings.into_iter())
        .filter_map(|setting| match setting.value.into_option()?.value? {
            Value::TextValue(text) => Some(text),
            Value::BoolValue(_) | Value::IntegerValue(_) | Value::LibraryIds(_) => None,
        })
        .collect()
}

/// `bucketer`'s summary, as `ListPlugins` gets it.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn summary(server: &TestServer) -> PluginSummary {
    let response = client(server)
        .list_plugins(ListPluginsRequest::default())
        .await
        .unwrap()
        .into_owned();
    (response.summaries.into_iter())
        .find(|summary| summary.id == "bucketer")
        .unwrap()
}

#[tokio::test]
async fn a_disabled_plugins_required_settings_can_be_sent_blank_or_left_out() {
    let server = with_bucketer_set_up_and_disabled().await;

    update(&server, &[("endpoint", text(""))]).await.unwrap();

    assert_eq!(stored_texts(&server).await, Vec::<String>::new());
    assert!(summary(&server).await.settings_missing);
}

#[tokio::test]
async fn a_cleared_disabled_plugin_is_refused_when_enabled_naming_the_setting() {
    let server = with_bucketer_set_up_and_disabled().await;
    update(&server, &[]).await.unwrap();

    let enable = EnablePluginRequest {
        id: "bucketer".to_owned(),
        ..Default::default()
    };
    let refusal = assert_refused(
        client(&server).enable_plugin(enable).await,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
    );

    assert_eq!(refusal.field, "settings.endpoint");
    assert!(!summary(&server).await.enabled);
}

#[tokio::test]
async fn a_disabled_plugin_sent_a_value_of_the_wrong_kind_is_refused_and_nothing_changes() {
    let server = with_bucketer_set_up_and_disabled().await;
    let number = SettingValue {
        value: Some(Value::IntegerValue(7)),
        ..Default::default()
    };

    let refusal = assert_refused(
        update(&server, &[("endpoint", number)]).await,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_SETTING_WRONG_KIND,
    );

    assert_eq!(refusal.field, "settings.endpoint");
    assert_eq!(stored_texts(&server).await, ["https://s3.example", "media"]);
}
