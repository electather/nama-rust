//! The Discord Installed Plugin, bundled in the Server, against a fake Discord: each User told
//! about a Request event gets it on the webhook they set in their Personal settings, and a User
//! whose webhook is gone loses their Personal settings for Discord.

#[path = "support/fake_discord.rs"]
mod fake_discord;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use std::time::{Duration, Instant};

use connectrpc::client::HttpClient;
use fake_discord::{FakeDiscord, Posted};
use nama_proto::nama::v1::{
    EnablePluginRequest, ListPersonalSettingsRequest, PluginServiceClient, ServerServiceClient,
    ServerSettings, SettingValue, UpdatePersonalSettingsRequest, UpdatePluginSettingsRequest,
    UpdateServerSettingsRequest, UserServiceClient, setting_value::Value,
};
use server::{SignedIn, TestServer, WAIT, requests::Requester, tmdb::DUNE};

fn text(key: &str, value: &str) -> (String, SettingValue) {
    let value = SettingValue {
        value: Some(Value::TextValue(value.to_owned())),
        ..Default::default()
    };
    (key.to_owned(), value)
}

/// A TMDB-backed Server at `https://nama.example.com` with Discord enabled against `discord`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn started(discord: &FakeDiscord) -> TestServer {
    let server = TestServer::builder().tmdb().start().await;
    let plugins = PluginServiceClient::new(HttpClient::plaintext(), server.client_config());
    plugins
        .update_plugin_settings(UpdatePluginSettingsRequest {
            id: "discord".to_owned(),
            settings: std::iter::once(text("api_url", discord.url())).collect(),
            ..Default::default()
        })
        .await
        .unwrap();
    plugins
        .enable_plugin(EnablePluginRequest {
            id: "discord".to_owned(),
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

fn users(user: &SignedIn) -> UserServiceClient<HttpClient> {
    UserServiceClient::new(HttpClient::plaintext(), user.client_config())
}

/// `user` sets their Discord webhook to `webhook`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn set_webhook(user: &SignedIn, webhook: &str) {
    users(user)
        .update_personal_settings(UpdatePersonalSettingsRequest {
            plugin_id: "discord".to_owned(),
            settings: std::iter::once(text("webhook", webhook)).collect(),
            ..Default::default()
        })
        .await
        .unwrap();
}

/// Whether `user` has a Discord webhook set, as they list their Personal settings.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn has_webhook(user: &SignedIn) -> bool {
    let listed = users(user)
        .list_personal_settings(ListPersonalSettingsRequest::default())
        .await
        .unwrap()
        .into_owned();
    listed
        .plugins
        .iter()
        .filter(|plugin| plugin.plugin_id == "discord")
        .flat_map(|plugin| &plugin.settings)
        .any(|setting| setting.key == "webhook" && setting.secret_set)
}

#[tokio::test]
async fn a_request_event_reaches_each_users_own_webhook_and_never_the_others() {
    let discord = FakeDiscord::start(&[]).await;
    let server = started(&discord).await;
    let ada = server.user_with_default_permissions("Ada").await;
    let ben = server.user_with_default_permissions("Ben").await;
    // Only the path counts, whatever host a User's webhook names.
    set_webhook(&ada, "https://discord.com/api/webhooks/1/ada-token").await;
    set_webhook(&ben, "https://discordapp.com/api/webhooks/2/ben-token").await;
    let dune = server.requests(&ada).ask(DUNE).await;
    server.requests(&ben).ask(DUNE).await;

    server.approve(dune.id).await;

    let mut posted = discord.wait_for_messages(2).await;
    posted.sort_by(|one, other| one.path.cmp(&other.path));
    let to = |path: &str| Posted {
        path: path.to_owned(),
        content: format!(
            "Dune (2021) is now Approved.\nhttps://nama.example.com/requests/{}",
            dune.id
        ),
    };
    assert_eq!(
        posted,
        [
            to("/api/webhooks/1/ada-token"),
            to("/api/webhooks/2/ben-token")
        ]
    );
}

#[tokio::test]
async fn a_user_whose_webhook_is_gone_loses_their_personal_settings_for_discord() {
    let discord = FakeDiscord::start(&["gone-token"]).await;
    let server = started(&discord).await;
    let ada = server.user_with_default_permissions("Ada").await;
    let ben = server.user_with_default_permissions("Ben").await;
    set_webhook(&ada, "https://discord.com/api/webhooks/1/gone-token").await;
    set_webhook(&ben, "https://discord.com/api/webhooks/2/ben-token").await;
    let dune = server.requests(&ada).ask(DUNE).await;
    server.requests(&ben).ask(DUNE).await;

    server.approve(dune.id).await;

    discord.wait_for_messages(2).await;
    let started = Instant::now();
    while has_webhook(&ada).await {
        assert!(
            started.elapsed() < WAIT,
            "Ada still has her Discord webhook"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(has_webhook(&ben).await);
}
