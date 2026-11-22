//! Linking the Server through the enabled Plugin that links it (ADR 0011, ADR 0094), here an
//! in-process one scripted by each test, and a Server without one, which has nothing to Link.

#[path = "support/refused.rs"]
mod refused;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use std::{
    future,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use connectrpc::{ErrorCode, client::HttpClient};
use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    server_link::{ServerLink, ServerLinker},
};
use nama_proto::nama::v1::{
    self as proto, GetServerLinkRequest, RefusalReason, ServerServiceClient,
    StartServerLinkRequest, UnlinkServerRequest,
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
    link: Option<ServerLink>,
    /// The name it was last asked to start Linking as.
    started_as: Option<String>,
}

impl Scripted {
    fn script(&self) -> MutexGuard<'_, Script> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn answer(&self) -> Result<ServerLink, PluginError> {
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
    fn link(&self) -> BoxFuture<'_, Result<ServerLink, PluginError>> {
        Box::pin(future::ready(self.answer()))
    }

    fn start_link<'a>(
        &'a self,
        server_name: &'a str,
    ) -> BoxFuture<'a, Result<ServerLink, PluginError>> {
        self.script().started_as = Some(server_name.to_owned());
        let started = self.answer().map(|_| ServerLink::Waiting {
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
            self.script().link = Some(ServerLink::NotLinked);
        }
        Box::pin(future::ready(unlinked))
    }
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn with_linker(linker: &Scripted) -> TestServer {
    let plugins = Registry::new();
    plugins.enable(Arc::new(linker.clone())).unwrap();
    TestServer::builder().plugins(plugins).start().await
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
}

#[tokio::test]
async fn the_server_shows_the_code_its_plugin_starts_and_then_whose_it_is() {
    let linker = Scripted::default();
    linker.script().link = Some(ServerLink::NotLinked);
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
    linker.script().link = Some(ServerLink::Linked {
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
