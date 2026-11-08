//! The Telegram Installed Plugin, bundled in the Server, against a fake Telegram: its settings on
//! the Plugins page, and every Request event posted to its channel.

#[path = "support/fake_telegram.rs"]
mod fake_telegram;
#[path = "support/log.rs"]
mod log;
#[expect(dead_code, reason = "the harness's plain `start` has no use here")]
#[path = "support/server.rs"]
mod server;

use std::{collections::BTreeMap, fs, sync::Arc};

use connectrpc::client::{ClientConfig, HttpClient};
use fake_telegram::{BOT_TOKEN, FakeTelegram};
use log::Log;
use nama_plugin_api::settings::{SettingValue, Settings};
use nama_plugin_tmdb::{
    Tmdb,
    testing::{API_KEY as TMDB_KEY, FakeTmdb},
};
use nama_proto::nama::v1::{
    ApproveRequestRequest, CreateLibraryRequest, CreateRequestRequest, CreateUserRequest,
    EnablePluginRequest, GetRequestRequest, LibraryKind, LibraryServiceClient, ListPluginsRequest,
    Permission, Plugin, PluginServiceClient, Request, RequestServiceClient, RequestState,
    ScanLibraryRequest, SessionDelivery, SetPermissionsRequest, SignInRequest, SignInServiceClient,
    UpdatePluginSettingsRequest, UserServiceClient, setting_value::Value,
};
use nama_server::{config::Config, plugin_settings, plugins::Registry};
use serde_json::{Value as Json, json};
use server::{PASSWORD, TestServer};

/// Movies the fake TMDB knows.
const ALIEN: i64 = 348;
const DUNE: i64 = 438_631;

/// The channel the Telegram Plugin posts to.
const CHANNEL: &str = "@nama_requests";

/// A Server with the TMDB Plugin enabled against a fake TMDB, beside a fake Telegram, and a
/// member, Ada, whose Requests are Pending.
struct Fixture {
    server: TestServer,
    telegram: FakeTelegram,
    ada: ClientConfig,
    _tmdb: FakeTmdb,
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
impl Fixture {
    async fn start() -> Self {
        let tmdb = FakeTmdb::start().await;
        let plugins = Registry::new();
        let settings = Settings::new(BTreeMap::from([(
            "api_key".to_owned(),
            SettingValue::Text(TMDB_KEY.to_owned()),
        )]));
        plugins
            .enable_with_settings(Arc::new(Tmdb::with_endpoints(tmdb.endpoints())), settings)
            .unwrap();
        let (server, db) = TestServer::start_with_db(Config::default(), plugins).await;
        // As on a Server's first start.
        plugin_settings::import(&db, &BTreeMap::new())
            .await
            .unwrap();
        let ada = member(&server, "Ada").await;
        let fixture = Self {
            server,
            telegram: FakeTelegram::start().await,
            ada,
            _tmdb: tmdb,
        };
        // Listing is what reads the Installed Plugins, bundled ones too.
        fixture.listed().await;
        fixture
    }

    /// A Fixture where an Admin set Telegram up with `bot_token`, the fake Telegram's URL and
    /// `channel`, if any, and enabled it.
    async fn with_telegram(bot_token: &str, channel: Option<&str>) -> Self {
        let fixture = Self::start().await;
        fixture.set_up_telegram(bot_token, channel).await;
        fixture
            .plugins()
            .enable_plugin(EnablePluginRequest {
                id: "telegram".to_owned(),
                ..Default::default()
            })
            .await
            .unwrap();
        fixture
    }

    /// Saves Telegram's settings: `bot_token`, the fake Telegram's URL and `channel`, if any.
    async fn set_up_telegram(&self, bot_token: &str, channel: Option<&str>) {
        let mut settings = vec![
            text("bot_token", bot_token),
            text("api_url", self.telegram.url()),
        ];
        settings.extend(channel.map(|channel| text("channel", channel)));
        self.plugins()
            .update_plugin_settings(UpdatePluginSettingsRequest {
                id: "telegram".to_owned(),
                settings: settings.into_iter().collect(),
                ..Default::default()
            })
            .await
            .unwrap();
    }

