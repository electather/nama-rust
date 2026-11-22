//! Linking the Server, and a User, through the enabled Plugin that links it (ADR 0011, ADR 0094),
//! here an in-process one scripted by each test, and a Server without one, which has nothing to
//! Link. The Plugin is told the Server's Server Addresses.

#[path = "support/refused.rs"]
mod refused;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use std::{
    future,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::Duration,
};

use connectrpc::{
    ErrorCode,
    client::{ClientConfig, HttpClient},
};
use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    server_link::{Link, ServerAddresses, ServerLinker},
};
use nama_proto::nama::v1::{
    self as proto, GetServerLinkRequest, GetUserLinkRequest, RefusalReason, ServerServiceClient,
    ServerSettings, StartServerLinkRequest, StartUserLinkRequest, UnlinkServerRequest,
    UnlinkUserRequest, UpdateServerSettingsRequest, UserServiceClient,
};
use nama_server::plugins::Registry;
use refused::assert_refused;
use server::TestServer;

/// A Plugin that links the Server, answering what its test scripts.
#[derive(Clone, Default)]
struct Scripted(Arc<Mutex<Script>>);

#[derive(Default)]
struct Script {
    /// What it answers `link`; `None` fails, as with the service unreachable.
    link: Option<Link>,
    /// The name it was last asked to start Linking as.
    started_as: Option<String>,
    /// What it answers `user_link` for every User; `None` while the Server isn't Linked.
    user_link: Option<Link>,
    /// The User it was last asked to start Linking, by id and name.
    user_started: Option<(i64, String)>,
    /// The Server Addresses it was last told.
    addresses: Option<ServerAddresses>,
}

impl Scripted {
    fn script(&self) -> MutexGuard<'_, Script> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn answer(&self) -> Result<Link, PluginError> {
        self.script()
            .link
            .clone()
            .ok_or_else(|| "the service is unreachable".into())
    }
}

impl Plugin for Scripted {
    fn manifest(&self) -> Manifest {
        Manifest {
            extension_points: vec![PointKey::provide::<dyn ServerLinker>()],
            ..Manifest::new(PluginId::from_static("linker"))
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        registrar.provide::<dyn ServerLinker>(Priority::DEFAULT, Arc::new(self.clone()));
        Ok(())
    }
}

impl ServerLinker for Scripted {
    fn link(&self) -> BoxFuture<'_, Result<Link, PluginError>> {
        Box::pin(future::ready(self.answer()))
    }

    fn start_link<'a>(&'a self, server_name: &'a str) -> BoxFuture<'a, Result<Link, PluginError>> {
        self.script().started_as = Some(server_name.to_owned());
        let started = self.answer().map(|_| Link::Waiting {
            code: "BDFH-JKLM".to_owned(),
            url: "https://hub.example".to_owned(),
        });
        if let Ok(waiting) = &started {
            self.script().link = Some(waiting.clone());
        }
        Box::pin(future::ready(started))
    }

    fn unlink(&self) -> BoxFuture<'_, Result<(), PluginError>> {
        let unlinked = self.answer().map(|_| ());
        if unlinked.is_ok() {
            self.script().link = Some(Link::NotLinked);
        }
        Box::pin(future::ready(unlinked))
    }

    fn user_link(&self, _user_id: i64) -> BoxFuture<'_, Result<Option<Link>, PluginError>> {
        Box::pin(future::ready(Ok(self.script().user_link.clone())))
    }

    fn start_user_link<'a>(
        &'a self,
        user_id: i64,
        user_name: &'a str,
    ) -> BoxFuture<'a, Result<Link, PluginError>> {
        let waiting = Link::Waiting {
            code: "CDFG-HJKL".to_owned(),
            url: "https://hub.example".to_owned(),
        };
        let mut script = self.script();
        script.user_started = Some((user_id, user_name.to_owned()));
        script.user_link = Some(waiting.clone());
        Box::pin(future::ready(Ok(waiting)))
    }

    fn unlink_user(&self, _user_id: i64) -> BoxFuture<'_, Result<(), PluginError>> {
        self.script().user_link = Some(Link::NotLinked);
        Box::pin(future::ready(Ok(())))
    }

    fn set_addresses(&self, addresses: ServerAddresses) -> BoxFuture<'_, Result<(), PluginError>> {
        self.script().addresses = Some(addresses);
        Box::pin(future::ready(Ok(())))
    }
}

