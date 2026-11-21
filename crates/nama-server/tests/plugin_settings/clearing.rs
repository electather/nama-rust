//! Clearing a Plugin's settings over the API: a disabled Plugin's required settings, checked when
//! it is enabled rather than when they are saved, and a stored secret, forgotten by its key.

use std::sync::{Arc, Mutex, PoisonError};

use connectrpc::{ConnectError, ErrorCode, client::HttpClient};
use nama_plugin_api::{
    Manifest, Plugin, PluginError, PluginId, Registrar,
    settings::{SettingField, SettingKind, SettingsSchema},
};
use nama_proto::nama::v1::{
    DisablePluginRequest, EnablePluginRequest, GetPluginRequest, ListPluginsRequest,
    PluginServiceClient, PluginSummary, RefusalReason, Setting, SettingValue,
    UpdatePluginSettingsRequest, UpdatePluginSettingsResponse, setting_value::Value,
};
use nama_server::plugins::Registry;

use crate::{refused::assert_refused, server::TestServer};

/// The Built-in Plugin `bucketer`, with the required text settings `endpoint` and `bucket`, the
/// required secret `secret_key` and the optional secret `session_token`, keeping the secrets each
/// enable hands it.
struct Bucketer(SecretsGiven);

/// The `secret_key` and `session_token` one server's `bucketer` was enabled with, each time, since
/// a secret's value never comes back over the API.
type SecretsGiven = Arc<Mutex<Vec<(Option<String>, Option<String>)>>>;

impl Plugin for Bucketer {
    fn manifest(&self) -> Manifest {
        let field = |key: &str, kind, required| SettingField {
            key: key.to_owned(),
            label: key.to_owned(),
            description: String::new(),
            kind,
            required,
            default: None,
            min: None,
            max: None,
            choices: Vec::new(),
        };
        Manifest {
            version: "0.0.0".to_owned(),
            settings: SettingsSchema {
                fields: vec![
                    field("endpoint", SettingKind::Text, true),
                    field("bucket", SettingKind::Text, true),
                    field("secret_key", SettingKind::Secret, true),
                    field("session_token", SettingKind::Secret, false),
                ],
            },
            ..Manifest::new(PluginId::from_static("bucketer"))
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        let secret = |key| registrar.settings().text(key).map(str::to_owned);
        let given = (secret("secret_key"), secret("session_token"));
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(given);
        Ok(())
    }
}

fn client(server: &TestServer) -> PluginServiceClient<HttpClient> {
    PluginServiceClient::new(HttpClient::plaintext(), server.client_config())
}

/// A server with `bucketer` saved with all its settings, and so running, as the Admin, and the
/// secrets it is enabled with.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn with_bucketer_set_up() -> (TestServer, SecretsGiven) {
    let given = SecretsGiven::default();
    let plugins = Registry::new();
    let made = Arc::clone(&given);
    plugins.describe_builtin(move || Arc::new(Bucketer(Arc::clone(&made))));
    let server = TestServer::builder().plugins(plugins).start().await;
    let secrets = [
        ("secret_key", text("s3cr3t")),
        ("session_token", text("t0k3n")),
    ];
    update(&server, &[texts(), secrets].concat()).await.unwrap();
    (server, given)
}

/// The secrets `bucketer` was last enabled with.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn last_given(given: &SecretsGiven) -> (Option<String>, Option<String>) {
    let given = given.lock().unwrap_or_else(PoisonError::into_inner);
    given.last().cloned().unwrap()
}

/// [`with_bucketer_set_up`], then disabled.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn with_bucketer_set_up_and_disabled() -> TestServer {
    let (server, _) = with_bucketer_set_up().await;
    let disable = DisablePluginRequest {
        id: "bucketer".to_owned(),
        ..Default::default()
    };
    client(&server).disable_plugin(disable).await.unwrap();
    server
}