    /// The Telegram Plugin as the Plugins page lists it.
    async fn listed(&self) -> Option<Plugin> {
        let listed = self
            .plugins()
            .list_plugins(ListPluginsRequest::default())
            .await
            .unwrap()
            .into_owned();
        listed
            .plugins
            .into_iter()
            .find(|plugin| plugin.id == "telegram")
    }

    fn plugins(&self) -> PluginServiceClient<HttpClient> {
        PluginServiceClient::new(HttpClient::plaintext(), self.server.client_config())
    }

    fn requests(&self) -> RequestServiceClient<HttpClient> {
        RequestServiceClient::new(HttpClient::plaintext(), self.server.client_config())
    }

    /// Ada asks for the Movie `tmdb_id`, which opens a Pending Request.
    async fn ada_asks_for(&self, tmdb_id: i64) -> Request {
        RequestServiceClient::new(HttpClient::plaintext(), self.ada.clone())
            .create_request(CreateRequestRequest {
                movie_tmdb_id: tmdb_id,
                ..Default::default()
            })
            .await
            .unwrap()
            .into_owned()
            .request
            .into_option()
            .unwrap()
    }

    async fn approve(&self, id: i64) {
        self.requests()
            .approve_request(ApproveRequestRequest {
                id,
                ..Default::default()
            })
            .await
            .unwrap();
    }

    async fn get(&self, id: i64) -> Request {
        self.requests()
            .get_request(GetRequestRequest {
                id,
                ..Default::default()
            })
            .await
            .unwrap()
            .into_owned()
            .request
            .into_option()
            .unwrap()
    }

    /// Creates a Movies Library holding Alien and starts scanning it.
    async fn library_with_alien(&self) -> tempfile::TempDir {
        let media = tempfile::tempdir().unwrap();
        let folder = media.path().canonicalize().unwrap();
        let file = folder.join("Alien (1979)/Alien (1979).mkv");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, b"video").unwrap();
        let config = self.server.client_config();
        let libraries = LibraryServiceClient::new(HttpClient::plaintext(), config);
        let id = libraries
            .create_library(CreateLibraryRequest {
                name: "Movies".to_owned(),
                kind: LibraryKind::LIBRARY_KIND_MOVIES.into(),
                folders: vec![folder.to_str().unwrap().to_owned()],
                ..Default::default()
            })
            .await
            .unwrap()
            .into_owned()
            .library
            .into_option()
            .unwrap()
            .id;
        libraries
            .scan_library(ScanLibraryRequest {
                id,
                ..Default::default()
            })
            .await
            .unwrap();
        media
    }
}

