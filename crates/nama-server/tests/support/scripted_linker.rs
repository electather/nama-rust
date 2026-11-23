//! A Plugin that links the Server, scripted by each test, and the calls tests make around it.

use std::{
    future,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::Duration,
};

use connectrpc::{
    ConnectError,
    client::{ClientConfig, HttpClient},
};
use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    points::WatchStateChange,
    server_link::{AccountSignIn, Link, ServerAddresses, ServerLinker, WatchlistEntry},
};
use nama_proto::nama::v1::{DeleteUserRequest, UserServiceClient};
use nama_server::plugins::Registry;

use crate::server::{self, TestServer};

/// A Plugin that links the Server, answering what its test scripts.
#[derive(Clone, Default)]
pub(crate) struct Scripted(Arc<Mutex<Script>>);

#[derive(Default)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each is a switch a test flips on its own, not states of one thing"
)]
pub(crate) struct Script {
    /// What it answers `link`; `None` fails, as with the service unreachable.
    pub(crate) link: Option<Link>,
    /// Whether asking for the Server's Link, or a User's, never answers, as with the service slow
    /// to time out.
    pub(crate) links_hang: bool,
    /// The name it was last asked to start Linking as.
    pub(crate) started_as: Option<String>,
    /// What it answers `user_link` for every User; `None` while the Server isn't Linked.
    pub(crate) user_link: Option<Link>,
    /// The User it was last asked to start Linking, by id and name.
    pub(crate) user_started: Option<(i64, String)>,
    /// The User it was last asked to invite, by id, name and email.
    pub(crate) invited: Option<(i64, String, String)>,
    /// Where the sign-in with an Account it last started returns to.
    pub(crate) returns_to: Option<String>,
    /// The User an Account approved the sign-in as, until it is finished.
    pub(crate) approved: Option<i64>,
    /// How often it was asked to finish a sign-in with an Account.
    pub(crate) finishes: usize,
    /// The Server Addresses it was last told.
    pub(crate) addresses: Option<ServerAddresses>,
    /// The Users it was asked to Unlink, by id, in order.
    pub(crate) unlinked_users: Vec<i64>,
    /// Whether Unlinking a User fails, as with the service unreachable.
    pub(crate) unlink_user_fails: bool,
    /// Whether Unlinking a User never answers, as with the service slow to time out.
    pub(crate) unlink_user_hangs: bool,
}

impl Scripted {
    pub(crate) fn script(&self) -> MutexGuard<'_, Script> {
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
        if self.script().links_hang {
            return Box::pin(future::pending());
        }
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
        let script = self.script();
        if script.links_hang {
            return Box::pin(future::pending());
        }
        Box::pin(future::ready(Ok(script.user_link.clone())))
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

    fn invite_user<'a>(
        &'a self,
        user_id: i64,
        user_name: &'a str,
        email: &'a str,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        let invited = self.answer().map(|_| {
            self.script().invited = Some((user_id, user_name.to_owned(), email.to_owned()));
        });
        Box::pin(future::ready(invited))
    }

    fn start_account_sign_in<'a>(
        &'a self,
        return_url: &'a str,
    ) -> BoxFuture<'a, Result<AccountSignIn, PluginError>> {
        let started = self.answer().map(|_| {
            self.script().returns_to = Some(return_url.to_owned());
            AccountSignIn {
                url: "https://hub.example/server-sign-in/1".to_owned(),
                handle: "handle".to_owned(),
            }
        });
        Box::pin(future::ready(started))
    }

    fn finish_account_sign_in<'a>(
        &'a self,
        handle: &'a str,
        code: &'a str,
    ) -> BoxFuture<'a, Result<Option<i64>, PluginError>> {
        self.script().finishes += 1;
        let finished = self.answer().map(|_| {
            let approved = self.script().approved.take();
            approved.filter(|_| handle == "handle" && code == "code")
        });
        Box::pin(future::ready(finished))
    }

    fn set_addresses(&self, addresses: ServerAddresses) -> BoxFuture<'_, Result<(), PluginError>> {
        self.script().addresses = Some(addresses);
        Box::pin(future::ready(Ok(())))
    }

    /// Pulling is `tests/sync.rs`'s.
    fn watch_states(
        &self,
        _user_id: i64,
    ) -> BoxFuture<'_, Result<Vec<WatchStateChange>, PluginError>> {
        Box::pin(future::ready(Ok(Vec::new())))
    }

    /// Syncing is `tests/sync.rs`'s.
    fn sync_watchlist(
        &self,
        _user_id: i64,
        _watchlist: Vec<WatchlistEntry>,
    ) -> BoxFuture<'_, Result<Vec<WatchlistEntry>, PluginError>> {
        Box::pin(future::ready(Ok(Vec::new())))
    }
}

pub(crate) async fn with_linker(linker: &Scripted) -> TestServer {
    TestServer::builder()
        .plugins(enabling(linker))
        .start()
        .await
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
pub(crate) fn enabling(linker: &Scripted) -> Registry {
    let plugins = Registry::new();
    plugins.enable(Arc::new(linker.clone())).unwrap();
    plugins
}

pub(crate) fn users(config: ClientConfig) -> UserServiceClient<HttpClient> {
    UserServiceClient::new(HttpClient::plaintext(), config)
}

/// Waits until `linker` was asked to Unlink exactly `expected`, since a delete doesn't wait for it.
pub(crate) async fn until_unlinked(linker: &Scripted, expected: &[i64]) {
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
pub(crate) async fn delete(who: ClientConfig, user_id: i64) -> Result<(), ConnectError> {
    users(who)
        .delete_user(DeleteUserRequest {
            user_id,
            ..Default::default()
        })
        .await
        .map(drop)
}
