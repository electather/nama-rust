//! The Telegram Installed Plugin, bundled in the Server, against a fake Telegram: its settings on
//! the Plugins page, every Request event posted to its channel, Users connecting their own chat,
//! and the messages they get there.

#[path = "support/fake_telegram.rs"]
mod fake_telegram;
#[path = "support/log.rs"]
mod log;
#[expect(dead_code, reason = "the harness's plain `start` has no use here")]
#[path = "support/server.rs"]
mod server;

use std::{collections::BTreeMap, fs, sync::Arc, time::Duration};

use connectrpc::client::{ClientConfig, HttpClient};
use fake_telegram::{BOT_TOKEN, FakeTelegram};
use log::Log;
use nama_plugin_api::settings::{SettingValue, Settings};
use nama_plugin_tmdb::{
    Tmdb,
    testing::{API_KEY as TMDB_KEY, FakeTmdb},
};
use nama_proto::nama::v1::{
    ApproveRequestRequest, CancelRequestRequest, CreateLibraryRequest, CreateRequestRequest,
    CreateUserRequest, DeletePluginConnectionRequest, DeleteUserRequest, EnablePluginRequest,
    GetPluginRequest, GetRequestRequest, LibraryKind, LibraryServiceClient,
    ListPluginConnectionsRequest, ListPluginsRequest, Permission, Plugin, PluginConnectionState,
    PluginServiceClient, Request, RequestServiceClient, RequestState, ScanLibraryRequest,
    ServerServiceClient, ServerSettings, SessionDelivery, SetPermissionsRequest, SignInRequest,
    SignInServiceClient, StartPluginConnectionRequest, UpdatePluginSettingsRequest,
    UpdateServerSettingsRequest, UserServiceClient, setting_value::Value,
};
use nama_server::{config::Config, db::Database, plugin_settings, plugins::Registry};
use serde_json::{Value as Json, json};
use server::{PASSWORD, TestServer};

/// Movies the fake TMDB knows.
const ALIEN: i64 = 348;
const DUNE: i64 = 438_631;

/// The channel the Telegram Plugin posts to.
const CHANNEL: &str = "@nama_requests";

/// The chat Ada presses Start in.
const ADA_CHAT: i64 = 987_654_321;

/// How long a test waits for a Plugin Connection.
const WAIT: Duration = Duration::from_secs(10);

/// A Server with the TMDB Plugin enabled against a fake TMDB, beside a fake Telegram, and a User,
/// Ada, whose Requests are Pending.
struct Fixture {
    server: TestServer,
    db: Arc<Database>,
    telegram: FakeTelegram,
    ada_id: i64,
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
        let (ada_id, ada) = user(&server, "Ada").await;
        let fixture = Self {
            server,
            db,
            telegram: FakeTelegram::start().await,
            ada_id,
            ada,
            _tmdb: tmdb,
        };
        // Listing is what reads the Installed Plugins, bundled ones too.
        fixture
            .plugins()
            .list_plugins(ListPluginsRequest::default())
            .await
            .unwrap();
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

    /// An Admin sets the Server URL to `url`.
    async fn set_server_url(&self, url: &str) {
        ServerServiceClient::new(HttpClient::plaintext(), self.server.client_config())
            .update_server_settings(UpdateServerSettingsRequest {
                settings: ServerSettings {
                    url: url.to_owned(),
                    ..Default::default()
                }
                .into(),
                ..Default::default()
            })
            .await
            .unwrap();
    }

    /// The Telegram Plugin as its details page gets it.
    async fn got(&self) -> Plugin {
        let request = GetPluginRequest {
            id: "telegram".to_owned(),
            ..Default::default()
        };
        let got = self.plugins().get_plugin(request).await.unwrap();
        got.into_owned().plugin.into_option().unwrap()
    }

    fn ada_users(&self) -> UserServiceClient<HttpClient> {
        UserServiceClient::new(HttpClient::plaintext(), self.ada.clone())
    }