/// Creates a User named `name` with what a new User gets, signs them in, and returns client
/// settings that call as them.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn member(server: &TestServer, name: &str) -> ClientConfig {
    let as_admin = UserServiceClient::new(HttpClient::plaintext(), server.client_config());
    let id = as_admin
        .create_user(CreateUserRequest {
            name: name.to_owned(),
            password: PASSWORD.to_owned(),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .id;
    as_admin
        .set_permissions(SetPermissionsRequest {
            user_id: id,
            permissions: [Permission::Request, Permission::ViewRequests]
                .map(Into::into)
                .into(),
            ..Default::default()
        })
        .await
        .unwrap();
    let token = SignInServiceClient::new(HttpClient::plaintext(), server.anonymous_client_config())
        .sign_in(SignInRequest {
            name: name.to_owned(),
            password: PASSWORD.to_owned(),
            device_name: "Phone".to_owned(),
            delivery: SessionDelivery::SESSION_DELIVERY_TOKEN.into(),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .token;
    server.client_config_as(&token)
}

fn text(key: &str, text: &str) -> (String, nama_proto::nama::v1::SettingValue) {
    let value = nama_proto::nama::v1::SettingValue {
        value: Some(Value::TextValue(text.to_owned())),
        ..Default::default()
    };
    (key.to_owned(), value)
}

/// A message to the channel saying `text`, as `sendMessage` takes it.
fn to_channel(text: &str) -> Json {
    json!({ "chat_id": CHANNEL, "text": text })
}

#[tokio::test]
async fn a_new_pending_request_is_posted_to_the_channel() {
    let fixture = Fixture::with_telegram(BOT_TOKEN, Some(CHANNEL)).await;

    fixture.ada_asks_for(DUNE).await;

    let sent = fixture.telegram.wait_for_messages(1).await;
    assert_eq!(
        sent,
        [to_channel(
            "New Request: Dune (2021), from Ada, is Pending."
        )]
    );
}

#[tokio::test]
async fn a_request_becoming_available_is_posted_to_the_channel() {
    let fixture = Fixture::with_telegram(BOT_TOKEN, Some(CHANNEL)).await;
    let alien = fixture.ada_asks_for(ALIEN).await;
    fixture.approve(alien.id).await;

    let _media = fixture.library_with_alien().await;

    let sent = fixture.telegram.wait_for_messages(3).await;
    assert!(
        sent.contains(&to_channel("Alien (1979) is now Available.")),
        "{sent:?}"
    );
}

#[tokio::test]
async fn with_no_channel_set_nothing_is_posted() {
    let fixture = Fixture::with_telegram(BOT_TOKEN, None).await;
    fixture.ada_asks_for(ALIEN).await;

    // Whatever was to be posted about Alien was handed over before this.
    fixture.set_up_telegram(BOT_TOKEN, Some(CHANNEL)).await;
    fixture.ada_asks_for(DUNE).await;

    let sent = fixture.telegram.wait_for_messages(1).await;
    assert_eq!(
        sent,
        [to_channel(
            "New Request: Dune (2021), from Ada, is Pending."
        )]
    );
}

#[tokio::test]
async fn a_refused_bot_token_is_logged_and_the_request_is_unaffected() {
    let log = Log::global();
    let fixture = Fixture::with_telegram("654321:not-the-token", Some(CHANNEL)).await;

    let dune = fixture.ada_asks_for(DUNE).await;

    log.wait_for(
        &[
            "telegram",
            "provider failed",
            "Telegram refused the bot token",
        ],
        1,
    )
    .await;
    assert_eq!(fixture.get(dune.id).await.state, RequestState::Pending);
    assert!(
        !log.contents().contains("not-the-token"),
        "{}",
        log.contents()
    );
}

#[tokio::test]
async fn an_unreachable_telegram_is_logged_in_plain_words() {
    let log = Log::global();
    let fixture = Fixture::with_telegram(BOT_TOKEN, Some(CHANNEL)).await;
    fixture.telegram.go_down();

    fixture.ada_asks_for(DUNE).await;

    log.wait_for(
        &["telegram", "provider failed", "Telegram didn't answer"],
        1,
    )
    .await;
    assert!(!log.contents().contains(BOT_TOKEN), "{}", log.contents());
}

#[tokio::test]
async fn the_plugins_page_lists_telegrams_three_settings_without_the_bot_token() {
    let fixture = Fixture::start().await;
    fixture.set_up_telegram(BOT_TOKEN, Some(CHANNEL)).await;

    let listed = fixture.listed().await.expect("Telegram is listed");

    let settings: Vec<_> = listed
        .settings
        .iter()
        .map(|setting| (setting.key.as_str(), setting.value.as_option().is_some()))
        .collect();
    assert_eq!(
        settings,
        [("bot_token", false), ("channel", true), ("api_url", true)]
    );
    let token = &listed.settings[0];
    assert!(token.secret_set);
    assert!(!format!("{listed:?}").contains(BOT_TOKEN), "{listed:?}");
}
