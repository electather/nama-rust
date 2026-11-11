//! Helpers the Radarr and Sonarr tests share, as their Plugins share Servarr's settings and
//! progress.

use std::sync::Arc;

use connectrpc::{ConnectError, client::HttpClient};
use nama_plugin_api::fulfiller::Fulfiller;
use nama_proto::nama::v1::{
    PluginServiceClient, SettingValue, TestPluginSettingsRequest, TestPluginSettingsResponse,
    setting_value::Value,
};
use nama_server::plugins::Registry;

use crate::server::TestServer;

/// A Server whose Admin sets up a Servarr Plugin against a fake Servarr.
pub(crate) trait ServarrFixture {
    /// The Plugin's id, such as `radarr`.
    const PLUGIN: &'static str;
    /// The kind of title its Fulfiller is registered under.
    type Kind: 'static;

    /// The Server, called as its Admin.
    fn server(&self) -> &TestServer;
    /// The Server's Plugins, to ask the Fulfiller directly.
    fn registry(&self) -> &Registry;
    /// The fake Servarr's URL.
    fn servarr_url(&self) -> String;

    /// The Plugin service, as the Admin calls it.
    fn plugins(&self) -> PluginServiceClient<HttpClient> {
        PluginServiceClient::new(HttpClient::plaintext(), self.server().client_config())
    }

    /// The fake Servarr's URL and `api_key` as the Plugin's settings.
    fn settings(&self, api_key: &str) -> Vec<(String, SettingValue)> {
        vec![
            text(("url", &self.servarr_url())),
            text(("api_key", api_key)),
        ]
    }

    /// Tests the Plugin's settings with the fake Servarr's URL and `api_key`, as the Plugins
    /// page's "Test" does.
    async fn test_settings(
        &self,
        api_key: &str,
    ) -> Result<TestPluginSettingsResponse, ConnectError> {
        let request = TestPluginSettingsRequest {
            id: Self::PLUGIN.to_owned(),
            settings: self.settings(api_key).into_iter().collect(),
            ..Default::default()
        };
        Ok(self
            .plugins()
            .test_plugin_settings(request)
            .await?
            .into_owned())
    }

    /// Asks the Plugin's Fulfiller what it is downloading for `references`, with its error as
    /// Users would see it.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    async fn progress(&self, references: &[String]) -> Result<Vec<String>, String> {
        self.registry()
            .ask_first(|fulfiller: Arc<dyn Fulfiller<Self::Kind>>| async move {
                fulfiller.progress(references).await
            })
            .await
            .unwrap()
            .result
            .map(|by_reference| by_reference.into_keys().collect())
            .map_err(|failure| failure.message)
    }
}

/// `(key, value)` as the Plugins page sends a text setting.
pub(crate) fn text((key, value): (&str, &str)) -> (String, SettingValue) {
    let value = SettingValue {
        value: Some(Value::TextValue(value.to_owned())),
        ..Default::default()
    };
    (key.to_owned(), value)
}

/// The `(value, label)` of each choice `tested` offers for the setting `key`.
pub(crate) fn choices<'a>(
    tested: &'a TestPluginSettingsResponse,
    key: &str,
) -> Vec<(&'a str, &'a str)> {
    tested.choices.get(key).map_or_else(Vec::new, |offered| {
        offered
            .choices
            .iter()
            .map(|choice| (choice.value.as_str(), choice.label.as_str()))
            .collect()
    })
}
