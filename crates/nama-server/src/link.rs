//! Every call the Server makes to the enabled Plugin that links it, such as the Hub Plugin (ADR
//! 0011, ADR 0094), answered in the Kernel's terms: a [`Link`], or an [`Error`] that tells "no
//! Plugin links the Server" from "it couldn't tell". It speaks only the Plugin contract. Each call's
//! bound is named once:
//!
//! | Calls                                                              | Bound         |
//! |--------------------------------------------------------------------|---------------|
//! | Reads a page needs: the Server's Link, a User's Link               | [`PAGE_READ`] |
//! | Changes at the service: starting or ending a Link, inviting a User, starting or finishing an Account sign-in, Unlinking a deleted User | none |
//! | Bulk transfers, and the Server Addresses tick                      | [`BULK`]      |
//!
//! A change isn't cut: cut mid-way, it could leave the service changed without the Server knowing,
//! such as an invite sent for a User the Server then deletes. The Plugin's own limit on each call
//! to its service bounds it.
//!
//! It remembers whose Link the last look found Linked, the Server's and each User's, and tells its
//! followers ([`Linking::follow`]) when a look finds one Linked that the last didn't; the first
//! look since the Server started counts.

use std::{
    collections::HashSet,
    future::Future,
    sync::{Arc, Mutex, PoisonError},
    time::Duration,
};

pub(crate) use nama_plugin_api::server_link::Link;
use nama_plugin_api::{
    PluginError, PluginId,
    points::{TrackPreference, WatchStateChange},
    server_link::{AccountSignIn, ServerAddresses, ServerLinker, WatchlistEntry},
};
use tokio::sync::{broadcast, oneshot};
use tracing::Instrument as _;

use crate::{diagnostics, plugins::ExtensionPoints};

/// How long a page waits on the Plugin for the Server's Link or a User's, so Server info and the
/// Users list answer even when it doesn't.
const PAGE_READ: Duration = Duration::from_secs(10);

/// How long a bulk transfer, or a call of the Server Addresses tick, may take, so a Plugin that
/// never answers can't hold up every later scan's pulls or minute's tick.
const BULK: Duration = Duration::from_mins(1);

/// How many new Links may be told before a follower has heard them.
const NEW_LINKS_BUFFERED: usize = 64;

/// Why the Plugin that links the Server gave no answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum Error {
    /// No enabled Plugin links the Server.
    #[error("no enabled Plugin links the Server")]
    NoLinkingPlugin,
    /// It couldn't tell: it failed, as with its service unreachable, or a page stopped waiting on
    /// it. Its page says why.
    #[error("the Plugin that links the Server failed; its page says why")]
    Failed,
}

/// Whose Link it is: the Server's, or the User's with this id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Whose {
    Server,
    User(i64),
}

/// Asks the enabled Plugin that links the Server. Cloning it gives another handle to the same.
#[derive(Debug, Clone)]
pub(crate) struct Linking {
    points: ExtensionPoints,
    /// Whose Link the last look found Linked.
    linked: Arc<Mutex<HashSet<Whose>>>,
    new_links: broadcast::Sender<Whose>,
}

impl Linking {
    /// Asks the Plugins of `points`, having seen no Link yet.
    pub(crate) fn new(points: ExtensionPoints) -> Self {
        Self {
            points,
            linked: Arc::default(),
            new_links: broadcast::channel(NEW_LINKS_BUFFERED).0,
        }
    }

    /// Whose Link turns Linked from now on, as a look finds it.
    pub(crate) fn follow(&self) -> broadcast::Receiver<Whose> {
        self.new_links.subscribe()
    }

    /// Whether an enabled Plugin links the Server; most Servers have none.
    pub(crate) fn has_linking_plugin(&self) -> bool {
        !self.points.providers::<dyn ServerLinker>().is_empty()
    }

