//! What Plugins hear of the Server and its Users being Linked and Unlinked (ADR 0105): each change
//! is an Event Observers are told, and a notification each Notifier is handed for its channel.

#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/fake_hub.rs"]
mod fake_hub;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use std::{
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::Duration,
};

use connectrpc::client::HttpClient;
use fake_hub::FakeHub;
use nama_plugin_api::{
    BoxFuture, Manifest, Observer, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    notifier::{Notification, Notifier, Recipient},
    points::{self, ObservedEvents},
};
use nama_plugin_proto::nama::plugin::v1::{self as wire, event::Event};
use nama_proto::nama::v1::{
    GetUserLinkRequest, ServerServiceClient, UnlinkServerRequest, UnlinkUserRequest,
    UpdateServerSettingsRequest, UserServiceClient,
};
use nama_server::plugins::Registry;
use server::{TestServer, WAIT};

/// What a Plugin heard: an Event it observed, or a notification handed to it for a recipient.
#[derive(Debug, Clone, PartialEq)]
enum Heard {
    Observed(Event),
    Handed(Recipient, Event),
}

/// An in-process Plugin that keeps every Link Event it observes and every notification its
/// Notifier is handed.
#[derive(Clone, Default)]
struct Listener(Arc<Mutex<Vec<Heard>>>);

impl Listener {
    fn heard(&self) -> MutexGuard<'_, Vec<Heard>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Waits until it has heard `expected` and nothing else, in either order, as Observers and
    /// Notifiers are told apart, then forgets it.
    async fn until_heard(&self, expected: &[Heard]) {
        let started = tokio::time::Instant::now();
        loop {
            let heard = self.heard().clone();
            if heard.len() >= expected.len() {
                assert!(
                    expected.iter().all(|one| heard.contains(one)) && heard.len() == expected.len(),
                    "heard {heard:?}, not {expected:?}"
                );
                self.heard().clear();
                return;
            }
            assert!(started.elapsed() < WAIT, "heard only {heard:?}");
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
}

impl Plugin for Listener {
    fn manifest(&self) -> Manifest {
        Manifest {
            version: "1.0.0".to_owned(),
            extension_points: vec![
                PointKey::observe::<points::Events>(),
                PointKey::provide::<dyn Notifier>(),
            ],
            ..Manifest::new(PluginId::from_static("listener"))
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        registrar.observe::<points::Events>(self.clone());
        registrar.provide::<dyn Notifier>(Priority::DEFAULT, Arc::new(self.clone()));
        Ok(())
    }
}

impl Observer<points::Events> for Listener {
    fn observe<'a>(&'a self, event: &'a ObservedEvents) -> BoxFuture<'a, Result<(), PluginError>> {
        let links = (event.events.iter()).filter_map(|one| one.event.clone().filter(is_link));
        self.heard().extend(links.map(Heard::Observed));
        Box::pin(async { Ok(()) })
    }
}

impl Notifier for Listener {
    fn deliver<'a>(
        &'a self,
        notification: &'a Notification,
        recipient: &'a Recipient,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        if let Some(event) = notification.event.event.clone().filter(is_link) {
            assert!(notification.request.as_option().is_none());
            self.heard().push(Heard::Handed(recipient.clone(), event));
        }
        Box::pin(async { Ok(()) })
    }
}

fn is_link(event: &Event) -> bool {
    matches!(
        event,
        Event::ServerLinked(_)
            | Event::ServerUnlinked(_)
            | Event::UserLinked(_)
            | Event::UserUnlinked(_)
    )
}

/// `event` both observed and handed for the channel.
fn heard(event: impl Into<Event>) -> [Heard; 2] {
    let event = event.into();
    [
        Heard::Observed(event.clone()),
        Heard::Handed(Recipient::Channel, event),
    ]
}

/// A Server whose config names a fake Hub, with the listening Plugin enabled.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn listened_to() -> (TestServer, FakeHub, Listener) {
    let hub = FakeHub::start().await;
    let listener = Listener::default();
    let plugins = Registry::new();
    plugins.enable(Arc::new(listener.clone())).unwrap();
    let server = TestServer::builder()
        .config(hub.config())
        .plugins(plugins)
        .start()
        .await;
    (server, hub, listener)
}

fn settings(server: &TestServer) -> ServerServiceClient<HttpClient> {
    ServerServiceClient::new(HttpClient::plaintext(), server.client_config())
}

#[tokio::test]
async fn linking_and_unlinking_the_server_and_a_user_are_each_told_once() {
    let (server, hub, listener) = listened_to().await;

    hub.link(&server).await;
    listener
        .until_heard(&heard(wire::ServerLinked::default()))
        .await;

    let grace = server.user_with_default_permissions("Grace").await;
    let users = UserServiceClient::new(HttpClient::plaintext(), grace.client_config());
    hub.link_user(grace.id);
    users
        .get_user_link(GetUserLinkRequest::default())
        .await
        .unwrap();
    let grace_linked = wire::UserLinked {
        user_id: grace.id,
        name: "Grace".to_owned(),
        ..Default::default()
    };
    listener.until_heard(&heard(grace_linked)).await;

    users
        .unlink_user(UnlinkUserRequest::default())
        .await
        .unwrap();
    let grace_unlinked = wire::UserUnlinked {
        user_id: grace.id,
        name: "Grace".to_owned(),
        ..Default::default()
    };
    listener.until_heard(&heard(grace_unlinked)).await;

    settings(&server)
        .unlink_server(UnlinkServerRequest::default())
        .await
        .unwrap();
    listener
        .until_heard(&heard(wire::ServerUnlinked::default()))
        .await;
}

#[tokio::test]
async fn a_server_the_hub_forgot_is_told_unlinked() {
    let (server, hub, listener) = listened_to().await;
    hub.link(&server).await;
    listener
        .until_heard(&heard(wire::ServerLinked::default()))
        .await;
    hub.forget_server();

    // Reporting the Server Addresses is how the Server learns the Hub forgot it.
    settings(&server)
        .update_server_settings(UpdateServerSettingsRequest {
            settings: nama_proto::nama::v1::ServerSettings {
                url: "https://nama.example.com".to_owned(),
                ..Default::default()
            }
            .into(),
            ..Default::default()
        })
        .await
        .unwrap();

    listener
        .until_heard(&heard(wire::ServerUnlinked::default()))
        .await;
}
