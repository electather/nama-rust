//! Listing the Server's Plugins and their settings, without ever sending a secret's value, and
//! changing those settings and enabling and disabling them without a restart, over a real socket
//! and against a real SQLite database.

#[path = "support/refused.rs"]
mod refused;
#[path = "support/server.rs"]
#[expect(dead_code, reason = "the harness's plain `start` has no use here")]
mod server;

use std::{collections::BTreeMap, sync::Arc};

use buffa::Message;
use connectrpc::{ConnectError, ErrorCode, client::HttpClient};
use nama_plugin_api::{
    PluginId, auth::Authenticator, metadata::MetadataProvider, settings::SettingValue,
};
use nama_plugin_test_choices as test_choices;
use nama_proto::nama::v1::{
    DisablePluginRequest, EnablePluginRequest, ListPluginsRequest, ListPluginsResponse, Plugin,
    PluginKind, PluginServiceClient, RefusalReason, Setting, SettingKind,
    TestPluginSettingsRequest, TestPluginSettingsResponse, UpdatePluginSettingsRequest,
    UpdatePluginSettingsResponse, setting_value::Value,
};
use nama_server::{
    config::{Config, PluginConfig},
    db::Database,
    plugin_settings,
    plugins::{Registry, SETTINGS_CHECK_TIMEOUT},
};
use refused::assert_refused;
use server::TestServer;

const TMDB_KEY: &str = "tmdb-key-that-must-stay-on-the-server";

/// A Server whose Built-in Plugins have `settings`, stored as an upgrade imports them.
struct Started {
    server: TestServer,
    db: Arc<Database>,
    plugins: Registry,
}

impl Started {
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    async fn with(settings: &BTreeMap<String, PluginConfig>) -> Self {
        let plugins = Registry::with_builtins(settings);
        let (server, db) = TestServer::start_with_db(Config::default(), plugins.clone()).await;
        plugin_settings::import(&db, settings).await.unwrap();
        Self {
            server,
            db,
            plugins,
        }
    }

    fn client(&self) -> PluginServiceClient<HttpClient> {
        PluginServiceClient::new(HttpClient::plaintext(), self.server.client_config())
    }

    /// Sets the Plugin `id`'s settings to `settings`, as the Admin.
    async fn update(
        &self,
        id: &str,
        settings: impl IntoIterator<Item = (&str, Value)>,
    ) -> Result<UpdatePluginSettingsResponse, ConnectError> {
        let request = UpdatePluginSettingsRequest {
            id: id.to_owned(),
            settings: setting_values(settings),
            ..Default::default()
        };
        self.client()
            .update_plugin_settings(request)
            .await
            .map(connectrpc::UnaryResponse::into_owned)
    }

    /// Runs the Plugin `id`'s settings check on `settings`, as the Admin.
    async fn test(
        &self,
        id: &str,
        settings: impl IntoIterator<Item = (&str, Value)>,
    ) -> Result<TestPluginSettingsResponse, ConnectError> {
        let request = TestPluginSettingsRequest {
            id: id.to_owned(),
            settings: setting_values(settings),
            ..Default::default()
        };
        self.client()
            .test_plugin_settings(request)
            .await
            .map(connectrpc::UnaryResponse::into_owned)
    }

    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    async fn stored(&self) -> BTreeMap<String, PluginConfig> {
        plugin_settings::stored(&self.db).await.unwrap().unwrap()
    }

    /// Whether some enabled Plugin provides `T`, as Matching looks for a metadata Provider to call.
    async fn provides<T: ?Sized + Send + Sync + 'static>(&self) -> bool {
        self.plugins
            .call_provider(|_: Arc<T>| async {})
            .await
            .is_some()
    }

    /// Whether Matching has a metadata Provider to call.
    async fn can_match(&self) -> bool {
        self.provides::<dyn MetadataProvider>().await
    }
}

/// Starts a Server whose Built-in Plugins have `settings`, stored as an upgrade imports them, and
/// lists its Plugins as the Admin.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn list_plugins_with(settings: BTreeMap<String, PluginConfig>) -> ListPluginsResponse {
    Started::with(&settings)
        .await
        .client()
        .list_plugins(ListPluginsRequest::default())
        .await
        .unwrap()
        .into_owned()
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

/// `settings` as a request's map of setting values, which is whatever map type buffa generates.
fn setting_values<'a, M: FromIterator<(String, nama_proto::nama::v1::SettingValue)>>(
    settings: impl IntoIterator<Item = (&'a str, Value)>,
) -> M {
    settings
        .into_iter()
        .map(|(key, value)| {
            let value = nama_proto::nama::v1::SettingValue {
                value: Some(value),
                ..Default::default()
            };
            (key.to_owned(), value)
        })
        .collect()
}