    /// The Server's Link.
    pub(crate) async fn server_link(&self) -> Result<Link, Error> {
        Ok(self.look(Whose::Server).await?.unwrap_or(Link::NotLinked))
    }

    /// Whether the Server is Linked, so its Users can sign in with their Accounts. Not, when the
    /// Plugin couldn't tell.
    pub(crate) async fn is_server_linked(&self) -> bool {
        matches!(self.server_link().await, Ok(Link::Linked { .. }))
    }

    /// The User `user_id`'s Link, or `None` while they can't Link, before the Server is Linked.
    /// Asking is how the Plugin learns of an Unlink made on the service, such as by deleting the
    /// Account (ADR 0099).
    pub(crate) async fn user_link(&self, user_id: i64) -> Result<Option<Link>, Error> {
        self.look(Whose::User(user_id)).await
    }

    /// `whose` Link, waiting on it no longer than [`PAGE_READ`].
    async fn look(&self, whose: Whose) -> Result<Option<Link>, Error> {
        let linking = self.clone();
        let looking = async move {
            let link = match whose {
                Whose::Server => (linking.ask(async |linker| linker.link().await).await).map(Some),
                Whose::User(user_id) => {
                    (linking.ask(async |linker| linker.user_link(user_id).await)).await
                }
            };
            if let Ok(link) = &link {
                linking.saw(whose, link.as_ref());
            }
            link
        };
        // In a task of its own, so a look the page stopped waiting on still finishes: the Hub
        // Plugin keeps the Link a waiting code's poll finds, which cut mid-way it would lose.
        let looked = tokio::spawn(looking.in_current_span());
        match tokio::time::timeout(PAGE_READ, looked).await {
            Ok(Ok(link)) => link,
            Ok(Err(panicked)) => {
                tracing::error!("looking at a Link panicked: {panicked}");
                Err(Error::Failed)
            }
            Err(_elapsed) => {
                tracing::warn!("the Plugin that links the Server didn't say where a Link stands");
                Err(Error::Failed)
            }
        }
    }

    /// Remembers whether `whose` `link` is Linked, telling followers if it just turned Linked.
    fn saw(&self, whose: Whose, link: Option<&Link>) {
        let mut linked = self.linked.lock().unwrap_or_else(PoisonError::into_inner);
        let is_new = if matches!(link, Some(Link::Linked { .. })) {
            linked.insert(whose)
        } else {
            linked.remove(&whose);
            false
        };
        drop(linked);
        if is_new {
            // Fails only while nothing follows, which has nothing to push.
            let _: Result<usize, _> = self.new_links.send(whose);
        }
    }

    /// Starts Linking the Server, which the service lists as `server_name`, and answers the code.
    pub(crate) async fn start_server_link(&self, server_name: &str) -> Result<Link, Error> {
        let link = (self.ask(async |linker| linker.start_link(server_name).await)).await?;
        self.saw(Whose::Server, Some(&link));
        Ok(link)
    }

    /// Unlinks the Server, or forgets its waiting code.
    pub(crate) async fn unlink_server(&self) -> Result<(), Error> {
        self.ask(async |linker| linker.unlink().await).await?;
        self.saw(Whose::Server, None);
        Ok(())
    }

    /// Starts Linking the User `user_id`, whom the service lists as `user_name`, and answers their
    /// code, or their Link if they are Linked already.
    pub(crate) async fn start_user_link(
        &self,
        user_id: i64,
        user_name: &str,
    ) -> Result<Link, Error> {
        let link = self
            .ask(async |linker| linker.start_user_link(user_id, user_name).await)
            .await?;
        self.saw(Whose::User(user_id), Some(&link));
        Ok(link)
    }

    /// Unlinks the User `user_id`, or forgets the code they wait on.
    pub(crate) async fn unlink_user(&self, user_id: i64) -> Result<(), Error> {
        (self.ask(async |linker| linker.unlink_user(user_id).await)).await?;
        self.saw(Whose::User(user_id), None);
        Ok(())
    }

