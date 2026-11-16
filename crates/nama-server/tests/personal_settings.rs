//! Each User reading, setting and forgetting their own Personal settings for an enabled Plugin
//! (ADR 0046): a secret's value never sent back, a required one refused blank, and nobody, an
//! Admin included, seeing another User's values.

#[path = "support/refused.rs"]
mod refused;
#[path = "support/server.rs"]
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
mod server;
#[path = "support/test_plugin.rs"]
mod test_plugin;

use std::collections::HashMap;

use connectrpc::{ConnectError, ErrorCode, client::ClientConfig, client::HttpClient};
use nama_proto::nama::v1::{
    DeletePersonalSettingsRequest, EnablePluginRequest, ListPersonalSettingsRequest,
    ListPluginsRequest, PersonalSettings, PluginServiceClient, RefusalReason, SettingValue,
    UpdatePersonalSettingsRequest, UserServiceClient, setting_value::Value,
};
use nama_server::plugins::Registry;
use refused::assert_refused;
use server::TestServer;

const PLUGIN: &str = "test-plugin";

/// A Server with the test Plugin, which declares a required TEXT `handle` and an optional SECRET
/// `token` as Personal settings, enabled.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn started() -> TestServer {
    let server = TestServer::builder()
        .plugins(Registry::with_builtins())
        .start()
        .await;
    let dir = server.data_dir().join("plugins");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("test.wasm"), test_plugin::build("")).unwrap();
    let plugins = PluginServiceClient::new(HttpClient::plaintext(), server.client_config());
    // Listing is what reads `plugins/`.
    plugins
        .list_plugins(ListPluginsRequest::default())
        .await
        .unwrap();
    plugins
        .enable_plugin(EnablePluginRequest {
            id: PLUGIN.to_owned(),
            ..Default::default()
        })
        .await
        .unwrap();
    server
}

fn users(config: ClientConfig) -> UserServiceClient<HttpClient> {
    UserServiceClient::new(HttpClient::plaintext(), config)
}

fn text(text: &str) -> SettingValue {
    SettingValue {
        value: Some(Value::TextValue(text.to_owned())),
        ..Default::default()
    }
}

/// Sets the caller's Personal settings for the test Plugin to `values`, each one text.
async fn update(
    client: &UserServiceClient<HttpClient>,
    values: &[(&str, &str)],
) -> Result<PersonalSettings, ConnectError> {
    let request = UpdatePersonalSettingsRequest {
        plugin_id: PLUGIN.to_owned(),
        settings: values
            .iter()
            .map(|(key, value)| ((*key).to_owned(), text(value)))
            .collect(),
        ..Default::default()
    };
    let response = client.update_personal_settings(request).await?.into_owned();
    Ok(response.plugin.into_option().unwrap_or_default())
}

/// The caller's Personal settings for the test Plugin as they list them, or `None` if it isn't
/// listed.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn listed(client: &UserServiceClient<HttpClient>) -> Option<PersonalSettings> {
    let listed = client
        .list_personal_settings(ListPersonalSettingsRequest::default())
        .await
        .unwrap()
        .into_owned();
    listed
        .plugins
        .into_iter()
        .find(|plugin| plugin.plugin_id == PLUGIN)
}

/// Each setting's shown text value, or `None` for none, and whether a secret is set, by key.
fn shown(plugin: &PersonalSettings) -> HashMap<String, (Option<String>, bool)> {
    plugin
        .settings
        .iter()
        .map(|setting| {
            let value = match setting.value.as_option().and_then(|it| it.value.as_ref()) {
                Some(Value::TextValue(text)) => Some(text.clone()),
                _ => None,
            };
            (setting.key.clone(), (value, setting.secret_set))
        })
        .collect()
}

fn entry(value: Option<&str>, secret_set: bool) -> (Option<String>, bool) {
    (value.map(str::to_owned), secret_set)
}

#[tokio::test]
async fn a_user_sets_reads_updates_keeping_a_blank_secret_and_deletes_their_personal_settings() {
    let server = started().await;
    let ada = users(server.user("Ada", &[]).await.client_config());

    let fresh = listed(&ada).await.unwrap();
    let set = update(&ada, &[("handle", "ada"), ("token", "tk_secret")])
        .await
        .unwrap();
    let read = listed(&ada).await.unwrap();
    let kept = update(&ada, &[("handle", "ada2"), ("token", ""), ("unknown", "x")])
        .await
        .unwrap();
    ada.delete_personal_settings(DeletePersonalSettingsRequest {
        plugin_id: PLUGIN.to_owned(),
        ..Default::default()
    })
    .await
    .unwrap();
    let deleted = listed(&ada).await.unwrap();

    assert_eq!(fresh.name, "Test Plugin");
    assert!(!fresh.logo_url.is_empty(), "{fresh:?}");
    let keys: Vec<_> = fresh.settings.iter().map(|it| it.key.as_str()).collect();
    assert_eq!(keys, ["handle", "token"]);
    let expected = HashMap::from([
        ("handle".to_owned(), entry(Some("ada"), false)),
        ("token".to_owned(), entry(None, true)),
    ]);
    assert_eq!(shown(&set), expected);
    assert_eq!(shown(&read), expected);
    assert!(!format!("{read:?}").contains("tk_secret"), "{read:?}");
    assert_eq!(
        shown(&kept),
        HashMap::from([
            ("handle".to_owned(), entry(Some("ada2"), false)),
            ("token".to_owned(), entry(None, true)),
        ])
    );
    let empty = HashMap::from([
        ("handle".to_owned(), entry(None, false)),
        ("token".to_owned(), entry(None, false)),
    ]);
    assert_eq!(shown(&fresh), empty);
    assert_eq!(shown(&deleted), empty);
}

