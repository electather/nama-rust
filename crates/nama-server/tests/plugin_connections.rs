//! What the Kernel does with a Plugin that connects Users to an outside service and notifies them
//! there (ADR 0021), through an in-process chat Plugin: Users connecting and disconnecting, and
//! who is told about each Request event.

#[path = "support/log.rs"]
mod log;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::Duration,
};

use connectrpc::client::HttpClient;
use log::Log;
use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    connector::{ConnectionStarted, Connector},
    notifier::{Notification, Notifier, Recipient, RecipientGone},
};
use nama_plugin_proto::nama::plugin::v1 as wire;
use nama_proto::nama::v1::{
    CancelRequestRequest, DeletePluginConnectionRequest, DeleteUserRequest, LibraryKind,
    ListPluginConnectionsRequest, Permission, PluginConnectionState, Request, ServerServiceClient,
    ServerSettings, SetPermissionsRequest, StartPluginConnectionRequest,
    UpdateServerSettingsRequest, UserServiceClient,
};
use nama_server::plugins::Registry;
use server::{
    Library, SignedIn, TestServer, WAIT,
    requests::Requester,
    tmdb::{ALIEN, DUNE},
};

/// The chat Plugin's id.
const CHAT: &str = "chat";

/// Ada's chat, as the chat Plugin names it.
const ADA_CHAT: &str = "ada-chat";

/// The link the Server gives to the Request `id`, once an Admin set its URL.
fn link(id: i64) -> String {
    format!("https://nama.example.com/requests/{id}")
}

/// What a notification told about its Request.
#[derive(Debug, Clone, PartialEq)]
enum Told {
    New,
    Joined,
    Now(wire::RequestState),
    Cancelled,
}

impl Told {
    #[expect(
        clippy::panic,
        reason = "the Kernel hands a Notifier only Request events; another fails its delivery, which a test then misses"
    )]
    fn of(notification: &Notification) -> Self {
        match &notification.event.event {
            Some(wire::event::Event::RequestCreated(_)) => Self::New,
            Some(wire::event::Event::RequesterAdded(_)) => Self::Joined,
            Some(wire::event::Event::RequestStateChanged(_)) => {
                Self::Now(notification.request.state.as_known().unwrap_or_default())
            }
            Some(wire::event::Event::RequestCancelled(_)) => Self::Cancelled,
            other => panic!("told about {other:?}"),
        }
    }
}

/// Whom a notification was handed for, what it told, and the link it came with.
type Delivery = (Recipient, Told, Option<String>);

/// `deliveries`, in an order that doesn't depend on which delivery ran first.
fn sorted(mut deliveries: Vec<Delivery>) -> Vec<Delivery> {
    deliveries.sort_by_key(|delivery| format!("{delivery:?}"));
    deliveries
}

/// The chat service, as its Plugin sees it.
#[derive(Default)]
struct Service {
    /// The chat that sent each code.
    started: HashMap<String, String>,
    /// Chats that blocked the bot.
    blocked: HashSet<String>,
    down: bool,
    /// Every notification handed to the Notifier, delivered or not.
    handed: Vec<Delivery>,
}

/// An in-process Plugin for a chat service: its Connector links a User by the code their chat
/// sends, and its Notifier keeps every notification it is handed, failing while the service is
/// down and for a chat that blocked the bot.
#[derive(Clone, Default)]
struct Chat(Arc<Mutex<Service>>);

impl Chat {
    fn service(&self) -> MutexGuard<'_, Service> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// `chat` sends the code `code`, as a User finishing with their link does.
    fn send_code(&self, code: &str, chat: &str) {
        self.service()
            .started
            .insert(code.to_owned(), chat.to_owned());
    }