    /// Asks the Account with `email`, if there is one, to accept the User `user_id`, whom the
    /// service lists as `user_name`.
    pub(crate) async fn invite_user(
        &self,
        user_id: i64,
        user_name: &str,
        email: &str,
    ) -> Result<(), Error> {
        self.ask(async |linker| linker.invite_user(user_id, user_name, email).await)
            .await
    }

    /// Starts someone signing in with their Account, whom the service sends back to `return_url`.
    pub(crate) async fn start_account_sign_in(
        &self,
        return_url: &str,
    ) -> Result<AccountSignIn, Error> {
        self.ask(async |linker| linker.start_account_sign_in(return_url).await)
            .await
    }

    /// The id of the User whose Account approved the sign-in `handle` stands for, finished with
    /// the one-time `code` the service sent the person back with; `None` while it isn't approved,
    /// or once it expired.
    pub(crate) async fn finish_account_sign_in(
        &self,
        handle: &str,
        code: &str,
    ) -> Result<Option<i64>, Error> {
        self.ask(async |linker| linker.finish_account_sign_in(handle, code).await)
            .await
    }

    /// Unlinks the User `user_id`, just deleted, so the service forgets their Link or waiting code
    /// too, without waiting for it. A failure is logged, not answered: the User is gone either way,
    /// and the service being unreachable mustn't fail or hold up their delete (#1919). Must be
    /// called within a Tokio runtime.
    pub(crate) fn unlink_deleted_user(&self, user_id: i64) {
        // A User created later may get the id again.
        self.saw(Whose::User(user_id), None);
        let points = self.points.clone();
        let unlinking = async move {
            let unlinked = points
                .ask_first::<dyn ServerLinker, _, _, _, _>(async |linker| {
                    linker.unlink_user(user_id).await
                })
                .await;
            if let Some(answer) = unlinked
                && answer.result.is_err()
            {
                // Ids only: `ask_first` already puts the failure's message on the Plugin's page.
                // The User may never have Linked, so this says only that the service wasn't told.
                tracing::warn!(
                    plugin = %answer.plugin,
                    user = user_id,
                    "could not tell the service a User was deleted"
                );
            }
        };
        let span = diagnostics::following_current(
            tracing::info_span!(parent: None, "unlink_deleted_user"),
        );
        tokio::spawn(unlinking.instrument(span));
    }

    /// What `call` gets from the enabled Plugin that links the Server. Its failure, such as the
    /// service being unreachable, is logged.
    async fn ask<R, F>(&self, call: impl Fn(Arc<dyn ServerLinker>) -> F) -> Result<R, Error>
    where
        F: Future<Output = Result<R, PluginError>>,
    {
        let answer = (self.points)
            .ask_first::<dyn ServerLinker, _, _, _, _>(call)
            .await
            .ok_or(Error::NoLinkingPlugin)?;
        answer.result.map_err(|failure| {
            tracing::warn!(plugin = %answer.plugin, "Linking failed: {}", failure.message);
            Error::Failed
        })
    }

    /// The Watch State the service keeps of the User `user_id` from every Server they use.
    pub(crate) async fn watch_states(&self, user_id: i64) -> Option<Vec<WatchStateChange>> {
        let pull = async |linker: Arc<dyn ServerLinker>| linker.watch_states(user_id).await;
        self.transfer(user_id, "pull a User's Watch State", pull)
            .await
    }

    /// The Subtitle and Audio Preferences the service keeps of the User `user_id`.
    pub(crate) async fn track_preferences(&self, user_id: i64) -> Option<Vec<TrackPreference>> {
        let pull = async |linker: Arc<dyn ServerLinker>| linker.track_preferences(user_id).await;
        self.transfer(user_id, "pull a User's Track Preferences", pull)
            .await
    }