fn text(value: &str) -> Value {
    Value::TextValue(value.to_owned())
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
async fn every_builtin_and_bundled_plugin_is_listed_with_whether_it_is_enabled() {
    let listed = list_plugins_with(BTreeMap::new()).await;

    let kinds: Vec<_> = listed
        .plugins
        .iter()
        .map(|plugin| (plugin.id.as_str(), plugin.kind.as_known()))
        .collect();
    let built_in = Some(PluginKind::PLUGIN_KIND_BUILT_IN);
    let installed = Some(PluginKind::PLUGIN_KIND_INSTALLED);
    assert_eq!(
        kinds,
        [
            ("local-auth", built_in),
            ("log-events", built_in),
            ("telegram", installed),
            (test_choices::ID, built_in),
            ("tmdb", built_in)
        ]
    );
    for plugin in &listed.plugins {
        assert!(!plugin.version.is_empty(), "{plugin:?}");
    }
    assert!(plugin(&listed, "local-auth").enabled);
    // TMDB can't run without an API key, and an Installed Plugin starts disabled.
    assert!(!plugin(&listed, "tmdb").enabled);
    assert!(!plugin(&listed, "telegram").enabled);
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

#[tokio::test]
async fn setting_the_tmdb_api_key_on_a_fresh_server_enables_it_without_a_restart() {
    let started = Started::with(&BTreeMap::new()).await;
    assert!(!started.can_match().await);

    let updated = started.update("tmdb", [("api_key", text(TMDB_KEY))]).await;

    let plugin = updated.unwrap().plugin.into_option().unwrap();
    assert!(plugin.enabled, "{plugin:?}");
    assert!(setting(&plugin, "api_key").secret_set);
    assert!(started.can_match().await);
}

#[tokio::test]
async fn new_settings_replace_the_running_ones_and_survive_a_restart() {
    let started = Started::with(&tmdb_with_key()).await;

    started
        .update(
            "tmdb",
            [("api_key", text("new-key")), ("language", text("fr-FR"))],
        )
        .await
        .unwrap();

    let tmdb = started.stored().await.remove("tmdb").unwrap();
    assert_eq!(
        tmdb.settings,
        BTreeMap::from([
            (
                "api_key".to_owned(),
                SettingValue::Text("new-key".to_owned())
            ),
            (
                "language".to_owned(),
                SettingValue::Text("fr-FR".to_owned())
            ),
        ])
    );
    assert!(started.can_match().await);
    // A restart enables the Built-in Plugins with what is stored.
    let restarted = Registry::with_builtins(&started.stored().await);
    assert!(restarted.enabled().contains(&PluginId::from_static("tmdb")));
}

#[tokio::test]
async fn a_secret_left_blank_or_out_keeps_its_stored_value() {
    let started = Started::with(&tmdb_with_key()).await;

    started
        .update("tmdb", [("api_key", text("")), ("language", text("fr-FR"))])
        .await
        .unwrap();
    started
        .update("tmdb", [("language", text("de-DE"))])
        .await
        .unwrap();

    let tmdb = started.stored().await.remove("tmdb").unwrap();
    assert_eq!(
        tmdb.settings.get("api_key"),
        Some(&SettingValue::Text(TMDB_KEY.to_owned()))
    );
    assert!(started.can_match().await);
}

#[tokio::test]
async fn a_missing_required_setting_is_refused_naming_it_and_nothing_changes() {
    let mut settings = BTreeMap::new();
    settings.insert(
        "tmdb".to_owned(),
        PluginConfig {
            settings: BTreeMap::from([(
                "language".to_owned(),
                SettingValue::Text("de-DE".to_owned()),
            )]),
            ..PluginConfig::default()
        },
    );
    let started = Started::with(&settings).await;

    let updated = started.update("tmdb", [("language", text("fr-FR"))]).await;

    let refusal = assert_refused(
        updated,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
    );
    assert_eq!(refusal.field, "settings.api_key");
    assert_eq!(started.stored().await, settings);
}

#[tokio::test]
async fn a_value_of_the_wrong_kind_is_refused_naming_it_and_nothing_changes() {
    let started = Started::with(&tmdb_with_key()).await;

    let updated = started
        .update(
            "tmdb",
            [
                ("api_key", text("new-key")),
                ("language", Value::BoolValue(true)),
            ],
        )
        .await;

    let refusal = assert_refused(
        updated,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_SETTING_WRONG_KIND,
    );
    assert_eq!(refusal.field, "settings.language");
    assert_eq!(
        refusal.params.get("settingKind").map(String::as_str),
        Some("SETTING_KIND_TEXT")
    );
    assert_eq!(started.stored().await, tmdb_with_key());
}

#[tokio::test]
async fn a_disabled_plugins_new_settings_are_saved_but_it_stays_disabled() {
    let mut settings = BTreeMap::new();
    settings.insert(
        "tmdb".to_owned(),
        PluginConfig {
            enabled: false,
            settings: BTreeMap::new(),
        },
    );
    let started = Started::with(&settings).await;

    let updated = started.update("tmdb", [("api_key", text(TMDB_KEY))]).await;

    assert!(!updated.unwrap().plugin.into_option().unwrap().enabled);
    assert!(!started.can_match().await);
    let tmdb = started.stored().await.remove("tmdb").unwrap();
    assert!(!tmdb.enabled);
    assert!(tmdb.settings.contains_key("api_key"));
}

#[tokio::test]
async fn an_unknown_plugin_is_refused() {
    let started = Started::with(&BTreeMap::new()).await;

    let updated = started.update("nope", []).await;

    let refusal = assert_refused(
        updated,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_PLUGIN_NOT_FOUND,
    );
    assert_eq!(refusal.field, "id");
}

#[tokio::test]
async fn disabling_tmdb_stops_matching_from_calling_it_until_it_is_enabled_again() {
    let started = Started::with(&tmdb_with_key()).await;
    assert!(started.can_match().await);

    started
        .client()
        .disable_plugin(disable("tmdb"))
        .await
        .unwrap();
    assert!(!started.can_match().await);

    started
        .client()
        .enable_plugin(enable("tmdb"))
        .await
        .unwrap();
    assert!(started.can_match().await);
}

#[tokio::test]
async fn a_plugins_enabled_state_survives_a_restart() {
    let started = Started::with(&tmdb_with_key()).await;

    started
        .client()
        .disable_plugin(disable("tmdb"))
        .await
        .unwrap();
    started
        .client()
        .disable_plugin(disable("log-events"))
        .await
        .unwrap();
    started
        .client()
        .enable_plugin(enable("log-events"))
        .await
        .unwrap();

    // A restart enables what the database holds, as `main` does.
    let stored = plugin_settings::stored(&started.db).await.unwrap().unwrap();
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
    let started = Started::with(&BTreeMap::new()).await;

    let refusal = assert_refused(
        started.client().disable_plugin(disable("local-auth")).await,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_LAST_AUTHENTICATOR,
    );

    assert_eq!(refusal.field, "id");
    assert_eq!(refusal.params["pluginId"], "local-auth");
    assert!(started.provides::<dyn Authenticator>().await);
    assert_eq!(
        plugin_settings::stored(&started.db).await.unwrap(),
        Some(BTreeMap::new())
    );
}

#[tokio::test]
async fn enabling_a_plugin_missing_a_required_setting_is_refused_naming_it() {
    let started = Started::with(&BTreeMap::new()).await;

    let refusal = assert_refused(
        started.client().enable_plugin(enable("tmdb")).await,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
    );

    assert_eq!(refusal.field, "settings.api_key");
    assert_eq!(refusal.params["settingKey"], "api_key");
    assert!(!started.can_match().await);
    assert_eq!(
        plugin_settings::stored(&started.db).await.unwrap(),
        Some(BTreeMap::new())
    );
}

#[tokio::test]
async fn enabling_a_plugin_with_a_setting_of_the_wrong_kind_is_refused_naming_it() {
    let tmdb = PluginConfig {
        enabled: false,
        settings: BTreeMap::from([("api_key".to_owned(), SettingValue::Integer(7))]),
    };
    let started = Started::with(&BTreeMap::from([("tmdb".to_owned(), tmdb)])).await;

    let refusal = assert_refused(
        started.client().enable_plugin(enable("tmdb")).await,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_SETTING_WRONG_KIND,
    );

    assert_eq!(refusal.field, "settings.api_key");
    assert_eq!(refusal.params["settingKind"], "SETTING_KIND_SECRET");
    assert!(!started.can_match().await);
}

#[tokio::test]
async fn enabling_or_disabling_an_unknown_plugin_is_refused() {
    let started = Started::with(&BTreeMap::new()).await;

    let refusal = assert_refused(
        started
            .client()
            .enable_plugin(enable("no-such-plugin"))
            .await,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_PLUGIN_NOT_FOUND,
    );
    assert_eq!(refusal.field, "id");
    assert_eq!(refusal.params["pluginId"], "no-such-plugin");
    assert_refused(
        started
            .client()
            .disable_plugin(disable("no-such-plugin"))
            .await,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_PLUGIN_NOT_FOUND,
    );
}

/// The labels of the choices `tested` offers for the setting `key`.
fn choice_labels<'a>(tested: &'a TestPluginSettingsResponse, key: &str) -> Vec<&'a str> {
    tested.choices.get(key).map_or_else(Vec::new, |choices| {
        choices
            .choices
            .iter()
            .map(|choice| choice.label.as_str())
            .collect()
    })
}

