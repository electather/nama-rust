//! Calling the Hub as the Linked Server, whose Link the Kernel keeps (ADR 0105), and through it
//! sharing what the Admin chooses with Nama's developers (ADR 0054). Every Hub call made as the
//! Server goes through [`Linker::call`], so one rule decides what a Link the Hub dropped means.
//! The Server's Users' Links are in `users.rs`, syncing their Watch State both ways in `watch_states.rs`, their Subtitle and
//! Audio Preferences in `track_preferences.rs`, and their Watchlists in `watchlist.rs`, their plays
//! as Viewing Events in `viewing.rs`, and signing in with an Account, and Users created from one,
//! in `accounts.rs`. What goes to the Hub waits in a queue per kind, in `queue.rs`.

mod accounts;
pub(crate) mod queue;
mod track_preferences;
mod users;
mod viewing;
mod watch_states;
mod watchlist;

use std::{
    collections::HashMap,
    future::Future,
    sync::{Arc, Mutex, PoisonError},
};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use nama_hub_proto::nama::hub::v1::{
    TrackPreferenceChange as TrackPreferenceChangeProto, WatchStateChange as WatchStateChangeProto,
};
use nama_plugin_api::{
    BoxFuture, PluginError,
    automation::{Kernel, functions::StoreGet},
    contract::StoreKey,
    diagnostics_sharer::{DiagnosticsSharer, Signal},
    job::{JobRunner, RunJob},
    points::{TrackPreference, WatchStateChange},
    server_link::{AccountSignIn, Link, ServerLinker, WatchlistEntry},
};
use ring::digest;

use crate::{
    KernelLink,
    hub::{Hub, LinkDropped, LinkedHub},
};

use self::queue::Queue;

pub(crate) use self::{
    track_preferences::TrackPreferenceSync,
    viewing::{ViewingEvents, send_job},
    watch_states::{WatchStateSync, sync_job},
};

/// Runs the Hub Plugin's Jobs. One runner serves them all, as the Kernel asks only the first
/// `JobRunner` a Plugin provides.
pub(crate) struct Jobs {
    pub(crate) linker: Arc<Linker>,
    pub(crate) viewing: ViewingEvents,
}

impl JobRunner for Jobs {
    fn run<'a>(&'a self, run: &'a RunJob) -> BoxFuture<'a, Result<(), PluginError>> {
        Box::pin(async move {
            match run.job_id.as_str() {
                watch_states::SYNC => {
                    // Each queue apart, so one the Hub refuses holds back only its own.
                    let watch_states = self.linker.send_queued().await;
                    let preferences = self.linker.send_queued_preferences().await;
                    watch_states.and(preferences)
                }
                viewing::SEND_JOB => self.viewing.send().await,
                other => Err(format!("the Hub Plugin has no Job {other}").into()),
            }
        })
    }
}

/// The Server's Link as the Kernel keeps it.
struct Kept {
    /// What the Server presents on every Hub call (ADR 0098).
    credential: String,
}

impl Kept {
    /// Names this Link apart from the Server's earlier ones, without holding its credential.
    fn id(&self) -> String {
        URL_SAFE_NO_PAD.encode(digest::digest(&digest::SHA256, self.credential.as_bytes()))
    }
}

/// Calls the Hub as the Server, through the Kernel's Link.
pub(crate) struct Linker {
    hub: Hub,
    link: Arc<dyn KernelLink>,
    kernel: Arc<dyn Kernel>,
    /// What the Plugin knows of each User's Link but what it keeps in the Store, under the
    /// [`Kept::id`] of the Server's Link it learned it through.
    users: Mutex<(Option<String>, HashMap<i64, users::UserState>)>,
    watch_states: Queue<WatchStateChangeProto>,
    preferences: Queue<TrackPreferenceChangeProto>,
}

impl Linker {
    pub(crate) fn new(hub: Hub, link: Arc<dyn KernelLink>, kernel: Arc<dyn Kernel>) -> Self {
        Self {
            watch_states: Queue::new(Arc::clone(&kernel)),
            preferences: Queue::new(Arc::clone(&kernel)),
            hub,
            link,
            kernel,
            users: Mutex::default(),
        }
    }