    /// The notifications handed so far, once there are `count`, in an order that doesn't depend
    /// on which delivery ran first.
    async fn wait_for_handed(&self, count: usize) -> Vec<Delivery> {
        let started = tokio::time::Instant::now();
        while self.service().handed.len() < count {
            assert!(
                started.elapsed() < WAIT,
                "handed only {:?}",
                self.service().handed
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        sorted(self.service().handed.clone())
    }
}

impl Plugin for Chat {
    fn manifest(&self) -> Manifest {
        Manifest {
            version: "1.0.0".to_owned(),
            extension_points: vec![
                PointKey::provide::<dyn Connector>(),
                PointKey::provide::<dyn Notifier>(),
            ],
            ..Manifest::new(PluginId::from_static(CHAT))
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        registrar.provide::<dyn Connector>(Priority::DEFAULT, Arc::new(self.clone()));
        registrar.provide::<dyn Notifier>(Priority::DEFAULT, Arc::new(self.clone()));
        Ok(())
    }
}

impl Connector for Chat {
    fn start_connection<'a>(
        &'a self,
        code: &'a str,
    ) -> BoxFuture<'a, Result<ConnectionStarted, PluginError>> {
        Box::pin(async move {
            Ok(ConnectionStarted {
                url: format!("https://chat.example.com/start?code={code}"),
                pending: code.to_owned(),
                ..ConnectionStarted::default()
            })
        })
    }

    fn poll_connection<'a>(
        &'a self,
        pending: &'a str,
    ) -> BoxFuture<'a, Result<Option<String>, PluginError>> {
        let chat = self.service().started.get(pending).cloned();
        Box::pin(async move { Ok(chat) })
    }
}

impl Notifier for Chat {
    fn deliver<'a>(
        &'a self,
        notification: &'a Notification,
        recipient: &'a Recipient,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        let mut service = self.service();
        let link = notification.request_url.clone();
        service
            .handed
            .push((recipient.clone(), Told::of(notification), link));
        let delivered = if service.down {
            Err("the chat service is down".into())
        } else if matches!(recipient, Recipient::Connection(chat) if service.blocked.contains(chat))
        {
            Err(RecipientGone.into())
        } else {
            Ok(())
        };
        drop(service);
        Box::pin(async move { delivered })
    }
}

/// A Server with TMDB on and the chat Plugin enabled, and a User, Ada, whose Requests are
/// Pending.
struct Fixture {
    server: TestServer,
    plugins: Registry,
    chat: Chat,
    ada: SignedIn,
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
impl Fixture {
    async fn start() -> Self {
        let chat = Chat::default();
        let plugins = Registry::new();
        plugins.enable(Arc::new(chat.clone())).unwrap();
        let server = TestServer::builder()
            .plugins(plugins.clone())
            .tmdb()
            .start()
            .await;
        let ada = server.user_with_default_permissions("Ada").await;
        Self {
            server,
            plugins,
            chat,
            ada,
        }
    }

    /// An Admin sets the Server URL, so notifications link to their Request.
    async fn set_server_url(&self) {
        ServerServiceClient::new(HttpClient::plaintext(), self.server.client_config())
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
    }

