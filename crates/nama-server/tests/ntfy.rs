//! The ntfy Installed Plugin, bundled in the Server, against a fake ntfy: each User told about a
//! Request event gets it on the topic they set in their Personal settings, with their own access
//! token if they set one.

#[path = "support/fake_ntfy.rs"]
mod fake_ntfy;
#[path = "support/refused.rs"]
mod refused;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use connectrpc::{ConnectError, ErrorCode, client::HttpClient};
use fake_ntfy::{FakeNtfy, Published};
use nama_proto::nama::v1::{
    EnablePluginRequest, PluginServiceClient, RefusalReason, ServerServiceClient, ServerSettings,
    SettingValue, UpdatePersonalSettingsRequest, UpdatePluginSettingsRequest,
    UpdateServerSettingsRequest, UserServiceClient, setting_value::Value,
};
use refused::assert_refused;
use server::{SignedIn, TestServer, requests::Requester, tmdb::DUNE};

fn text(key: &str, value: &str) -> (String, SettingValue) {
    let value = SettingValue {
        value: Some(Value::TextValue(value.to_owned())),
        ..Default::default()
    };
    (key.to_owned(), value)
}

/// `user` sets their ntfy Personal settings to `values`.
async fn set_personal(user: &SignedIn, values: &[(&str, &str)]) -> Result<(), ConnectError> {
    UserServiceClient::new(HttpClient::plaintext(), user.client_config())
        .update_personal_settings(UpdatePersonalSettingsRequest {
            plugin_id: "ntfy".to_owned(),
            settings: values.iter().map(|(key, value)| text(key, value)).collect(),
            ..Default::default()
        })
        .await?;
    Ok(())
}

#[tokio::test]
async fn a_topic_ntfy_doesnt_take_is_refused_on_saving_in_ntfys_words() {
    let server = TestServer::builder().start().await;
    PluginServiceClient::new(HttpClient::plaintext(), server.client_config())
        .enable_plugin(EnablePluginRequest {
            id: "ntfy".to_owned(),
            ..Default::default()
        })
        .await
        .unwrap();
    let ada = server.user_with_default_permissions("Ada").await;

    let refused = set_personal(&ada, &[("topic", "bad topic/../!")]).await;

    let message = refused.as_ref().err().and_then(|err| err.message.clone());
    assert_refused(
        refused,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_SETTINGS_CHECK_FAILED,
    );
    assert_eq!(
        message.as_deref(),
        Some("the Personal setting topic must be 1 to 64 letters, digits, _ or -")
    );
}

#[tokio::test]
async fn a_request_event_reaches_each_users_own_topic_with_their_own_token() {
    let server = TestServer::builder().tmdb().start().await;
    let ntfy = FakeNtfy::start().await;
    let plugins = PluginServiceClient::new(HttpClient::plaintext(), server.client_config());
    plugins
        .update_plugin_settings(UpdatePluginSettingsRequest {
            id: "ntfy".to_owned(),
            settings: std::iter::once(text("server_url", ntfy.url())).collect(),
            ..Default::default()
        })
        .await
        .unwrap();
    plugins
        .enable_plugin(EnablePluginRequest {
            id: "ntfy".to_owned(),
            ..Default::default()
        })
        .await
        .unwrap();
    ServerServiceClient::new(HttpClient::plaintext(), server.client_config())
        .update_server_settings(UpdateServerSettingsRequest {
            settings: ServerSettings {
                url: "https://nama.example.com".to_owned(),
                ..Default::default()
            }
            .into(),
            ..Default::default()
        })
        .await
        .unwrap();
    let ada = server.user_with_default_permissions("Ada").await;
    let ben = server.user_with_default_permissions("Ben").await;
    set_personal(&ada, &[("topic", "ada-topic"), ("access_token", "tk_ada")])
        .await
        .unwrap();
    set_personal(&ben, &[("topic", "ben-topic")]).await.unwrap();
    let dune = server.requests(&ada).ask(DUNE).await;
    server.requests(&ben).ask(DUNE).await;

    server.approve(dune.id).await;

    let mut published = ntfy.wait_for_messages(2).await;
    published.sort_by(|one, other| one.topic.cmp(&other.topic));
    let to = |topic: &str, authorization: Option<&str>| Published {
        topic: topic.to_owned(),
        title: Some("Nama".to_owned()),
        click: Some(format!("https://nama.example.com/requests/{}", dune.id)),
        authorization: authorization.map(str::to_owned),
        body: "Dune (2021) is now Approved.".to_owned(),
    };
    assert_eq!(
        published,
        [
            to("ada-topic", Some("Bearer tk_ada")),
            to("ben-topic", None)
        ]
    );
}
