//! Listing the Server's Plugins, getting one with its settings without ever sending a secret's
//! value, and changing a Plugin's settings and enabling and disabling it without a restart, over a
//! real socket and against a real SQLite database.

#[path = "support/refused.rs"]
mod refused;
#[path = "support/server.rs"]
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
mod server;

use std::{collections::BTreeMap, sync::Arc};

use buffa::Message;
use connectrpc::{ConnectError, ErrorCode, client::HttpClient};
use figment::{Figment, providers::Serialized};
use nama_plugin_api::{
    PluginId, auth::Authenticator, metadata::MetadataProvider, settings::SettingValue,
};
use nama_plugin_test_choices as test_choices;
use nama_proto::nama::v1::{
    DisablePluginRequest, EnablePluginRequest, ExtensionPoint, GetPluginRequest,
    ListPluginsRequest, ListPluginsResponse, Plugin, PluginKind, PluginServiceClient,
    PluginSummary, RefusalReason, Setting, SettingKind, TestPluginSettingsRequest,
    TestPluginSettingsResponse, UpdatePluginSettingsRequest, UpdatePluginSettingsResponse,
    setting_value::Value,
};
use nama_server::{
    config::{Config, PluginConfig},
    plugin_settings::{self, PluginSettings},
    plugins::{Registry, SETTINGS_CHECK_TIMEOUT},
};
use refused::assert_refused;
use server::TestServer;

const TMDB_KEY: &str = "tmdb-key-that-must-stay-on-the-server";

/// A Server whose Built-in Plugins have `settings`, stored as an upgrade imports them.
struct Started {
    server: TestServer,
    plugins: Registry,
}

impl Started {
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    async fn with(settings: &BTreeMap<String, PluginConfig>) -> Self {
        let plugins = Registry::with_builtins();
        let config = Figment::from(Serialized::defaults(Config::default()))
            .merge(Serialized::default("plugins", settings))
            .extract()
            .unwrap();
        let server = TestServer::builder()
            .config(config)
            .plugins(plugins.clone())
            .start()
            .await;
        Self { server, plugins }
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

    /// Every Plugin, as the Plugins list gets them as the Admin.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    async fn list(&self) -> ListPluginsResponse {
        self.client()
            .list_plugins(ListPluginsRequest::default())
            .await
            .unwrap()
            .into_owned()
    }

    /// The Plugin `id` with its settings, as its details page gets it as the Admin.
    #[expect(
        clippy::expect_used,
        reason = "a test helper fails its test by panicking"
    )]
    async fn get(&self, id: &str) -> Result<Plugin, ConnectError> {
        let request = GetPluginRequest {
            id: id.to_owned(),
            ..Default::default()
        };
        let got = self.client().get_plugin(request).await?.into_owned();
        Ok(got
            .plugin
            .into_option()
            .expect("GetPlugin returns the Plugin"))
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
        plugin_settings::stored(self.server.db())
            .await
            .unwrap()
            .unwrap()
    }

    /// The Plugins a restart enables from what is stored, as `main` starts them.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    async fn restart(&self) -> Vec<PluginId> {
        let config = Config {
            data_dir: self.server.data_dir().to_owned(),
            ..Config::default()
        };
        PluginSettings::start(
            Arc::clone(self.server.db()),
            Registry::with_builtins(),
            &config,
        )
        .await
        .unwrap()
        .registry()
        .enabled()
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
async fn list_plugins_with(settings: BTreeMap<String, PluginConfig>) -> ListPluginsResponse {
    Started::with(&settings).await.list().await
}

/// Starts a Server whose Built-in Plugins have `settings`, stored as an upgrade imports them, and
/// gets the Plugin `id` with its settings as the Admin.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn get_plugin_with(settings: BTreeMap<String, PluginConfig>, id: &str) -> Plugin {
    Started::with(&settings).await.get(id).await.unwrap()
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
        enabled: true,
    };
    BTreeMap::from([("tmdb".to_owned(), tmdb)])
}