async fn with_linker(linker: &Scripted) -> TestServer {
    TestServer::builder()
        .plugins(enabling(linker))
        .start()
        .await
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn enabling(linker: &Scripted) -> Registry {
    let plugins = Registry::new();
    plugins.enable(Arc::new(linker.clone())).unwrap();
    plugins
}

fn settings(server: &TestServer) -> ServerServiceClient<HttpClient> {
    ServerServiceClient::new(HttpClient::plaintext(), server.client_config())
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn link(server: &TestServer) -> Option<proto::ServerLink> {
    let got = settings(server)
        .get_server_link(GetServerLinkRequest::default())
        .await
        .unwrap();
    got.into_owned().link.into_option()
}

fn users(config: ClientConfig) -> UserServiceClient<HttpClient> {
    UserServiceClient::new(HttpClient::plaintext(), config)
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn user_link(config: ClientConfig) -> Option<proto::UserLink> {
    let got = users(config)
        .get_user_link(GetUserLinkRequest::default())
        .await
        .unwrap();
    got.into_owned().link.into_option()
}

#[tokio::test]
async fn a_server_without_a_plugin_that_links_it_has_nothing_to_link() {
    let server = TestServer::builder().start().await;

    assert_eq!(link(&server).await, None);
    assert_refused(
        settings(&server)
            .start_server_link(StartServerLinkRequest::default())
            .await,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_NO_SERVER_LINKER,
    );
    assert_refused(
        settings(&server)
            .unlink_server(UnlinkServerRequest::default())
            .await,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_NO_SERVER_LINKER,
    );
    assert_eq!(user_link(server.client_config()).await, None);
    assert_refused(
        users(server.client_config())
            .start_user_link(StartUserLinkRequest::default())
            .await,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_NO_SERVER_LINKER,
    );
    assert_refused(
        users(server.client_config())
            .unlink_user(UnlinkUserRequest::default())
            .await,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_NO_SERVER_LINKER,
    );
}

#[tokio::test]
async fn a_users_link_is_unset_until_the_server_is_linked() {
    let linker = Scripted::default();
    let server = with_linker(&linker).await;

    assert_eq!(user_link(server.client_config()).await, None);
}

#[tokio::test]
async fn a_user_shows_the_code_the_plugin_starts_for_them_and_then_whose_they_are() {
    let linker = Scripted::default();
    linker.script().user_link = Some(Link::NotLinked);
    let server = with_linker(&linker).await;
    let grace = server.user_with_default_permissions("Grace").await;
    assert_eq!(
        user_link(grace.client_config()).await,
        Some(proto::UserLink::default())
    );

    let started = users(grace.client_config())
        .start_user_link(StartUserLinkRequest::default())
        .await
        .unwrap()
        .into_owned()
        .link
        .into_option();

    let waiting = proto::UserLink {
        code: "CDFG-HJKL".to_owned(),
        url: "https://hub.example".to_owned(),
        ..Default::default()
    };
    assert_eq!(started, Some(waiting.clone()));
    assert_eq!(
        linker.script().user_started,
        Some((grace.id, "Grace".to_owned()))
    );
    assert_eq!(user_link(grace.client_config()).await, Some(waiting));
    linker.script().user_link = Some(Link::Linked {
        account: "grace@example.com".to_owned(),
    });
    let shown = user_link(grace.client_config()).await.unwrap();
    assert_eq!(shown.account, "grace@example.com");

    users(grace.client_config())
        .unlink_user(UnlinkUserRequest::default())
        .await
        .unwrap();

    assert_eq!(
        user_link(grace.client_config()).await,
        Some(proto::UserLink::default())
    );
}

#[tokio::test]
async fn the_server_shows_the_code_its_plugin_starts_and_then_whose_it_is() {
    let linker = Scripted::default();
    linker.script().link = Some(Link::NotLinked);
    let server = with_linker(&linker).await;
    assert_eq!(link(&server).await, Some(proto::ServerLink::default()));

    let started = settings(&server)
        .start_server_link(StartServerLinkRequest::default())
        .await
        .unwrap()
        .into_owned()
        .link
        .into_option();

    let waiting = proto::ServerLink {
        code: "BDFH-JKLM".to_owned(),
        url: "https://hub.example".to_owned(),
        ..Default::default()
    };
    assert_eq!(started, Some(waiting.clone()));
    assert_eq!(linker.script().started_as.as_deref(), Some("Nama"));
    assert_eq!(link(&server).await, Some(waiting));
    linker.script().link = Some(Link::Linked {
        account: "ada@example.com".to_owned(),
    });
    let shown = link(&server).await.unwrap();
    assert_eq!(shown.account, "ada@example.com");
    assert_eq!(shown.code, "");

    settings(&server)
        .unlink_server(UnlinkServerRequest::default())
        .await
        .unwrap();

    assert_eq!(link(&server).await, Some(proto::ServerLink::default()));
}

#[tokio::test]
async fn a_plugin_that_cant_reach_its_service_leaves_the_server_as_it_was() {
    let linker = Scripted::default();
    let server = with_linker(&linker).await;

    let started = settings(&server)
        .start_server_link(StartServerLinkRequest::default())
        .await;
    let unlinked = settings(&server)
        .unlink_server(UnlinkServerRequest::default())
        .await;

    assert_eq!(started.unwrap_err().code, ErrorCode::Unavailable);
    assert_eq!(unlinked.unwrap_err().code, ErrorCode::Unavailable);
}

#[tokio::test]
async fn the_plugin_is_told_the_server_url_the_admin_set() {
    let linker = Scripted::default();
    let server = with_linker(&linker).await;
    settings(&server)
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

    let _restarted = TestServer::builder()
        .plugins(enabling(&linker))
        .restart(server)
        .await;

    // The test Server listens on 127.0.0.1, which no other device on its LAN can reach.
    let told = ServerAddresses {
        url: "https://nama.example.com".to_owned(),
        lan: vec![],
    };
    let waited = tokio::time::timeout(server::WAIT, async {
        while linker.script().addresses.as_ref() != Some(&told) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    assert!(waited.is_ok(), "told {:?}", linker.script().addresses);
}