/// `bucketer`'s required text settings, sent so a running `bucketer` keeps them.
fn texts() -> [(&'static str, SettingValue); 2] {
    [
        ("endpoint", text("https://s3.example")),
        ("bucket", text("media")),
    ]
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
    update_forgetting(server, &[], settings).await
}

/// Sets `bucketer`'s settings to `settings`, forgetting the secrets `forgotten`, as the Admin.
async fn update_forgetting(
    server: &TestServer,
    forgotten: &[&str],
    settings: &[(&str, SettingValue)],
) -> Result<UpdatePluginSettingsResponse, ConnectError> {
    let request = UpdatePluginSettingsRequest {
        id: "bucketer".to_owned(),
        settings: (settings.iter())
            .map(|(key, value)| ((*key).to_owned(), value.clone()))
            .collect(),
        forgotten_secrets: forgotten.iter().map(|&key| key.to_owned()).collect(),
        ..Default::default()
    };
    client(server)
        .update_plugin_settings(request)
        .await
        .map(connectrpc::UnaryResponse::into_owned)
}

/// `bucketer`'s settings, as `GetPlugin` gets them.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn settings(server: &TestServer) -> Vec<Setting> {
    let request = GetPluginRequest {
        id: "bucketer".to_owned(),
        ..Default::default()
    };
    let response = client(server)
        .get_plugin(request)
        .await
        .unwrap()
        .into_owned();
    response.plugin.into_option().unwrap().settings
}

/// The keys of `bucketer`'s secrets that are set, as `GetPlugin` gets them.
async fn secrets_set(server: &TestServer) -> Vec<String> {
    (settings(server).await.into_iter())
        .filter(|setting| setting.secret_set)
        .map(|setting| setting.key)
        .collect()
}

/// The text values of `bucketer`'s settings that have one, as `GetPlugin` gets them.
async fn stored_texts(server: &TestServer) -> Vec<String> {
    (settings(server).await.into_iter())
        .filter_map(|setting| match setting.value.into_option()?.value? {
            Value::TextValue(text) => Some(text),
            Value::BoolValue(_)
            | Value::IntegerValue(_)
            | Value::LibraryIds(_)
            | Value::Texts(_) => None,
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

#[tokio::test]
async fn a_disabled_plugins_forgotten_secret_is_no_longer_set() {
    let server = with_bucketer_set_up_and_disabled().await;

    update_forgetting(&server, &["secret_key"], &[])
        .await
        .unwrap();

    assert_eq!(secrets_set(&server).await, ["session_token"]);
    assert!(summary(&server).await.settings_missing);
}

#[tokio::test]
async fn forgetting_a_running_plugins_required_secret_is_refused_and_nothing_changes() {
    let (server, _) = with_bucketer_set_up().await;

    let refusal = assert_refused(
        update_forgetting(&server, &["secret_key"], &texts()).await,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
    );

    assert_eq!(refusal.field, "settings.secret_key");
    assert_eq!(secrets_set(&server).await, ["secret_key", "session_token"]);
    assert!(summary(&server).await.enabled);
}

#[tokio::test]
async fn a_running_plugin_forgetting_an_optional_secret_runs_again_without_it() {
    let (server, given) = with_bucketer_set_up().await;

    update_forgetting(&server, &["session_token"], &texts())
        .await
        .unwrap();

    assert_eq!(secrets_set(&server).await, ["secret_key"]);
    assert!(summary(&server).await.enabled);
    assert_eq!(last_given(&given), (Some("s3cr3t".to_owned()), None));
}

#[tokio::test]
async fn a_forgotten_secret_also_sent_takes_the_sent_value() {
    let (server, given) = with_bucketer_set_up().await;

    let sent = [texts().as_slice(), &[("secret_key", text("n3w"))]].concat();
    update_forgetting(&server, &["secret_key"], &sent)
        .await
        .unwrap();

    assert_eq!(secrets_set(&server).await, ["secret_key", "session_token"]);
    let n3w = (Some("n3w".to_owned()), Some("t0k3n".to_owned()));
    assert_eq!(last_given(&given), n3w);
}

#[tokio::test]
async fn forgetting_a_key_that_isnt_a_secret_changes_nothing() {
    let (server, _) = with_bucketer_set_up().await;

    update_forgetting(&server, &["endpoint", "region"], &texts())
        .await
        .unwrap();

    assert_eq!(stored_texts(&server).await, ["https://s3.example", "media"]);
    assert_eq!(secrets_set(&server).await, ["secret_key", "session_token"]);
}

#[tokio::test]
async fn a_secret_left_out_or_sent_blank_keeps_its_stored_value() {
    let (server, _) = with_bucketer_set_up().await;

    let sent = [texts().as_slice(), &[("secret_key", text(""))]].concat();
    update(&server, &sent).await.unwrap();

    assert_eq!(secrets_set(&server).await, ["secret_key", "session_token"]);
}