#[expect(
    clippy::expect_used,
    reason = "a test helper fails its test by panicking"
)]
fn plugin<'a>(listed: &'a ListPluginsResponse, id: &str) -> &'a PluginSummary {
    listed
        .summaries
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
        .summaries
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
            ("radarr", installed),
            ("sonarr", installed),
            ("telegram", installed),
            (test_choices::ID, built_in),
            ("tmdb", built_in)
        ]
    );
    for plugin in &listed.summaries {
        assert!(!plugin.version.is_empty(), "{plugin:?}");
    }
    assert!(plugin(&listed, "local-auth").enabled);
    // TMDB can't run without an API key, and an Installed Plugin starts disabled.
    assert!(!plugin(&listed, "tmdb").enabled);
    assert!(!plugin(&listed, "radarr").enabled);
    assert!(!plugin(&listed, "sonarr").enabled);
    assert!(!plugin(&listed, "telegram").enabled);
}

// `plugins/AGENTS.md`: every Plugin ships a logo.
#[tokio::test]
async fn every_builtin_and_bundled_plugin_has_a_logo_the_server_keeps() {
    let listed = list_plugins_with(BTreeMap::new()).await;

    for plugin in &listed.summaries {
        assert!(plugin.logo_svg.starts_with("<svg"), "{plugin:?}");
    }
}

#[tokio::test]
async fn every_plugin_is_listed_with_its_extension_points_enabled_or_not() {
    use ExtensionPoint::*;

    let listed = list_plugins_with(BTreeMap::new()).await;

    let points = |id| &plugin(&listed, id).extension_points[..];
    assert_eq!(points("local-auth"), [EXTENSION_POINT_AUTHENTICATOR]);
    assert_eq!(
        points("log-events"),
        [
            EXTENSION_POINT_SERVER_STARTED_OBSERVER,
            EXTENSION_POINT_EVENTS_OBSERVER
        ]
    );
    assert_eq!(points("radarr"), [EXTENSION_POINT_MOVIE_FULFILLER]);
    assert_eq!(points("sonarr"), [EXTENSION_POINT_SHOW_FULFILLER]);
    assert_eq!(
        points("telegram"),
        [EXTENSION_POINT_NOTIFIER, EXTENSION_POINT_CONNECTOR]
    );
    assert_eq!(
        points(test_choices::ID),
        [
            EXTENSION_POINT_MOVIE_FULFILLER,
            EXTENSION_POINT_SHOW_FULFILLER
        ]
    );
    assert_eq!(points("tmdb"), [EXTENSION_POINT_METADATA_PROVIDER]);
}

#[tokio::test]
async fn a_plugins_extension_points_come_with_it() {
    let tmdb = get_plugin_with(BTreeMap::new(), "tmdb").await;

    assert_eq!(
        tmdb.extension_points,
        [ExtensionPoint::EXTENSION_POINT_METADATA_PROVIDER]
    );
}

#[tokio::test]
async fn every_builtin_and_bundled_plugin_says_its_name_and_what_it_does() {
    let started = Started::with(&BTreeMap::new()).await;
    let listed = started
        .client()
        .list_plugins(ListPluginsRequest::default())
        .await
        .unwrap()
        .into_owned();

    for summary in &listed.summaries {
        assert!(!summary.name.is_empty(), "{summary:?}");
        assert!(!summary.description.is_empty(), "{summary:?}");
        let got = started.get(&summary.id).await.unwrap();
        assert_eq!(
            (got.name, got.description),
            (summary.name.clone(), summary.description.clone())
        );
    }
    assert_eq!(plugin(&listed, "tmdb").name, "TMDB");
    assert_eq!(plugin(&listed, "radarr").name, "Radarr");
}

#[tokio::test]
async fn plugins_are_listed_without_their_settings() {
    let listed = list_plugins_with(tmdb_with_key()).await;

    // Encoded again, the response holds everything the Server sent, fields this client doesn't
    // know included.
    let sent = listed.encode_to_vec();
    assert!(
        !sent
            .windows(b"api_key".len())
            .any(|bytes| bytes == b"api_key"),
        "{listed:?}"
    );
}

