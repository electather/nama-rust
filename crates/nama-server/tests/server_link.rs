//! Linking the Server, and a User, through the enabled Plugin that links it (ADR 0011, ADR 0094),
//! here an in-process one scripted by each test, and a Server without one, which has nothing to
//! Link. The Plugin is told the Server's Server Addresses, and shows whether the service reached it
//! from outside.

#[path = "support/log.rs"]
mod log;
#[path = "support/refused.rs"]
mod refused;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use std::{
    future,
    num::NonZeroU16,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::Duration,
};

use connectrpc::{
    ConnectError, ErrorCode,
    client::{ClientConfig, HttpClient},
};
use log::Log;
use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    server_link::{Link, Reachability, ServerAddresses, ServerLinker},
};
use nama_proto::nama::v1::{
    self as proto, DeleteUserRequest, GetServerLinkRequest, GetUserLinkRequest, ListUsersRequest,
    Permission, RefusalReason, ServerServiceClient, ServerSettings, StartServerLinkRequest,
    StartUserLinkRequest, UnlinkServerRequest, UnlinkUserRequest, UpdateServerSettingsRequest,
    UserServiceClient,
};
use nama_server::{config::Config, plugins::Registry};
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
    /// The Users it was asked to Unlink, by id, in order.
    unlinked_users: Vec<i64>,
    /// Whether Unlinking a User fails, as with the service unreachable.
    unlink_user_fails: bool,
    /// Whether Unlinking a User never answers, as with the service slow to time out.
    unlink_user_hangs: bool,
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

    fn unlink_user(&self, user_id: i64) -> BoxFuture<'_, Result<(), PluginError>> {
        let mut script = self.script();
        script.unlinked_users.push(user_id);
        if script.unlink_user_hangs {
            return Box::pin(future::pending());
        }
        if script.unlink_user_fails {
            return Box::pin(future::ready(Err("the service is unreachable".into())));
        }
        script.user_link = Some(Link::NotLinked);
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

/// Waits until `linker` was asked to Unlink exactly `expected`, since a delete doesn't wait for it.
async fn until_unlinked(linker: &Scripted, expected: &[i64]) {
    let waited = tokio::time::timeout(server::WAIT, async {
        while linker.script().unlinked_users != expected {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    assert!(
        waited.is_ok(),
        "Unlinked {:?}, not {expected:?}",
        linker.script().unlinked_users
    );
}

/// Deletes the User `user_id`, calling as `who`.
async fn delete(who: ClientConfig, user_id: i64) -> Result<(), ConnectError> {
    users(who)
        .delete_user(DeleteUserRequest {
            user_id,
            ..Default::default()
        })
        .await
        .map(drop)
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
        reachability: None,
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
        reachability: Some(Reachability {
            address: "http://203.0.113.7:7980".to_owned(),
            reachable: true,
        }),
    });
    let shown = link(&server).await.unwrap();
    assert_eq!(shown.account, "ada@example.com");
    assert_eq!(shown.code, "");
    assert_eq!(shown.public_address, "http://203.0.113.7:7980");
    assert!(shown.reachable);

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

    // The test Server listens on 127.0.0.1, which no other device on its LAN can reach, so it
    // opens no port on the router either.
    told(
        &linker,
        &ServerAddresses {
            url: "https://nama.example.com/".to_owned(),
            lan: vec![],
            public_port: None,
        },
    )
    .await;
}

/// Waits until `linker` was told `addresses`.
async fn told(linker: &Scripted, addresses: &ServerAddresses) {
    let waited = tokio::time::timeout(server::WAIT, async {
        while linker.script().addresses.as_ref() != Some(addresses) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    assert!(waited.is_ok(), "told {:?}", linker.script().addresses);
}

#[tokio::test]
async fn the_plugin_is_told_the_port_the_admin_forwarded() {
    let linker = Scripted::default();

    let _server = TestServer::builder()
        .config(Config {
            forwarded_port: NonZeroU16::new(7443),
            ..Config::default()
        })
        .plugins(enabling(&linker))
        .start()
        .await;

    told(
        &linker,
        &ServerAddresses {
            public_port: Some(7443),
            ..ServerAddresses::default()
        },
    )
    .await;
}

#[tokio::test]
async fn deleting_a_user_unlinks_them_once() {
    let linker = Scripted::default();
    linker.script().user_link = Some(Link::Linked {
        account: "grace@example.com".to_owned(),
        reachability: None,
    });
    let server = with_linker(&linker).await;
    let grace = server.user_with_default_permissions("Grace").await;

    delete(server.client_config(), grace.id).await.unwrap();

    until_unlinked(&linker, &[grace.id]).await;
}

#[tokio::test]
async fn deleting_a_user_doesnt_wait_for_the_plugin_to_unlink_them() {
    let linker = Scripted::default();
    linker.script().unlink_user_hangs = true;
    let server = with_linker(&linker).await;
    let grace = server.user_with_default_permissions("Grace").await;

    let deleted =
        tokio::time::timeout(server::WAIT, delete(server.client_config(), grace.id)).await;

    deleted.expect("the delete waited for the Plugin").unwrap();
    until_unlinked(&linker, &[grace.id]).await;
}

#[tokio::test]
async fn an_admin_deleting_themselves_is_unlinked() {
    let linker = Scripted::default();
    let server = with_linker(&linker).await;
    let ada = server.user("Ada", &[Permission::Admin]).await;

    delete(ada.client_config(), ada.id).await.unwrap();

    until_unlinked(&linker, &[ada.id]).await;
}

#[tokio::test]
async fn an_unlinked_user_is_deleted() {
    let linker = Scripted::default();
    linker.script().user_link = Some(Link::NotLinked);
    let server = with_linker(&linker).await;
    let grace = server.user_with_default_permissions("Grace").await;

    delete(server.client_config(), grace.id).await.unwrap();
}

#[tokio::test]
async fn a_refused_delete_unlinks_no_one() {
    let linker = Scripted::default();
    let server = with_linker(&linker).await;

    delete(server.client_config(), 9999).await.unwrap_err();

    assert_eq!(linker.script().unlinked_users, Vec::<i64>::new());
}

#[tokio::test]
async fn a_user_is_deleted_without_a_plugin_that_links_the_server() {
    let server = TestServer::builder().start().await;
    let grace = server.user_with_default_permissions("Grace").await;

    delete(server.client_config(), grace.id).await.unwrap();
}

#[tokio::test]
async fn a_plugin_that_cant_unlink_a_deleted_user_leaves_them_deleted_and_warns() {
    let log = Log::global();
    let linker = Scripted::default();
    linker.script().unlink_user_fails = true;
    let server = with_linker(&linker).await;
    let grace = server.user_with_default_permissions("Grace").await;

    delete(server.client_config(), grace.id).await.unwrap();

    let listed = users(server.client_config())
        .list_users(ListUsersRequest::default())
        .await
        .unwrap()
        .into_owned()
        .users;
    assert!(listed.iter().all(|user| user.id != grace.id));
    log.wait_for(
        &["could not tell the service a User was deleted", "linker"],
        1,
    )
    .await;
}
