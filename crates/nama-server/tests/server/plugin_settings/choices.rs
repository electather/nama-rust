//! Saving a Plugin setting holding one or several picked choices, over the API (ADR 0088).

use std::sync::Arc;

use connectrpc::{ConnectError, ErrorCode, client::HttpClient};
use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, Registrar,
    settings::{
        Choice, Choices, SettingField, SettingKind, Settings, SettingsCheck, SettingsSchema,
    },
};
use nama_proto::nama::v1::{
    GetPluginRequest, PluginServiceClient, RefusalReason, SettingValue, Texts,
    UpdatePluginSettingsRequest, UpdatePluginSettingsResponse, setting_value::Value,
};
use nama_server::plugins::Registry;

use crate::support::{refused::assert_refused, server::TestServer};

/// The Built-in Plugin `rater`, with the Choices setting `ratings` and the Choice setting `rating`
/// both offering G, PG and PG-13, and, if `checks`, a settings check offering only R for each.
struct Rater {
    checks: bool,
}

impl Plugin for Rater {
    fn manifest(&self) -> Manifest {
        let ratings = SettingField {
            key: "ratings".to_owned(),
            label: "Ratings".to_owned(),
            description: String::new(),
            kind: SettingKind::Choices,
            required: false,
            default: None,
            min: None,
            max: None,
            choices: ["G", "PG", "PG-13"].map(choice).into(),
        };
        let rating = SettingField {
            key: "rating".to_owned(),
            label: "Rating".to_owned(),
            kind: SettingKind::Choice,
            ..ratings.clone()
        };
        Manifest {
            version: "0.0.0".to_owned(),
            settings: SettingsSchema {
                fields: vec![ratings, rating],
            },
            ..Manifest::new(PluginId::from_static("rater"))
        }
    }

    fn settings_check(&self) -> Option<&dyn SettingsCheck> {
        self.checks.then_some(self)
    }

    fn register(&self, _registrar: &mut Registrar) -> Result<(), PluginError> {
        Ok(())
    }
}

impl SettingsCheck for Rater {
    fn check<'a>(&'a self, _settings: &'a Settings) -> BoxFuture<'a, Result<Choices, PluginError>> {
        Box::pin(async {
            Ok(["ratings", "rating"]
                .map(|key| (key.to_owned(), vec![choice("R")]))
                .into())
        })
    }
}

fn choice(value: &str) -> Choice {
    Choice {
        value: value.to_owned(),
        label: value.to_owned(),
    }
}

async fn with_rater(checks: bool) -> TestServer {
    let plugins = Registry::new();
    plugins.describe_builtin(move || Arc::new(Rater { checks }));
    TestServer::builder().plugins(plugins).start().await
}

/// Sets `rater`'s `ratings` to `values`, as the Admin.
async fn rate(
    server: &TestServer,
    values: &[&str],
) -> Result<UpdatePluginSettingsResponse, ConnectError> {
    let ratings = Value::Texts(Box::new(Texts {
        values: values.iter().map(|&value| value.to_owned()).collect(),
        ..Default::default()
    }));
    save(server, "ratings", ratings).await
}

/// Sets `rater`'s `key` to `value`, as the Admin.
async fn save(
    server: &TestServer,
    key: &str,
    value: Value,
) -> Result<UpdatePluginSettingsResponse, ConnectError> {
    let value = SettingValue {
        value: Some(value),
        ..Default::default()
    };
    let request = UpdatePluginSettingsRequest {
        id: "rater".to_owned(),
        settings: std::iter::once((key.to_owned(), value)).collect(),
        ..Default::default()
    };
    PluginServiceClient::new(HttpClient::plaintext(), server.client_config())
        .update_plugin_settings(request)
        .await
        .map(connectrpc::UnaryResponse::into_owned)
}