#[tokio::test]
async fn a_blank_required_handle_is_refused() {
    let server = started().await;
    let ada = users(server.user("Ada", &[]).await.client_config());

    let refused = update(&ada, &[("handle", ""), ("token", "tk")]).await;

    let refusal = assert_refused(
        refused,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
    );
    assert_eq!(refusal.field, "settings.handle");
}

#[tokio::test]
async fn an_unknown_plugin_is_refused() {
    let server = started().await;
    let ada = users(server.user("Ada", &[]).await.client_config());

    let refused = ada
        .update_personal_settings(UpdatePersonalSettingsRequest {
            plugin_id: "no-such-plugin".to_owned(),
            ..Default::default()
        })
        .await;

    let refusal = assert_refused(
        refused,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_PLUGIN_NOT_FOUND,
    );
    assert_eq!(refusal.field, "plugin_id");
}

#[tokio::test]
async fn each_user_an_admin_included_sees_only_their_own_personal_settings() {
    let server = started().await;
    let ada = users(server.user("Ada", &[]).await.client_config());
    let admin = users(server.client_config());

    update(&ada, &[("handle", "ada")]).await.unwrap();
    let admins_before = listed(&admin).await.unwrap();
    update(&admin, &[("handle", "root")]).await.unwrap();
    let adas = listed(&ada).await.unwrap();
    let admins = listed(&admin).await.unwrap();

    assert_eq!(shown(&admins_before)["handle"], entry(None, false));
    assert_eq!(shown(&adas)["handle"], entry(Some("ada"), false));
    assert_eq!(shown(&admins)["handle"], entry(Some("root"), false));
}

#[tokio::test]
async fn a_failed_check_refuses_the_save_in_the_plugins_words_and_keeps_the_earlier_values() {
    let server = started().await;
    let ada = users(server.user("Ada", &[]).await.client_config());
    update(&ada, &[("handle", "ada")]).await.unwrap();

    let refused = update(&ada, &[("handle", "taken")]).await;

    let message = refused.as_ref().err().and_then(|err| err.message.clone());
    let refusal = assert_refused(
        refused,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_SETTINGS_CHECK_FAILED,
    );
    assert_eq!(
        message.as_deref(),
        Some("the handle taken is someone else's")
    );
    assert_eq!(refusal.field, "");
    assert_eq!(
        refusal.params.get("pluginId").map(String::as_str),
        Some(PLUGIN)
    );
    let kept = listed(&ada).await.unwrap();
    assert_eq!(shown(&kept)["handle"], entry(Some("ada"), false));
}

#[tokio::test]
async fn a_kept_secret_reaches_the_check_and_is_blanked_out_of_its_reason() {
    let server = started().await;
    let ada = users(server.user("Ada", &[]).await.client_config());
    update(&ada, &[("handle", "ada"), ("token", "tk_secret")])
        .await
        .unwrap();

    let refused = update(&ada, &[("handle", "echo")]).await;

    let err = refused.unwrap_err();
    assert_eq!(
        err.message.as_deref(),
        Some("the token … doesn't open echo")
    );
    assert!(!format!("{err:?}").contains("tk_secret"), "{err:?}");
}

#[tokio::test]
async fn a_check_that_loops_or_panics_refuses_the_save_and_leaves_the_plugin_enabled() {
    let server = started().await;
    let ada = users(server.user("Ada", &[]).await.client_config());

    let looped = update(&ada, &[("handle", "loop")]).await;
    // As many as would disable a Plugin whose enabled instance crashed.
    let mut panicked = Vec::new();
    for _ in 0..3 {
        panicked.push(update(&ada, &[("handle", "panic")]).await);
    }
    let saved = update(&ada, &[("handle", "ada")]).await;

    let looped_message = looped.as_ref().err().and_then(|err| err.message.clone());
    let refusal = assert_refused(
        looped,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_SETTINGS_CHECK_FAILED,
    );
    assert_eq!(
        looped_message.as_deref(),
        Some("test-plugin didn't finish checking its settings within 5 seconds")
    );
    assert_eq!(
        refusal.params.get("timeoutSeconds").map(String::as_str),
        Some("5")
    );
    for refused in panicked {
        let message = refused.as_ref().err().and_then(|err| err.message.clone());
        assert_refused(
            refused,
            ErrorCode::FailedPrecondition,
            RefusalReason::REFUSAL_REASON_SETTINGS_CHECK_FAILED,
        );
        assert_eq!(
            message.as_deref(),
            Some("test-plugin failed while checking its settings")
        );
    }
    assert_eq!(shown(&saved.unwrap())["handle"], entry(Some("ada"), false));
    assert!(
        listed(&ada).await.is_some(),
        "the Plugin should stay enabled"
    );
}