#[tokio::test]
async fn testing_settings_returns_the_choices_the_check_offers() {
    let started = Started::with(&BTreeMap::new()).await;

    let tested = started
        .test(test_choices::ID, [("api_key", text(test_choices::API_KEY))])
        .await
        .unwrap();

    assert_eq!(choice_labels(&tested, "profile"), ["HD", "Ultra HD"]);
    assert_eq!(tested.choices["profile"].choices[0].value, "1");
}

/// Settings stored for the test Plugin: `api_key` and nothing else.
fn test_choices_with_key(api_key: &str) -> BTreeMap<String, PluginConfig> {
    let config = PluginConfig {
        settings: BTreeMap::from([("api_key".to_owned(), SettingValue::Text(api_key.to_owned()))]),
        ..PluginConfig::default()
    };
    BTreeMap::from([(test_choices::ID.to_owned(), config)])
}

/// Asserts that `sent`, encoded as the Server sent it, holds no secret's value.
fn assert_no_secret(sent: &[u8]) {
    let secret = test_choices::API_KEY.as_bytes();
    assert!(!sent.windows(secret.len()).any(|bytes| bytes == secret));
}

#[tokio::test]
async fn a_failed_check_is_refused_with_the_plugins_error() {
    let started = Started::with(&BTreeMap::new()).await;

    let tested = started
        .test(test_choices::ID, [("api_key", text("wrong-key"))])
        .await;

    let refusal = assert_refused(
        tested.clone(),
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_SETTINGS_CHECK_FAILED,
    );
    assert_eq!(refusal.params["pluginId"], test_choices::ID);
    assert_eq!(
        tested.unwrap_err().message.as_deref(),
        Some("the service refused the API key")
    );
}

