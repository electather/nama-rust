//! A User's Watch State on their other Servers coming back through the enabled Plugin that links
//! the Server (ADR 0008), here a stand-in for the Hub that two Servers share.

#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/browsing.rs"]
mod browsing;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use std::{
    future,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::Duration,
};

use browsing::{ALIEN_1080P, Fixture};
use connectrpc::client::{ClientConfig, HttpClient};
use nama_plugin_api::{
    BoxFuture, Manifest, Observer, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    points::{WatchStateChange, WatchStateChanges, WatchStatesChanged},
    server_link::{AccountSignIn, Link, ServerAddresses, ServerLinker},
};
use nama_proto::nama::v1::{
    BrowseServiceClient, LibraryKind, ListContinueWatchingRequest, ReportProgressRequest,
    WatchStateServiceClient, continue_watching_entry::MovieOrEpisode,
};
use nama_server::plugins::Registry;
use server::{TestServer, WAIT, tmdb::ALIEN};

/// A stand-in for the Hub, shared by every Server it's enabled on: it keeps each Watch State change
/// a Server tells of, and hands them all back to every User's pull, as one Account's.
#[derive(Clone, Default)]
struct Hub(Arc<Mutex<Vec<WatchStateChange>>>);

impl Hub {
    fn kept(&self) -> MutexGuard<'_, Vec<WatchStateChange>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// A Server the Hub is enabled on, as `builder` starts it.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    fn on(&self, builder: server::Builder) -> server::Builder {
        let plugins = Registry::new();
        plugins.enable(Arc::new(self.clone())).unwrap();
        builder.plugins(plugins)
    }
}

impl Plugin for Hub {
    fn manifest(&self) -> Manifest {
        Manifest {
            extension_points: vec![
                PointKey::provide::<dyn ServerLinker>(),
                PointKey::observe::<WatchStatesChanged>(),
            ],
            ..Manifest::new(PluginId::from_static("hub"))
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        registrar.provide::<dyn ServerLinker>(Priority::DEFAULT, Arc::new(self.clone()));
        registrar.observe::<WatchStatesChanged>(self.clone());
        Ok(())
    }
}

impl Observer<WatchStatesChanged> for Hub {
    fn observe<'a>(
        &'a self,
        event: &'a WatchStateChanges,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        self.kept().extend(event.changes.iter().cloned());
        Box::pin(future::ready(Ok(())))
    }
}

/// Only what pulling asks; the rest is `tests/server_link.rs`'s.
impl ServerLinker for Hub {
    fn link(&self) -> BoxFuture<'_, Result<Link, PluginError>> {
        Box::pin(future::ready(Ok(linked())))
    }

    fn start_link<'a>(&'a self, _: &'a str) -> BoxFuture<'a, Result<Link, PluginError>> {
        Box::pin(future::ready(Ok(linked())))
    }

    fn unlink(&self) -> BoxFuture<'_, Result<(), PluginError>> {
        Box::pin(future::ready(Ok(())))
    }

    fn user_link(&self, _: i64) -> BoxFuture<'_, Result<Option<Link>, PluginError>> {
        Box::pin(future::ready(Ok(Some(linked()))))
    }

    fn start_user_link<'a>(
        &'a self,
        _: i64,
        _: &'a str,
    ) -> BoxFuture<'a, Result<Link, PluginError>> {
        Box::pin(future::ready(Ok(linked())))
    }

    fn unlink_user(&self, _: i64) -> BoxFuture<'_, Result<(), PluginError>> {
        Box::pin(future::ready(Ok(())))
    }

    fn invite_user<'a>(
        &'a self,
        _: i64,
        _: &'a str,
        _: &'a str,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        Box::pin(future::ready(Ok(())))
    }

    fn start_account_sign_in<'a>(
        &'a self,
        _: &'a str,
    ) -> BoxFuture<'a, Result<AccountSignIn, PluginError>> {
        Box::pin(future::ready(Err("no sign-in with an Account here".into())))
    }

    fn finish_account_sign_in<'a>(
        &'a self,
        _: &'a str,
    ) -> BoxFuture<'a, Result<Option<i64>, PluginError>> {
        Box::pin(future::ready(Ok(None)))
    }

    fn set_addresses(&self, _: ServerAddresses) -> BoxFuture<'_, Result<(), PluginError>> {
        Box::pin(future::ready(Ok(())))
    }

    fn watch_states(&self, _: i64) -> BoxFuture<'_, Result<Vec<WatchStateChange>, PluginError>> {
        Box::pin(future::ready(Ok(self.kept().clone())))
    }
}

fn linked() -> Link {
    Link::Linked {
        account: "ada@example.com".to_owned(),
    }
}

/// Watches Alien a minute in on `fx`'s Server as its Admin, and waits until the Hub has it.
#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "a test helper fails its test by panicking"
)]
async fn watch_alien(fx: &Fixture, hub: &Hub) {
    let watch_states =
        WatchStateServiceClient::new(HttpClient::plaintext(), fx.server.client_config());
    let report = ReportProgressRequest {
        version_id: fx.version_ids(ALIEN).await[0],
        position_ms: 60_000,
        duration_ms: 7_200_000,
        ..Default::default()
    };
    watch_states.report_progress(report).await.unwrap();
    until("the Hub has the progress", async || !hub.kept().is_empty()).await;
}

/// The position of Alien in the Continue Watching of the User signed in as `who`, if it's there.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn alien_in_continue_watching(who: &ClientConfig) -> Option<u64> {
    let browse = BrowseServiceClient::new(HttpClient::plaintext(), who.clone());
    let listed = browse
        .list_continue_watching(ListContinueWatchingRequest::default())
        .await
        .unwrap()
        .into_owned();
    listed
        .entries
        .into_iter()
        .find_map(|entry| match entry.movie_or_episode {
            Some(MovieOrEpisode::Movie(movie)) => Some(movie.watch_state.position_ms),
            _ => None,
        })
}

/// Waits until `done`, as a pull happens in the background.
async fn until(what: &str, mut done: impl AsyncFnMut() -> bool) {
    let waited = tokio::time::timeout(WAIT, async {
        while !done().await {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;
    assert!(waited.is_ok(), "timed out waiting until {what}");
}

#[tokio::test]
async fn progress_watched_on_one_server_shows_once_signed_in_on_another() {
    let hub = Hub::default();
    let first = Fixture::alien_on(hub.on(TestServer::builder())).await;
    let second = Fixture::alien_on(hub.on(TestServer::builder())).await;

    watch_alien(&first, &hub).await;
    let ada = second.server.user_with_default_permissions("Ada").await;

    until("Continue Watching shows the progress", async || {
        alien_in_continue_watching(&ada.client_config()).await == Some(60_000)
    })
    .await;
}

#[tokio::test]
async fn progress_on_a_title_the_server_lacks_shows_once_a_scan_adds_it() {
    let hub = Hub::default();
    let first = Fixture::alien_on(hub.on(TestServer::builder())).await;
    let second = Fixture::start_on(
        LibraryKind::LIBRARY_KIND_MOVIES,
        hub.on(TestServer::builder()),
    )
    .await;
    watch_alien(&first, &hub).await;
    let ada = second.server.user_with_default_permissions("Ada").await;
    assert_eq!(alien_in_continue_watching(&ada.client_config()).await, None);

    second.write(ALIEN_1080P, b"video");
    second.scan_and_match().await;

    until("Continue Watching shows the progress", async || {
        alien_in_continue_watching(&ada.client_config()).await == Some(60_000)
    })
    .await;
}