    /// Ada's Plugin Connections as she lists them, by Plugin id.
    async fn ada_connections(&self) -> Vec<(String, PluginConnectionState)> {
        let listed = self
            .ada_users()
            .list_plugin_connections(ListPluginConnectionsRequest::default())
            .await
            .unwrap()
            .into_owned();
        listed
            .connections
            .into_iter()
            .map(|connection| (connection.plugin_id, connection.state.as_known().unwrap()))
            .collect()
    }

    /// Ada starts connecting Telegram, and gets the link to open.
    async fn ada_starts_connecting(&self) -> String {
        self.ada_users()
            .start_plugin_connection(StartPluginConnectionRequest {
                plugin_id: "telegram".to_owned(),
                ..Default::default()
            })
            .await
            .unwrap()
            .into_owned()
            .url
    }

    /// Ada connects Telegram through its deep link, pressing Start in her chat, and waits until
    /// she is connected.
    async fn connect_ada(&self) {
        let link = self.ada_starts_connecting().await;
        let (_, code) = link.split_once("?start=").unwrap();
        self.telegram.press_start(code, ADA_CHAT);
        self.wait_for_ada(PluginConnectionState::PLUGIN_CONNECTION_STATE_CONNECTED)
            .await;
    }

    /// Waits until Ada's Telegram connection is in `state`.
    async fn wait_for_ada(&self, state: PluginConnectionState) {
        let started = tokio::time::Instant::now();
        loop {
            let connections = self.ada_connections().await;
            if connections == [("telegram".to_owned(), state)] {
                return;
            }
            assert!(
                started.elapsed() < WAIT,
                "Ada's connections stayed {connections:?}"
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    /// What the Server keeps as Ada's Plugin Connection to Telegram, if anything.
    async fn adas_stored_connection(&self) -> Option<String> {
        sqlx::query_scalar(
            "SELECT connection FROM plugin_connections WHERE user_id = ? AND plugin_id = 'telegram'",
        )
        .bind(self.ada_id)
        .fetch_optional(self.db.reader())
        .await
        .unwrap()
    }

    fn plugins(&self) -> PluginServiceClient<HttpClient> {
        PluginServiceClient::new(HttpClient::plaintext(), self.server.client_config())
    }

    /// Ada asks for the Movie `tmdb_id`, which opens a Pending Request.
    async fn ada_asks_for(&self, tmdb_id: i64) -> Request {
        asks_for(&self.ada, tmdb_id).await
    }

    /// An Admin gives Ada `MANAGE_REQUESTS` on top of what a new User gets.
    async fn ada_manages_requests(&self) {
        UserServiceClient::new(HttpClient::plaintext(), self.server.client_config())
            .set_permissions(SetPermissionsRequest {
                user_id: self.ada_id,
                permissions: [
                    Permission::Request,
                    Permission::ViewRequests,
                    Permission::ManageRequests,
                ]
                .map(Into::into)
                .into(),
                ..Default::default()
            })
            .await
            .unwrap();
    }

    fn requests(&self) -> RequestServiceClient<HttpClient> {
        RequestServiceClient::new(HttpClient::plaintext(), self.server.client_config())
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

/// Creates a User named `name` with what a new User gets, signs them in, and returns their id and
/// client settings that call as them.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn user(server: &TestServer, name: &str) -> (i64, ClientConfig) {
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
    (id, server.client_config_as(&token))
}

/// The User whose client settings are `user` asks for the Movie `tmdb_id`, which opens a Pending
/// Request.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn asks_for(user: &ClientConfig, tmdb_id: i64) -> Request {
    RequestServiceClient::new(HttpClient::plaintext(), user.clone())
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

fn text(key: &str, value: &str) -> (String, nama_proto::nama::v1::SettingValue) {
    let value = nama_proto::nama::v1::SettingValue {
        value: Some(Value::TextValue(value.to_owned())),
        ..Default::default()
    };
    (key.to_owned(), value)
}

/// A message to the channel saying `text`, as `sendMessage` takes it.
fn to_channel(text: &str) -> Json {
    json!({ "chat_id": CHANNEL, "text": text })
}

/// A message to Ada's own chat saying `text`, as `sendMessage` takes it.
fn to_ada(text: &str) -> Json {
    json!({ "chat_id": ADA_CHAT.to_string(), "text": text })
}

/// `messages`, in an order that doesn't depend on which delivery finished first.
fn sorted(mut messages: Vec<Json>) -> Vec<Json> {
    messages.sort_by_key(Json::to_string);
    messages
}

#[tokio::test]
async fn a_new_pending_request_is_posted_to_the_channel_with_no_link_while_no_server_url_is_set() {
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
async fn with_a_server_url_set_a_message_links_to_the_request_in_the_web_app() {
    let fixture = Fixture::with_telegram(BOT_TOKEN, Some(CHANNEL)).await;
    fixture.set_server_url("https://nama.example.com/").await;

    let dune = fixture.ada_asks_for(DUNE).await;

    let sent = fixture.telegram.wait_for_messages(1).await;
    let link = format!("https://nama.example.com/requests/{}", dune.id);
    assert_eq!(
        sent,
        [to_channel(&format!(
            "New Request: Dune (2021), from Ada, is Pending.\n{link}"
        ))]
    );
}

#[tokio::test]
async fn a_cancelled_request_has_no_link_since_it_is_gone_from_the_web_app() {
    let fixture = Fixture::with_telegram(BOT_TOKEN, Some(CHANNEL)).await;
    fixture.set_server_url("https://nama.example.com").await;
    let dune = fixture.ada_asks_for(DUNE).await;

    fixture
        .requests()
        .cancel_request(CancelRequestRequest {
            id: dune.id,
            ..Default::default()
        })
        .await
        .unwrap();

    let sent = fixture.telegram.wait_for_messages(2).await;
    assert!(
        sent.contains(&to_channel("The Request for Dune (2021) was cancelled.")),
        "{sent:?}"
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
    // Time for the delivery about Alien to run: once the settings change it would be skipped,
    // and this test would pass without trying it.
    tokio::time::sleep(Duration::from_millis(500)).await;

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

// One test for both, since the log is the whole test binary's and their lines are alike.
#[tokio::test]
async fn a_refused_or_malformed_bot_token_is_logged_and_the_request_is_unaffected() {
    let log = Log::global();
    let refused = [
        "telegram",
        "provider failed",
        "Telegram refused the bot token",
    ];
    // Telegram answers 401 to an unknown token, and 404 to one not shaped like a token.
    for (times, token) in [(1, "654321:not-the-token"), (2, "not-a-token")] {
        let fixture = Fixture::with_telegram(token, Some(CHANNEL)).await;

        let dune = fixture.ada_asks_for(DUNE).await;

        log.wait_for(&refused, times).await;
        assert_eq!(fixture.get(dune.id).await.state, RequestState::Pending);
        assert!(!log.contents().contains(token), "{}", log.contents());
    }
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
async fn an_answer_that_is_not_json_is_logged_as_making_no_sense() {
    let log = Log::global();
    let fixture = Fixture::with_telegram(BOT_TOKEN, Some(CHANNEL)).await;
    fixture.telegram.talk_nonsense();

    fixture.ada_asks_for(DUNE).await;

    log.wait_for(
        &[
            "telegram",
            "provider failed",
            "Telegram's answer made no sense",
        ],
        1,
    )
    .await;
    assert!(!log.contents().contains(BOT_TOKEN), "{}", log.contents());
}

#[tokio::test]
async fn telegrams_details_page_gets_its_three_settings_without_the_bot_token() {
    let fixture = Fixture::start().await;
    fixture.set_up_telegram(BOT_TOKEN, Some(CHANNEL)).await;

    let telegram = fixture.got().await;

    let settings: Vec<_> = telegram
        .settings
        .iter()
        .map(|setting| (setting.key.as_str(), setting.value.as_option().is_some()))
        .collect();
    assert_eq!(
        settings,
        [("bot_token", false), ("channel", true), ("api_url", true)]
    );
    let token = &telegram.settings[0];
    assert!(token.secret_set);
    assert!(!format!("{telegram:?}").contains(BOT_TOKEN), "{telegram:?}");
}

#[tokio::test]
async fn connecting_through_the_deep_link_stores_the_chat_id_as_the_users_plugin_connection_unseen_by_them()
 {
    let fixture = Fixture::with_telegram(BOT_TOKEN, None).await;
    let before = fixture.ada_connections().await;

    let link = fixture.ada_starts_connecting().await;
    let waiting = fixture.ada_connections().await;
    let (bot, code) = link.split_once("?start=").unwrap();
    fixture.telegram.press_start(code, ADA_CHAT);
    fixture
        .wait_for_ada(PluginConnectionState::PLUGIN_CONNECTION_STATE_CONNECTED)
        .await;
    let listed = fixture
        .ada_users()
        .list_plugin_connections(ListPluginConnectionsRequest::default())
        .await
        .unwrap()
        .into_owned();

    let telegram = |state| vec![("telegram".to_owned(), state)];
    assert_eq!(
        before,
        telegram(PluginConnectionState::PLUGIN_CONNECTION_STATE_NOT_CONNECTED)
    );
    assert_eq!(
        waiting,
        telegram(PluginConnectionState::PLUGIN_CONNECTION_STATE_WAITING)
    );
    assert_eq!(bot, "https://t.me/nama_test_bot");
    assert!(code.len() >= 32, "the one-time code {code:?} is guessable");
    assert_eq!(
        fixture.adas_stored_connection().await.as_deref(),
        Some("987654321")
    );
    assert!(
        !format!("{listed:?}").contains("987654321"),
        "the chat id reached Ada: {listed:?}"
    );
}

#[tokio::test]
async fn a_start_with_another_code_connects_nobody() {
    let fixture = Fixture::with_telegram(BOT_TOKEN, None).await;
    fixture.ada_starts_connecting().await;

    fixture.telegram.press_start("someone-elses-code", ADA_CHAT);
    // Two polls' time.
    tokio::time::sleep(Duration::from_secs(5)).await;

    assert_eq!(
        fixture.ada_connections().await,
        [(
            "telegram".to_owned(),
            PluginConnectionState::PLUGIN_CONNECTION_STATE_WAITING
        )]
    );
    assert_eq!(fixture.adas_stored_connection().await, None);
}

#[tokio::test]
async fn disconnecting_deletes_the_plugin_connection() {
    let fixture = Fixture::with_telegram(BOT_TOKEN, None).await;
    fixture.connect_ada().await;

    fixture
        .ada_users()
        .delete_plugin_connection(DeletePluginConnectionRequest {
            plugin_id: "telegram".to_owned(),
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(
        fixture.ada_connections().await,
        [(
            "telegram".to_owned(),
            PluginConnectionState::PLUGIN_CONNECTION_STATE_NOT_CONNECTED
        )]
    );
    assert_eq!(fixture.adas_stored_connection().await, None);
}

#[tokio::test]
async fn deleting_the_user_deletes_their_plugin_connection() {
    let fixture = Fixture::with_telegram(BOT_TOKEN, None).await;
    fixture.connect_ada().await;

    UserServiceClient::new(HttpClient::plaintext(), fixture.server.client_config())
        .delete_user(DeleteUserRequest {
            user_id: fixture.ada_id,
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(fixture.adas_stored_connection().await, None);
}

#[tokio::test]
async fn telegram_is_not_listed_to_connect_while_disabled() {
    let fixture = Fixture::start().await;
    fixture.set_up_telegram(BOT_TOKEN, None).await;

    let listed = fixture.ada_connections().await;
    let refused = fixture
        .ada_users()
        .start_plugin_connection(StartPluginConnectionRequest {
            plugin_id: "telegram".to_owned(),
            ..Default::default()
        })
        .await
        .unwrap_err();

    assert_eq!(listed, []);
    assert_eq!(refused.code, connectrpc::ErrorCode::NotFound);
}

#[tokio::test]
async fn a_requester_who_connected_is_messaged_with_the_link_when_their_request_becomes_available()
{
    let fixture = Fixture::with_telegram(BOT_TOKEN, None).await;
    fixture.set_server_url("https://nama.example.com").await;
    fixture.connect_ada().await;
    let alien = fixture.ada_asks_for(ALIEN).await;
    fixture.approve(alien.id).await;

    let _media = fixture.library_with_alien().await;

    let sent = fixture.telegram.wait_for_messages(2).await;
    let link = format!("https://nama.example.com/requests/{}", alien.id);
    assert_eq!(
        sorted(sent),
        sorted(vec![
            to_ada(&format!("Alien (1979) is now Approved.\n{link}")),
            to_ada(&format!("Alien (1979) is now Available.\n{link}")),
        ])
    );
}

#[tokio::test]
async fn a_user_with_manage_requests_who_connected_is_messaged_about_a_new_pending_request() {
    let fixture = Fixture::with_telegram(BOT_TOKEN, None).await;
    fixture.connect_ada().await;
    fixture.ada_manages_requests().await;
    let (_, ben) = user(&fixture.server, "Ben").await;

    asks_for(&ben, DUNE).await;

    let sent = fixture.telegram.wait_for_messages(1).await;
    assert_eq!(
        sent,
        [to_ada("New Request: Dune (2021), from Ben, is Pending.")]
    );
}

#[tokio::test]
async fn a_requester_who_did_not_connect_is_skipped_and_the_channel_still_posts() {
    let fixture = Fixture::with_telegram(BOT_TOKEN, Some(CHANNEL)).await;
    fixture.connect_ada().await;
    let (_, ben) = user(&fixture.server, "Ben").await;
    let dune = asks_for(&ben, DUNE).await;
    fixture.ada_asks_for(DUNE).await;

    fixture.approve(dune.id).await;

    let sent = fixture.telegram.wait_for_messages(4).await;
    assert_eq!(
        sorted(sent),
        sorted(vec![
            to_channel("New Request: Dune (2021), from Ben, is Pending."),
            to_channel("Ada also asked for Dune (2021)."),
            to_channel("Dune (2021) is now Approved."),
            to_ada("Dune (2021) is now Approved."),
        ])
    );
}

#[tokio::test]
async fn a_user_who_blocked_the_bot_is_disconnected_and_messaged_no_more() {
    let fixture = Fixture::with_telegram(BOT_TOKEN, Some(CHANNEL)).await;
    fixture.connect_ada().await;
    fixture.telegram.block(ADA_CHAT);
    let alien = fixture.ada_asks_for(ALIEN).await;

    fixture.approve(alien.id).await;
    fixture
        .wait_for_ada(PluginConnectionState::PLUGIN_CONNECTION_STATE_NOT_CONNECTED)
        .await;
    let _media = fixture.library_with_alien().await;
    fixture.telegram.wait_for_messages(4).await;
    // Time for a message to Ada about Alien becoming Available, which would make a fifth.
    tokio::time::sleep(Duration::from_millis(500)).await;

    assert_eq!(
        sorted(fixture.telegram.sent()),
        sorted(vec![
            to_channel("New Request: Alien (1979), from Ada, is Pending."),
            to_channel("Alien (1979) is now Approved."),
            to_ada("Alien (1979) is now Approved."),
            to_channel("Alien (1979) is now Available."),
        ])
    );
    assert_eq!(fixture.adas_stored_connection().await, None);
}

#[tokio::test]
async fn a_message_failing_for_another_reason_leaves_the_user_connected() {
    let log = Log::global();
    let failed = [
        "telegram",
        "provider failed",
        "to User",
        "Telegram didn't answer",
    ];
    let failed_before = log.count(&failed);
    let fixture = Fixture::with_telegram(BOT_TOKEN, None).await;
    fixture.connect_ada().await;
    let alien = fixture.ada_asks_for(ALIEN).await;
    fixture.telegram.go_down();

    fixture.approve(alien.id).await;

    log.wait_for(&failed, failed_before + 1).await;
    assert_eq!(
        fixture.adas_stored_connection().await.as_deref(),
        Some("987654321")
    );
}