#[tokio::test]
async fn a_check_that_panics_is_a_failed_test_and_the_server_carries_on() {
    let started = Started::with(&BTreeMap::new()).await;

    let tested = started
        .test(
            test_choices::ID,
            [
                ("api_key", text(test_choices::API_KEY)),
                ("fail_with", text(test_choices::PANIC)),
            ],
        )
        .await;

    assert_refused(
        tested,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_SETTINGS_CHECK_FAILED,
    );
    let retested = started
        .test(test_choices::ID, [("api_key", text(test_choices::API_KEY))])
        .await;
    assert_eq!(
        choice_labels(&retested.unwrap(), "profile"),
        ["HD", "Ultra HD"]
    );
}

#[tokio::test]
async fn a_secret_left_blank_is_tested_with_its_stored_value() {
    let started = Started::with(&test_choices_with_key(test_choices::API_KEY)).await;

    let tested = started
        .test(test_choices::ID, [("api_key", text(""))])
        .await;

    assert_eq!(
        choice_labels(&tested.unwrap(), "profile"),
        ["HD", "Ultra HD"]
    );
}

#[tokio::test]
async fn unsaved_settings_are_tested_in_place_of_the_stored_ones_and_not_saved() {
    let started = Started::with(&test_choices_with_key("old-key")).await;

    let tested = started
        .test(test_choices::ID, [("api_key", text(test_choices::API_KEY))])
        .await;

    assert_eq!(
        choice_labels(&tested.unwrap(), "profile"),
        ["HD", "Ultra HD"]
    );
    assert_eq!(started.stored().await, test_choices_with_key("old-key"));
}

#[tokio::test]
async fn testing_without_a_required_setting_is_refused_naming_it() {
    let started = Started::with(&BTreeMap::new()).await;

    let tested = started.test(test_choices::ID, []).await;

    let refusal = assert_refused(
        tested,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
    );
    assert_eq!(refusal.field, "settings.api_key");
}

#[tokio::test]
async fn neither_a_test_nor_its_failure_sends_a_secrets_value() {
    let started = Started::with(&test_choices_with_key(test_choices::API_KEY)).await;

    let passed = started.test(test_choices::ID, []).await.unwrap();
    let quoting_the_key = format!("the service refused {}", test_choices::API_KEY);
    let failed = started
        .test(test_choices::ID, [("fail_with", text(&quoting_the_key))])
        .await;

    assert_no_secret(&passed.encode_to_vec());
    assert_no_secret(format!("{failed:?}").as_bytes());
    let refusal = assert_refused(
        failed,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_SETTINGS_CHECK_FAILED,
    );
    assert_no_secret(&refusal.encode_to_vec());
}