/// The values `rater`'s `ratings` holds, as `GetPlugin` gets them as the Admin; none if it holds
/// nothing.
async fn rated(server: &TestServer) -> Option<Vec<String>> {
    match held(server, "ratings").await? {
        Value::Texts(texts) => Some(texts.values),
        Value::TextValue(_)
        | Value::BoolValue(_)
        | Value::IntegerValue(_)
        | Value::LibraryIds(_) => None,
    }
}

/// What `rater`'s `key` holds, as `GetPlugin` gets it as the Admin; none if it holds nothing.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn held(server: &TestServer, key: &str) -> Option<Value> {
    let request = GetPluginRequest {
        id: "rater".to_owned(),
        ..Default::default()
    };
    let response = PluginServiceClient::new(HttpClient::plaintext(), server.client_config())
        .get_plugin(request)
        .await
        .unwrap()
        .into_owned();
    let plugin = response.plugin.as_option()?;
    let setting = (plugin.settings.iter()).find(|setting| setting.key == key)?;
    setting.value.as_option()?.value.clone()
}

#[tokio::test]
async fn a_choices_setting_keeps_each_pick_once_in_the_order_its_choices_are_listed() {
    let server = with_rater(false).await;

    rate(&server, &["PG-13", "G", "PG-13"]).await.unwrap();

    assert_eq!(
        rated(&server).await,
        Some(vec!["G".to_owned(), "PG-13".to_owned()])
    );
}

#[tokio::test]
async fn a_choices_setting_saves_none_picked() {
    let server = with_rater(false).await;

    rate(&server, &[]).await.unwrap();

    assert_eq!(rated(&server).await, Some(Vec::new()));
}

#[tokio::test]
async fn a_choices_setting_picking_a_value_not_offered_is_refused_and_saves_nothing() {
    let server = with_rater(false).await;

    let refused = assert_refused(
        rate(&server, &["G", "R"]).await,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_CHOICE_NOT_OFFERED,
    );

    assert_eq!(refused.field, "settings.ratings");
    assert_eq!(rated(&server).await, None);
}

#[tokio::test]
async fn a_settings_checks_choices_replace_the_manifests() {
    let server = with_rater(true).await;

    let refused = assert_refused(
        rate(&server, &["G"]).await,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_CHOICE_NOT_OFFERED,
    );
    rate(&server, &["R"]).await.unwrap();

    assert_eq!(refused.field, "settings.ratings");
    assert_eq!(rated(&server).await, Some(vec!["R".to_owned()]));
}

#[tokio::test]
async fn a_choice_setting_listing_its_choices_saves_a_listed_one_without_a_settings_check() {
    let server = with_rater(false).await;

    save(&server, "rating", Value::TextValue("PG".to_owned()))
        .await
        .unwrap();

    assert_eq!(
        held(&server, "rating").await,
        Some(Value::TextValue("PG".to_owned()))
    );
}

#[tokio::test]
async fn a_choice_setting_refuses_a_value_its_choices_dont_list_naming_both_and_saves_nothing() {
    let server = with_rater(false).await;

    let saved = save(&server, "rating", Value::TextValue("R".to_owned())).await;
    let message = saved.as_ref().err().and_then(|err| err.message.clone());

    let refused = assert_refused(
        saved,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_CHOICE_NOT_OFFERED,
    );
    assert_eq!(refused.field, "settings.rating");
    assert_eq!(
        message.as_deref(),
        Some("R isn't one of the choices for the setting rating")
    );
    assert_eq!(held(&server, "rating").await, None);
}

#[tokio::test]
async fn a_settings_checks_choices_replace_a_choice_settings_own() {
    let server = with_rater(true).await;

    let refused = assert_refused(
        save(&server, "rating", Value::TextValue("G".to_owned())).await,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_CHOICE_NOT_OFFERED,
    );
    save(&server, "rating", Value::TextValue("R".to_owned()))
        .await
        .unwrap();

    assert_eq!(refused.field, "settings.rating");
    assert_eq!(
        held(&server, "rating").await,
        Some(Value::TextValue("R".to_owned()))
    );
}