    /// Makes `call` to the Hub as the Linked Server: `None` while the Server isn't Linked, and
    /// once the Hub answers it doesn't know the Link any more, which forgets the Link. A Hub that
    /// is unreachable or fails is a failure, and keeps the Link.
    pub(crate) async fn call<T, Called>(
        &self,
        call: impl FnOnce(LinkedHub) -> Called,
    ) -> Result<Option<T>, PluginError>
    where
        Called: Future<Output = Result<T, PluginError>>,
    {
        let Some(kept) = self.kept().await? else {
            return Ok(None);
        };
        match call(self.hub.linked(&kept.credential)).await {
            Err(err) if err.is::<LinkDropped>() => {
                self.link.forget(&kept.credential).await?;
                Ok(None)
            }
            called => called.map(Some),
        }
    }

    /// The Link that waits for someone to enter `code` on the Hub.
    fn waiting_on(&self, code: String) -> Link {
        Link::Waiting {
            code,
            url: self.hub.url().to_owned(),
        }
    }

    /// Whom `token`, which the Hub signed, names, as the Kernel checks it.
    pub(crate) async fn token_subject(&self, token: &str) -> Result<String, PluginError> {
        self.link.token_subject(token).await
    }

    /// Sends `batch` to the Hub as the Linked Server, or drops it while not Linked.
    async fn share_batch(&self, signal: Signal, batch: Vec<u8>) -> Result<(), PluginError> {
        self.call(|hub| hub.share(signal, batch)).await?;
        Ok(())
    }

    async fn kept(&self) -> Result<Option<Kept>, PluginError> {
        let kept = (self.link.credential().await?).map(|credential| Kept { credential });
        let id = kept.as_ref().map(Kept::id);
        let mut users = self.users.lock().unwrap_or_else(PoisonError::into_inner);
        // The Hub Unlinked the Server's Users with an earlier Link: what is known of them is that
        // Link's, so none shows once the Server is Linked again.
        if users.0 != id {
            *users = (id, HashMap::new());
        }
        drop(users);
        Ok(kept)
    }

    async fn stored(&self, key: &str) -> Result<Option<Vec<u8>>, PluginError> {
        Ok(self.kernel.ask::<StoreGet>(store_key(key)).await?.value)
    }
}

impl ServerLinker for Linker {
    fn user_link(&self, user_id: i64) -> BoxFuture<'_, Result<Option<Link>, PluginError>> {
        Box::pin(self.current_user_link(user_id))
    }

    fn start_user_link<'a>(
        &'a self,
        user_id: i64,
        user_name: &'a str,
    ) -> BoxFuture<'a, Result<Link, PluginError>> {
        Box::pin(self.start_user(user_id, user_name))
    }

    fn unlink_user(&self, user_id: i64) -> BoxFuture<'_, Result<(), PluginError>> {
        Box::pin(self.forget_user_link(user_id))
    }

    fn invite_user<'a>(
        &'a self,
        user_id: i64,
        user_name: &'a str,
        email: &'a str,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        Box::pin(self.invite(user_id, user_name, email))
    }

    fn start_account_sign_in<'a>(
        &'a self,
        return_url: &'a str,
    ) -> BoxFuture<'a, Result<AccountSignIn, PluginError>> {
        Box::pin(self.start_sign_in(return_url))
    }

    fn finish_account_sign_in<'a>(
        &'a self,
        handle: &'a str,
        code: &'a str,
    ) -> BoxFuture<'a, Result<Option<i64>, PluginError>> {
        Box::pin(self.finish_sign_in(handle, code))
    }

    fn watch_states(
        &self,
        user_id: i64,
    ) -> BoxFuture<'_, Result<Vec<WatchStateChange>, PluginError>> {
        Box::pin(self.pull(user_id))
    }

    fn track_preferences(
        &self,
        user_id: i64,
    ) -> BoxFuture<'_, Result<Vec<TrackPreference>, PluginError>> {
        Box::pin(self.pull_preferences(user_id))
    }

    fn sync_watchlist(
        &self,
        user_id: i64,
        watchlist: Vec<WatchlistEntry>,
    ) -> BoxFuture<'_, Result<Vec<WatchlistEntry>, PluginError>> {
        Box::pin(self.merge_watchlist(user_id, watchlist))
    }
}

impl DiagnosticsSharer for Linker {
    fn share(&self, signal: Signal, batch: Vec<u8>) -> BoxFuture<'_, Result<(), PluginError>> {
        Box::pin(self.share_batch(signal, batch))
    }
}

fn store_key(key: &str) -> StoreKey {
    StoreKey {
        key: key.to_owned(),
        ..Default::default()
    }
}