    fn ada_users(&self) -> UserServiceClient<HttpClient> {
        UserServiceClient::new(HttpClient::plaintext(), self.ada.client_config())
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

    /// Ada starts connecting the chat Plugin, and gets the link to open.
    async fn ada_starts_connecting(&self) -> String {
        self.ada_users()
            .start_plugin_connection(StartPluginConnectionRequest {
                plugin_id: CHAT.to_owned(),
                ..Default::default()
            })
            .await
            .unwrap()
            .into_owned()
            .url
    }

    /// Ada connects through her link, her chat sending its code, and waits until she is
    /// connected.
    async fn connect_ada(&self) {
        let link = self.ada_starts_connecting().await;
        let (_, code) = link.split_once("?code=").unwrap();
        self.chat.send_code(code, ADA_CHAT);
        self.wait_for_ada(PluginConnectionState::PLUGIN_CONNECTION_STATE_CONNECTED)
            .await;
    }

    /// Waits until Ada's connection to the chat Plugin is in `state`.
    async fn wait_for_ada(&self, state: PluginConnectionState) {
        let started = tokio::time::Instant::now();
        loop {
            let connections = self.ada_connections().await;
            if connections == [(CHAT.to_owned(), state)] {
                return;
            }
            assert!(
                started.elapsed() < WAIT,
                "Ada's connections stayed {connections:?}"
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    /// What the Server keeps as Ada's Plugin Connection to the chat Plugin, if anything.
    async fn adas_stored_connection(&self) -> Option<String> {
        sqlx::query_scalar(
            "SELECT connection FROM plugin_connections WHERE user_id = ? AND plugin_id = ?",
        )
        .bind(self.ada.id)
        .bind(CHAT)
        .fetch_optional(self.server.db().reader())
        .await
        .unwrap()
    }

    /// Ada asks for the Movie `tmdb_id`, which opens a Pending Request.
    async fn ada_asks_for(&self, tmdb_id: i64) -> Request {
        self.server.requests(&self.ada).ask(tmdb_id).await
    }

    /// An Admin gives Ada `MANAGE_REQUESTS` on top of what a new User gets.
    async fn ada_manages_requests(&self) {
        UserServiceClient::new(HttpClient::plaintext(), self.server.client_config())
            .set_permissions(SetPermissionsRequest {
                user_id: self.ada.id,
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

    /// Creates a Movies Library holding Alien and starts scanning it.
    async fn library_with_alien(&self) -> Library {
        self.server
            .library(
                LibraryKind::LIBRARY_KIND_MOVIES,
                &["Alien (1979)/Alien (1979).mkv"],
            )
            .await
    }
}

/// Ada's chat, as the Kernel hands it to the Notifier.
fn to_ada() -> Recipient {
    Recipient::Connection(ADA_CHAT.to_owned())
}

#[tokio::test]
async fn connecting_through_the_link_stores_the_connectors_answer_as_the_users_plugin_connection_unseen_by_them()
 {
    let fixture = Fixture::start().await;
    let before = fixture.ada_connections().await;

    let link = fixture.ada_starts_connecting().await;
    let waiting = fixture.ada_connections().await;
    let (start, code) = link.split_once("?code=").unwrap();
    fixture.chat.send_code(code, ADA_CHAT);
    fixture
        .wait_for_ada(PluginConnectionState::PLUGIN_CONNECTION_STATE_CONNECTED)
        .await;
    let listed = fixture
        .ada_users()
        .list_plugin_connections(ListPluginConnectionsRequest::default())
        .await
        .unwrap()
        .into_owned();

    let chat = |state| vec![(CHAT.to_owned(), state)];
    assert_eq!(
        before,
        chat(PluginConnectionState::PLUGIN_CONNECTION_STATE_NOT_CONNECTED)
    );
    assert_eq!(
        waiting,
        chat(PluginConnectionState::PLUGIN_CONNECTION_STATE_WAITING)
    );
    assert_eq!(start, "https://chat.example.com/start");
    assert!(code.len() >= 32, "the one-time code {code:?} is guessable");
    assert_eq!(
        fixture.adas_stored_connection().await.as_deref(),
        Some(ADA_CHAT)
    );
    assert!(
        !format!("{listed:?}").contains(ADA_CHAT),
        "the connection reached Ada: {listed:?}"
    );
}

#[tokio::test]
async fn disconnecting_deletes_the_plugin_connection() {
    let fixture = Fixture::start().await;
    fixture.connect_ada().await;

    fixture
        .ada_users()
        .delete_plugin_connection(DeletePluginConnectionRequest {
            plugin_id: CHAT.to_owned(),
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(
        fixture.ada_connections().await,
        [(
            CHAT.to_owned(),
            PluginConnectionState::PLUGIN_CONNECTION_STATE_NOT_CONNECTED
        )]
    );
    assert_eq!(fixture.adas_stored_connection().await, None);
}

#[tokio::test]
async fn deleting_the_user_deletes_their_plugin_connection() {
    let fixture = Fixture::start().await;
    fixture.connect_ada().await;

    UserServiceClient::new(HttpClient::plaintext(), fixture.server.client_config())
        .delete_user(DeleteUserRequest {
            user_id: fixture.ada.id,
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(fixture.adas_stored_connection().await, None);
}

#[tokio::test]
async fn a_disabled_connector_is_not_listed_to_connect() {
    let fixture = Fixture::start().await;
    fixture.plugins.disable(&PluginId::from_static(CHAT)).await;

    let listed = fixture.ada_connections().await;
    let refused = fixture
        .ada_users()
        .start_plugin_connection(StartPluginConnectionRequest {
            plugin_id: CHAT.to_owned(),
            ..Default::default()
        })
        .await
        .unwrap_err();

    assert_eq!(listed, []);
    assert_eq!(refused.code, connectrpc::ErrorCode::NotFound);
}

#[tokio::test]
async fn a_requester_who_connected_is_told_with_the_link_when_their_request_is_approved_and_becomes_available()
 {
    let fixture = Fixture::start().await;
    fixture.set_server_url().await;
    fixture.connect_ada().await;
    let alien = fixture.ada_asks_for(ALIEN).await;
    fixture.server.approve(alien.id).await;

    let _media = fixture.library_with_alien().await;

    let link = Some(link(alien.id));
    let approved = Told::Now(wire::RequestState::REQUEST_STATE_APPROVED);
    let available = Told::Now(wire::RequestState::REQUEST_STATE_AVAILABLE);
    assert_eq!(
        fixture.chat.wait_for_handed(5).await,
        sorted(vec![
            (Recipient::Channel, Told::New, link.clone()),
            (Recipient::Channel, approved.clone(), link.clone()),
            (Recipient::Channel, available.clone(), link.clone()),
            (to_ada(), approved, link.clone()),
            (to_ada(), available, link),
        ])
    );
}

#[tokio::test]
async fn a_user_with_manage_requests_who_connected_is_told_about_a_new_pending_request_with_no_link_while_no_server_url_is_set()
 {
    let fixture = Fixture::start().await;
    fixture.connect_ada().await;
    fixture.ada_manages_requests().await;
    let ben = fixture.server.user_with_default_permissions("Ben").await;

    fixture.server.requests(&ben).ask(DUNE).await;

    assert_eq!(
        fixture.chat.wait_for_handed(2).await,
        sorted(vec![
            (Recipient::Channel, Told::New, None),
            (to_ada(), Told::New, None),
        ])
    );
}

#[tokio::test]
async fn a_requester_who_did_not_connect_is_skipped_and_the_channel_is_still_told() {
    let fixture = Fixture::start().await;
    fixture.connect_ada().await;
    let ben = fixture.server.user_with_default_permissions("Ben").await;
    let dune = fixture.server.requests(&ben).ask(DUNE).await;
    fixture.ada_asks_for(DUNE).await;

    fixture.server.approve(dune.id).await;

    let approved = Told::Now(wire::RequestState::REQUEST_STATE_APPROVED);
    assert_eq!(
        fixture.chat.wait_for_handed(4).await,
        sorted(vec![
            (Recipient::Channel, Told::New, None),
            (Recipient::Channel, Told::Joined, None),
            (Recipient::Channel, approved.clone(), None),
            (to_ada(), approved, None),
        ])
    );
}

#[tokio::test]
async fn a_cancelled_request_comes_without_a_link_since_it_is_gone_from_the_web_app() {
    let fixture = Fixture::start().await;
    fixture.set_server_url().await;
    let dune = fixture.ada_asks_for(DUNE).await;

    fixture
        .server
        .admin_requests()
        .cancel_request(CancelRequestRequest {
            id: dune.id,
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(
        fixture.chat.wait_for_handed(2).await,
        sorted(vec![
            (Recipient::Channel, Told::New, Some(link(dune.id))),
            (Recipient::Channel, Told::Cancelled, None),
        ])
    );
}

#[tokio::test]
async fn a_user_whose_chat_blocked_the_bot_is_disconnected_and_told_no_more() {
    let fixture = Fixture::start().await;
    fixture.connect_ada().await;
    fixture.chat.service().blocked.insert(ADA_CHAT.to_owned());
    let alien = fixture.ada_asks_for(ALIEN).await;

    fixture.server.approve(alien.id).await;
    fixture
        .wait_for_ada(PluginConnectionState::PLUGIN_CONNECTION_STATE_NOT_CONNECTED)
        .await;
    let _media = fixture.library_with_alien().await;
    fixture.chat.wait_for_handed(4).await;
    // Time for Ada to be handed Alien becoming Available, which would make a fifth.
    tokio::time::sleep(Duration::from_millis(500)).await;

    let approved = Told::Now(wire::RequestState::REQUEST_STATE_APPROVED);
    let available = Told::Now(wire::RequestState::REQUEST_STATE_AVAILABLE);
    assert_eq!(
        sorted(fixture.chat.service().handed.clone()),
        sorted(vec![
            (Recipient::Channel, Told::New, None),
            (Recipient::Channel, approved.clone(), None),
            (Recipient::Channel, available, None),
            (to_ada(), approved, None),
        ])
    );
    assert_eq!(fixture.adas_stored_connection().await, None);
}

#[tokio::test]
async fn a_delivery_failing_for_another_reason_leaves_the_user_connected() {
    let log = Log::global();
    let failed = [
        CHAT,
        "provider failed",
        "to User",
        "the chat service is down",
    ];
    let failed_before = log.count(&failed);
    let fixture = Fixture::start().await;
    fixture.connect_ada().await;
    let alien = fixture.ada_asks_for(ALIEN).await;
    fixture.chat.service().down = true;

    fixture.server.approve(alien.id).await;

    log.wait_for(&failed, failed_before + 1).await;
    assert_eq!(
        fixture.adas_stored_connection().await.as_deref(),
        Some(ADA_CHAT)
    );
}