#[tokio::test]
async fn only_a_plugin_with_a_check_says_so_and_testing_one_without_is_refused() {
    let started = Started::with(&tmdb_with_key()).await;

    let listed = started
        .client()
        .list_plugins(ListPluginsRequest::default())
        .await
        .unwrap()
        .into_owned();
    let tested = started.test("tmdb", []).await;

    assert!(plugin(&listed, test_choices::ID).checks_settings);
    assert!(!plugin(&listed, "tmdb").checks_settings);
    assert_refused(
        tested,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_SETTINGS_CHECK_FAILED,
    );
}

/// Settings stored for the test Plugin: its API key, and `profile` chosen as `1` ("HD").
fn test_choices_with_profile() -> BTreeMap<String, PluginConfig> {
    let mut settings = test_choices_with_key(test_choices::API_KEY);
    if let Some(config) = settings.get_mut(test_choices::ID) {
        config
            .settings
            .insert("profile".to_owned(), SettingValue::Text("1".to_owned()));
    }
    settings
}

#[tokio::test]
async fn saving_a_choice_the_check_offers_saves_it() {
    let started = Started::with(&test_choices_with_profile()).await;

    let updated = started
        .update(test_choices::ID, [("profile", text("2"))])
        .await;

    assert!(updated.unwrap().plugin.into_option().unwrap().enabled);
    let config = started.stored().await.remove(test_choices::ID).unwrap();
    assert_eq!(
        config.settings.get("profile"),
        Some(&SettingValue::Text("2".to_owned()))
    );
}

#[tokio::test]
async fn saving_a_choice_the_check_didnt_offer_is_refused_naming_it_and_nothing_changes() {
    let started = Started::with(&test_choices_with_profile()).await;

    let updated = started
        .update(test_choices::ID, [("profile", text("3"))])
        .await;

    let refusal = assert_refused(
        updated,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_CHOICE_NOT_OFFERED,
    );
    assert_eq!(refusal.field, "settings.profile");
    assert_eq!(refusal.params["settingKey"], "profile");
    assert_eq!(refusal.params["pluginId"], test_choices::ID);
    assert_eq!(refusal.params["settingValue"], "3");
    assert_eq!(started.stored().await, test_choices_with_profile());
    assert!(
        started
            .plugins
            .enabled()
            .contains(&PluginId::from_static(test_choices::ID))
    );
}

#[tokio::test]
async fn saving_while_the_check_fails_is_refused_with_the_plugins_error_and_nothing_changes() {
    let started = Started::with(&test_choices_with_profile()).await;

    let updated = started
        .update(
            test_choices::ID,
            [("api_key", text("wrong-key")), ("profile", text("1"))],
        )
        .await;

    let refusal = assert_refused(
        updated.clone(),
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_SETTINGS_CHECK_FAILED,
    );
    assert_eq!(refusal.field, "settings.profile");
    assert_eq!(refusal.params["settingKey"], "profile");
    assert_eq!(
        updated.unwrap_err().message.as_deref(),
        Some("the service refused the API key")
    );
    assert_eq!(started.stored().await, test_choices_with_profile());
    assert!(
        started
            .plugins
            .enabled()
            .contains(&PluginId::from_static(test_choices::ID))
    );
}

#[tokio::test]
async fn a_plugin_whose_check_would_hang_starts_and_is_enabled_with_its_stored_choice() {
    let mut settings = test_choices_with_profile();
    if let Some(config) = settings.get_mut(test_choices::ID) {
        config.settings.insert(
            "fail_with".to_owned(),
            SettingValue::Text(test_choices::HANG.to_owned()),
        );
    }

    // Shorter than the Kernel's timeout for a check, so running the check would fail the test.
    let deadline = SETTINGS_CHECK_TIMEOUT / 2;
    let started = tokio::time::timeout(deadline, async {
        let started = Started::with(&settings).await;
        let client = started.client();
        client
            .disable_plugin(disable(test_choices::ID))
            .await
            .unwrap();
        client
            .enable_plugin(enable(test_choices::ID))
            .await
            .unwrap();
        started
    })
    .await
    .expect("starting and enabling ran the settings check");

    assert!(
        started
            .plugins
            .enabled()
            .contains(&PluginId::from_static(test_choices::ID))
    );
    assert_eq!(started.stored().await, settings);
}