#[tokio::test]
async fn a_plugin_is_listed_as_missing_settings_until_its_required_secret_is_stored() {
    let started = Started::with(&BTreeMap::new()).await;
    assert!(plugin(&started.list().await, "tmdb").settings_missing);
    assert!(!plugin(&started.list().await, "local-auth").settings_missing);

    started
        .update("tmdb", [("api_key", text(TMDB_KEY))])
        .await
        .unwrap();

    assert!(!plugin(&started.list().await, "tmdb").settings_missing);
}

#[tokio::test]
async fn getting_an_unknown_plugin_is_refused() {
    let started = Started::with(&BTreeMap::new()).await;

    let refusal = assert_refused(
        started.get("no-such-plugin").await,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_PLUGIN_NOT_FOUND,
    );
    assert_eq!(refusal.field, "id");
    assert_eq!(refusal.params["pluginId"], "no-such-plugin");
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

    let tmdb = get_plugin_with(settings, "tmdb").await;

    assert!(tmdb.enabled);
    let api_key = setting(&tmdb, "api_key");
    assert_eq!(api_key.kind, SettingKind::SETTING_KIND_SECRET);
    assert!(api_key.required);
    assert_eq!(api_key.label, "API key");
    assert!(api_key.secret_set);
    let language = setting(&tmdb, "language");
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
    let tmdb = get_plugin_with(tmdb_with_key(), "tmdb").await;

    // Encoded again, the Plugin holds everything the Server sent, fields this client doesn't know
    // included.
    let sent = tmdb.encode_to_vec();
    assert!(
        !sent
            .windows(TMDB_KEY.len())
            .any(|bytes| bytes == TMDB_KEY.as_bytes()),
        "{tmdb:?}"
    );
    let api_key = setting(&tmdb, "api_key");
    assert!(api_key.secret_set);
    assert!(api_key.value.is_unset(), "{api_key:?}");
}

#[tokio::test]
async fn a_secret_not_set_is_said_to_be_not_set() {
    let tmdb = get_plugin_with(BTreeMap::new(), "tmdb").await;

    let api_key = setting(&tmdb, "api_key");
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
    assert!(
        started
            .restart()
            .await
            .contains(&PluginId::from_static("tmdb"))
    );
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
            enabled: true,
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
    assert_eq!(
        started.restart().await,
        [
            PluginId::from_static("local-auth"),
            PluginId::from_static("log-events")
        ]
    );
    // Disabling kept the settings it was enabled with.
    assert_eq!(
        started.stored().await["tmdb"].settings,
        tmdb_with_key()["tmdb"].settings
    );
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
        plugin_settings::stored(started.server.db()).await.unwrap(),
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
        plugin_settings::stored(started.server.db()).await.unwrap(),
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
        enabled: true,
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

    let test_choices = started.get(test_choices::ID).await.unwrap();
    let tmdb = started.get("tmdb").await.unwrap();
    let tested = started.test("tmdb", []).await;

    assert!(test_choices.checks_settings);
    assert!(!tmdb.checks_settings);
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

    // Starting the Server is outside the deadline, since it is slow under load whatever the check does.
    let started = Started::with(&settings).await;
    let client = started.client();
    // Shorter than the Kernel's timeout for a check, so running the check would fail the test.
    tokio::time::timeout(SETTINGS_CHECK_TIMEOUT / 2, async {
        client
            .disable_plugin(disable(test_choices::ID))
            .await
            .unwrap();
        client
            .enable_plugin(enable(test_choices::ID))
            .await
            .unwrap();
    })
    .await
    .expect("enabling ran the settings check");

    assert!(
        started
            .plugins
            .enabled()
            .contains(&PluginId::from_static(test_choices::ID))
    );
    assert_eq!(started.stored().await, settings);
}
