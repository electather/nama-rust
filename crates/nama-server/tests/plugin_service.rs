//! Listing the Server's Plugins and their settings, without ever sending a secret's value, and
//! enabling and disabling them without a restart, over a real socket and against a real SQLite
//! database.

#[path = "support/refused.rs"]
mod refused;
#[path = "support/server.rs"]
#[expect(dead_code, reason = "the harness's plain `start` has no use here")]
mod server;

use std::{collections::BTreeMap, sync::Arc};

use buffa::Message;
use connectrpc::{ErrorCode, client::HttpClient};
use nama_plugin_api::{
    PluginId, auth::Authenticator, metadata::MetadataProvider, settings::SettingValue,
};
use nama_proto::nama::v1::{
    DisablePluginRequest, EnablePluginRequest, ListPluginsRequest, ListPluginsResponse, Plugin,
    PluginKind, PluginServiceClient, RefusalReason, Setting, SettingKind, setting_value::Value,
};
use nama_server::{
    config::{Config, PluginConfig},
    db::Database,
    plugin_settings,
    plugins::Registry,
};
use refused::assert_refused;
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

/// A Server whose Built-in Plugins have `settings`, stored as an upgrade imports them, with its
/// database and a handle to its Plugin registry.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn start_with(
    settings: &BTreeMap<String, PluginConfig>,
) -> (TestServer, Arc<Database>, Registry) {
    let plugins = Registry::with_builtins(settings);
    let (server, db) = TestServer::start_with_db(Config::default(), plugins.clone()).await;
    plugin_settings::import(&db, settings).await.unwrap();
    (server, db, plugins)
}

fn client(server: &TestServer) -> PluginServiceClient<HttpClient> {
    PluginServiceClient::new(HttpClient::plaintext(), server.client_config())
}

fn enable(id: &str) -> EnablePluginRequest {
    EnablePluginRequest {
        id: id.to_owned(),
        ..Default::default()
    }
}

fn disable(id: &str) -> DisablePluginRequest {
    DisablePluginRequest {
        id: id.to_owned(),
        ..Default::default()
    }
}

/// Whether some enabled Plugin provides `T`, as Matching looks for a metadata Provider to call.
async fn provides<T: ?Sized + Send + Sync + 'static>(plugins: &Registry) -> bool {
    plugins.call_provider(|_: Arc<T>| async {}).await.is_some()
}

#[tokio::test]
async fn disabling_tmdb_stops_matching_from_calling_it_until_it_is_enabled_again() {
    let (server, _db, plugins) = start_with(&tmdb_with_key()).await;
    assert!(provides::<dyn MetadataProvider>(&plugins).await);

    client(&server)
        .disable_plugin(disable("tmdb"))
        .await
        .unwrap();
    assert!(!provides::<dyn MetadataProvider>(&plugins).await);

    client(&server).enable_plugin(enable("tmdb")).await.unwrap();
    assert!(provides::<dyn MetadataProvider>(&plugins).await);
}

#[tokio::test]
async fn a_plugins_enabled_state_survives_a_restart() {
    let (server, db, _plugins) = start_with(&tmdb_with_key()).await;

    client(&server)
        .disable_plugin(disable("tmdb"))
        .await
        .unwrap();
    client(&server)
        .disable_plugin(disable("log-events"))
        .await
        .unwrap();
    client(&server)
        .enable_plugin(enable("log-events"))
        .await
        .unwrap();

    // A restart enables what the database holds, as `main` does.
    let stored = plugin_settings::stored(&db).await.unwrap().unwrap();
    let restarted = Registry::with_builtins(&stored);
    assert_eq!(
        restarted.enabled(),
        [
            PluginId::from_static("local-auth"),
            PluginId::from_static("log-events")
        ]
    );
    // Disabling kept the settings it was enabled with.
    assert_eq!(stored["tmdb"].settings, tmdb_with_key()["tmdb"].settings);
}

#[tokio::test]
async fn disabling_the_only_authenticator_is_refused_and_it_stays_enabled() {
    let (server, db, plugins) = start_with(&BTreeMap::new()).await;

    let refusal = assert_refused(
        client(&server).disable_plugin(disable("local-auth")).await,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_LAST_AUTHENTICATOR,
    );

    assert_eq!(refusal.field, "id");
    assert_eq!(refusal.params["pluginId"], "local-auth");
    assert!(provides::<dyn Authenticator>(&plugins).await);
    assert_eq!(
        plugin_settings::stored(&db).await.unwrap(),
        Some(BTreeMap::new())
    );
}

#[tokio::test]
async fn enabling_a_plugin_missing_a_required_setting_is_refused_naming_it() {
    let (server, db, plugins) = start_with(&BTreeMap::new()).await;

    let refusal = assert_refused(
        client(&server).enable_plugin(enable("tmdb")).await,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
    );

    assert_eq!(refusal.field, "settings.api_key");
    assert_eq!(refusal.params["settingKey"], "api_key");
    assert!(!provides::<dyn MetadataProvider>(&plugins).await);
    assert_eq!(
        plugin_settings::stored(&db).await.unwrap(),
        Some(BTreeMap::new())
    );
}

#[tokio::test]
async fn enabling_a_plugin_with_a_setting_of_the_wrong_kind_is_refused_naming_it() {
    let tmdb = PluginConfig {
        enabled: false,
        settings: BTreeMap::from([("api_key".to_owned(), SettingValue::Integer(7))]),
    };
    let (server, _db, plugins) = start_with(&BTreeMap::from([("tmdb".to_owned(), tmdb)])).await;

    let refusal = assert_refused(
        client(&server).enable_plugin(enable("tmdb")).await,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_SETTING_WRONG_KIND,
    );

    assert_eq!(refusal.field, "settings.api_key");
    assert_eq!(refusal.params["settingKind"], "SETTING_KIND_SECRET");
    assert!(!provides::<dyn MetadataProvider>(&plugins).await);
}

#[tokio::test]
async fn enabling_or_disabling_an_unknown_plugin_is_refused() {
    let (server, _db, _plugins) = start_with(&BTreeMap::new()).await;

    let refusal = assert_refused(
        client(&server)
            .enable_plugin(enable("no-such-plugin"))
            .await,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_PLUGIN_NOT_FOUND,
    );
    assert_eq!(refusal.field, "id");
    assert_eq!(refusal.params["pluginId"], "no-such-plugin");
    assert_refused(
        client(&server)
            .disable_plugin(disable("no-such-plugin"))
            .await,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_PLUGIN_NOT_FOUND,
    );
}
