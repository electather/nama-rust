//! Listing the Server's Plugins and their settings, without ever sending a secret's value, over a
//! real socket and against a real SQLite database.

#[path = "support/server.rs"]
#[expect(dead_code, reason = "the harness's plain `start` has no use here")]
mod server;

use std::collections::BTreeMap;

use buffa::Message;
use connectrpc::client::HttpClient;
use nama_plugin_api::settings::SettingValue;
use nama_proto::nama::v1::{
    ListPluginsRequest, ListPluginsResponse, Plugin, PluginKind, PluginServiceClient, Setting,
    SettingKind, setting_value::Value,
};
use nama_server::{
    config::{Config, PluginConfig},
    plugin_settings,
    plugins::Registry,
};
use server::TestServer;

const TMDB_KEY: &str = "tmdb-key-that-must-stay-on-the-server";

/// Starts a Server whose Built-in Plugins have `settings`, stored as an upgrade imports them, and
/// lists its Plugins as the Admin.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn list_plugins_with(settings: BTreeMap<String, PluginConfig>) -> ListPluginsResponse {
    let plugins = Registry::with_builtins(&settings);
    let (server, db) = TestServer::start_with_db(Config::default(), plugins).await;
    plugin_settings::import(&db, &settings).await.unwrap();

    PluginServiceClient::new(HttpClient::plaintext(), server.client_config())
        .list_plugins(ListPluginsRequest::default())
        .await
        .unwrap()
        .into_owned()
}

fn tmdb_with_key() -> BTreeMap<String, PluginConfig> {
    let tmdb = PluginConfig {
        settings: BTreeMap::from([(
            "api_key".to_owned(),
            SettingValue::Text(TMDB_KEY.to_owned()),
        )]),
        ..PluginConfig::default()
    };
    BTreeMap::from([("tmdb".to_owned(), tmdb)])
}

#[expect(
    clippy::expect_used,
    reason = "a test helper fails its test by panicking"
)]
fn plugin<'a>(listed: &'a ListPluginsResponse, id: &str) -> &'a Plugin {
    listed
        .plugins
        .iter()
        .find(|plugin| plugin.id == id)
        .expect("the Plugin is listed")
}

#[expect(
    clippy::expect_used,
    reason = "a test helper fails its test by panicking"
)]
fn setting<'a>(plugin: &'a Plugin, key: &str) -> &'a Setting {
    plugin
        .settings
        .iter()
        .find(|setting| setting.key == key)
        .expect("the Plugin has the setting")
}

#[tokio::test]
async fn every_builtin_plugin_is_listed_with_whether_it_is_enabled() {
    let listed = list_plugins_with(BTreeMap::new()).await;

    let ids: Vec<_> = listed
        .plugins
        .iter()
        .map(|plugin| plugin.id.as_str())
        .collect();
    assert_eq!(ids, ["local-auth", "log-events", "tmdb"]);
    for plugin in &listed.plugins {
        assert_eq!(plugin.kind, PluginKind::PLUGIN_KIND_BUILT_IN, "{plugin:?}");
        assert!(!plugin.version.is_empty(), "{plugin:?}");
    }
    assert!(plugin(&listed, "local-auth").enabled);
    // TMDB can't run without an API key.
    assert!(!plugin(&listed, "tmdb").enabled);
}

#[tokio::test]
async fn a_plugins_settings_come_with_their_schema_and_stored_values() {
    let mut settings = tmdb_with_key();
    if let Some(tmdb) = settings.get_mut("tmdb") {
        tmdb.settings.insert(
            "language".to_owned(),
            SettingValue::Text("de-DE".to_owned()),
        );
    }

    let listed = list_plugins_with(settings).await;

    let tmdb = plugin(&listed, "tmdb");
    assert!(tmdb.enabled);
    let api_key = setting(tmdb, "api_key");
    assert_eq!(api_key.kind, SettingKind::SETTING_KIND_SECRET);
    assert!(api_key.required);
    assert_eq!(api_key.label, "API key");
    assert!(api_key.secret_set);
    let language = setting(tmdb, "language");
    assert_eq!(language.kind, SettingKind::SETTING_KIND_TEXT);
    assert!(!language.secret_set);
    assert_eq!(
        language.value.as_option().and_then(|v| v.value.clone()),
        Some(Value::TextValue("de-DE".to_owned()))
    );
    assert_eq!(
        language
            .default_value
            .as_option()
            .and_then(|v| v.value.clone()),
        Some(Value::TextValue("en-US".to_owned()))
    );
}

#[tokio::test]
async fn a_secret_set_is_only_said_to_be_set() {
    let listed = list_plugins_with(tmdb_with_key()).await;

    // Encoded again, the response holds everything the Server sent, fields this client doesn't
    // know included.
    let sent = listed.encode_to_vec();
    assert!(
        !sent
            .windows(TMDB_KEY.len())
            .any(|bytes| bytes == TMDB_KEY.as_bytes()),
        "{listed:?}"
    );
    let api_key = setting(plugin(&listed, "tmdb"), "api_key");
    assert!(api_key.secret_set);
    assert!(api_key.value.is_unset(), "{api_key:?}");
}

#[tokio::test]
async fn a_secret_not_set_is_said_to_be_not_set() {
    let listed = list_plugins_with(BTreeMap::new()).await;

    let api_key = setting(plugin(&listed, "tmdb"), "api_key");
    assert!(!api_key.secret_set);
}