    /// Hands the service the User `user_id`'s whole `watchlist`, and answers the one it keeps.
    pub(crate) async fn sync_watchlist(
        &self,
        user_id: i64,
        watchlist: Vec<WatchlistEntry>,
    ) -> Option<Vec<WatchlistEntry>> {
        let sync = async |linker: Arc<dyn ServerLinker>| {
            linker.sync_watchlist(user_id, watchlist.clone()).await
        };
        self.transfer(user_id, "sync a User's Watchlist", sync)
            .await
    }

    /// What `call` moves of the User `user_id`, `None` without a Plugin that links the Server, or
    /// once it failed or took [`BULK`], which is logged as failing to `what`. The next sign-in,
    /// scan or change tries again.
    async fn transfer<T, F>(
        &self,
        user_id: i64,
        what: &str,
        call: impl Fn(Arc<dyn ServerLinker>) -> F,
    ) -> Option<T>
    where
        F: Future<Output = Result<T, PluginError>>,
    {
        let answer = (self.points)
            .ask_first::<dyn ServerLinker, _, _, _, _>(async |linker| {
                within_bulk(call(linker)).await
            })
            .await?;
        let plugin = &answer.plugin;
        answer
            .result
            .inspect_err(|_failure| {
                // Ids only: `ask_first` already puts the failure's message on the Plugin's page.
                tracing::warn!(%plugin, user = user_id, "could not {what}");
            })
            .ok()
    }

    /// The Plugin the Server Addresses tick asks: the first enabled one that links the Server.
    pub(crate) fn linking_plugin(&self) -> Option<PluginId> {
        (self.points.ranked::<dyn ServerLinker>().into_iter()).next()
    }

    /// Tells `plugin` the Server's `addresses`, in its scope.
    pub(crate) async fn tell_addresses(&self, plugin: &PluginId, addresses: ServerAddresses) {
        self.ask_in_scope(plugin, move |linker| {
            let addresses = addresses.clone();
            async move { linker.set_addresses(addresses).await }
        })
        .await;
    }

    /// Whether `plugin` has the Server Linked, asked in its scope for the Server Addresses tick.
    /// Not, if it fails to say.
    pub(crate) async fn is_server_linked_by(&self, plugin: &PluginId) -> bool {
        let link = self.ask_in_scope(plugin, |linker| async move { linker.link().await });
        let Some(link) = link.await else {
            return false;
        };
        self.saw(Whose::Server, Some(&link));
        matches!(link, Link::Linked { .. })
    }

    /// What `call` gets from `plugin`, asked in its scope, so disabling it or shutting down cancels
    /// and joins the call. `None` once it failed, didn't answer within [`BULK`], or its scope was
    /// cancelled.
    async fn ask_in_scope<R, F>(
        &self,
        plugin: &PluginId,
        call: impl Fn(Arc<dyn ServerLinker>) -> F + Send + Sync + 'static,
    ) -> Option<R>
    where
        R: Send + 'static,
        F: Future<Output = Result<R, PluginError>> + Send,
    {
        let (asking, asked) = (self.points.clone(), plugin.clone());
        let (told, answered) = oneshot::channel();
        let spawned = self.points.spawn_in(plugin, async move {
            let answer = asking
                .ask(&asked, |linker: Arc<dyn ServerLinker>| {
                    within_bulk(call(linker))
                })
                .await;
            let _: Result<(), _> = told.send(answer.and_then(|answer| answer.result.ok()));
        });
        if !spawned {
            return None;
        }
        // A cancelled scope drops the sender, which ends the wait as well.
        answered.await.ok().flatten()
    }
}

/// What `call` answers, or a failure once it has taken [`BULK`].
async fn within_bulk<T>(
    call: impl Future<Output = Result<T, PluginError>>,
) -> Result<T, PluginError> {
    tokio::time::timeout(BULK, call)
        .await
        .unwrap_or_else(|_elapsed| Err("it didn't answer in time".into()))
}
