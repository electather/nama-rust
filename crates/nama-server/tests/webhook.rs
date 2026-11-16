//! The Webhook Installed Plugin, bundled in the Server, against a fake webhook receiver: every
//! Request event posted to the Admin's URL as JSON, signed when they set a key, and a refusal
//! logged.

#[path = "support/fake_webhook.rs"]
mod fake_webhook;
#[path = "support/log.rs"]
mod log;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use std::fmt::Write as _;

use axum::http::StatusCode;
use connectrpc::client::HttpClient;
use fake_webhook::{FakeWebhook, Posted};
use hmac::{Hmac, KeyInit as _, Mac as _};
use log::Log;
use nama_proto::nama::v1::{
    EnablePluginRequest, LibraryKind, PluginServiceClient, ServerServiceClient, ServerSettings,
    SettingValue, UpdatePluginSettingsRequest, UpdateServerSettingsRequest, setting_value::Value,
};
use serde_json::Value as Json;
use server::{
    TestServer,
    requests::Requester,
    tmdb::{ALIEN, DUNE},
};
use sha2::Sha256;

const SECRET: &str = "s3cret-signing-key";

fn text(key: &str, value: &str) -> (String, SettingValue) {
    let value = SettingValue {
        value: Some(Value::TextValue(value.to_owned())),
        ..Default::default()
    };
    (key.to_owned(), value)
}

/// A TMDB-backed Server at `https://nama.example.com` with Webhook enabled against `webhook`,
/// signing with `secret` if given.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn started(webhook: &FakeWebhook, secret: Option<&str>) -> TestServer {
    let server = TestServer::builder().tmdb().start().await;
    let plugins = PluginServiceClient::new(HttpClient::plaintext(), server.client_config());
    let mut settings = vec![text("url", webhook.url())];
    settings.extend(secret.map(|secret| text("secret", secret)));
    plugins
        .update_plugin_settings(UpdatePluginSettingsRequest {
            id: "webhook".to_owned(),
            settings: settings.into_iter().collect(),
            ..Default::default()
        })
        .await
        .unwrap();
    plugins
        .enable_plugin(EnablePluginRequest {
            id: "webhook".to_owned(),
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
    server
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn json(posted: &Posted) -> Json {
    serde_json::from_slice(&posted.body).unwrap()
}

#[tokio::test]
async fn a_request_becoming_available_posts_one_json_body_with_its_event_request_link_and_text() {
    let webhook = FakeWebhook::start().await;
    let server = started(&webhook, None).await;
    let ada = server.user_with_default_permissions("Ada").await;
    let alien = server.requests(&ada).ask(ALIEN).await;
    server.approve(alien.id).await;

    let _media = server
        .library(
            LibraryKind::LIBRARY_KIND_MOVIES,
            &["Alien (1979)/Alien (1979).mkv"],
        )
        .await;

    let posted = webhook.wait_for_posts(3).await;
    let available: Vec<_> = posted
        .iter()
        .filter(|posted| json(posted)["text"] == "Alien (1979) is now Available.")
        .collect();
    let [available] = available.as_slice() else {
        panic!("not one post said Alien is Available: {posted:?}");
    };
    let body = json(available);
    assert!(body["event"].is_object(), "{body}");
    assert_eq!(
        body["request"]["state"], "REQUEST_STATE_AVAILABLE",
        "{body}"
    );
    assert_eq!(
        body["request_url"],
        format!("https://nama.example.com/requests/{}", alien.id)
    );
    assert_eq!(available.signature, None);
}

#[tokio::test]
async fn with_a_key_set_each_body_carries_its_hmac_sha256_signature() {
    let webhook = FakeWebhook::start().await;
    let server = started(&webhook, Some(SECRET)).await;
    let ada = server.user_with_default_permissions("Ada").await;

    server.requests(&ada).ask(DUNE).await;

    let posted = webhook.wait_for_posts(1).await;
    let mut mac = Hmac::<Sha256>::new_from_slice(SECRET.as_bytes()).unwrap();
    mac.update(&posted[0].body);
    let hmac = mac.finalize().into_bytes();
    let hex = hmac.iter().fold(String::new(), |mut hex, byte| {
        let _ = write!(hex, "{byte:02x}");
        hex
    });
    assert_eq!(posted[0].signature, Some(format!("sha256={hex}")));
}

#[tokio::test]
async fn a_refusal_is_logged_as_a_failed_delivery_with_its_status() {
    let log = Log::global();
    let webhook = FakeWebhook::start().await;
    webhook.answer(StatusCode::INTERNAL_SERVER_ERROR);
    let server = started(&webhook, None).await;
    let ada = server.user_with_default_permissions("Ada").await;

    server.requests(&ada).ask(DUNE).await;

    log.wait_for(
        &[
            "webhook",
            "provider failed",
            "the webhook failed with HTTP status 500",
        ],
        1,
    )
    .await;
}
