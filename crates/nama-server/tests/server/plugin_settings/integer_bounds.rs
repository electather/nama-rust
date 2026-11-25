//! Saving an integer setting against the bounds its schema declares, over the API, with the
//! bundled Cleanup's `days`, which may be no less than 0.

use connectrpc::{ConnectError, ErrorCode, client::HttpClient};
use nama_proto::nama::v1::{
    GetPluginRequest, PluginServiceClient, RefusalReason, Setting, SettingValue,
    UpdatePluginSettingsRequest, UpdatePluginSettingsResponse, setting_value::Value,
};

use crate::support::{refused::assert_refused, server::TestServer};

/// Sets the Cleanup's `days` to `days`, as the Admin.
async fn set_days(
    server: &TestServer,
    days: i64,
) -> Result<UpdatePluginSettingsResponse, ConnectError> {
    let days = SettingValue {
        value: Some(Value::IntegerValue(days)),
        ..Default::default()
    };
    let request = UpdatePluginSettingsRequest {
        id: "cleanup".to_owned(),
        settings: std::iter::once(("days".to_owned(), days)).collect(),
        ..Default::default()
    };
    PluginServiceClient::new(HttpClient::plaintext(), server.client_config())
        .update_plugin_settings(request)
        .await
        .map(connectrpc::UnaryResponse::into_owned)
}

/// The Cleanup's `days` setting, as `GetPlugin` gets it as the Admin.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn days_setting(server: &TestServer) -> Setting {
    let request = GetPluginRequest {
        id: "cleanup".to_owned(),
        ..Default::default()
    };
    let response = PluginServiceClient::new(HttpClient::plaintext(), server.client_config())
        .get_plugin(request)
        .await
        .unwrap()
        .into_owned();
    let plugin = response.plugin.into_option().unwrap();
    plugin
        .settings
        .into_iter()
        .find(|setting| setting.key == "days")
        .unwrap()
}

fn stored_days(setting: &Setting) -> Option<i64> {
    match setting.value.as_option()?.value.as_ref()? {
        Value::IntegerValue(days) => Some(*days),
        Value::TextValue(_) | Value::BoolValue(_) | Value::LibraryIds(_) | Value::Texts(_) => None,
    }
}

#[tokio::test]
async fn an_integer_under_its_minimum_is_refused_naming_it_and_saves_nothing() {
    let server = TestServer::builder().start().await;
    set_days(&server, 7).await.unwrap();

    let refused = assert_refused(
        set_days(&server, -5).await,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_SETTING_OUT_OF_RANGE,
    );

    assert_eq!(refused.field, "settings.days");
    assert_eq!(
        refused.params.get("settingKey").map(String::as_str),
        Some("days")
    );
    assert_eq!(refused.params.get("min").map(String::as_str), Some("0"));
    assert_eq!(refused.params.get("max"), None);
    assert_eq!(stored_days(&days_setting(&server).await), Some(7));
}

#[tokio::test]
async fn an_integer_at_its_minimum_saves_and_its_bounds_are_shown() {
    let server = TestServer::builder().start().await;

    set_days(&server, 0).await.unwrap();

    let days = days_setting(&server).await;
    assert_eq!(stored_days(&days), Some(0));
    assert_eq!((days.min_value, days.max_value), (Some(0), None));
}
