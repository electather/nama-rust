//! Calling the Hub as the Linked Server, whose Link the Kernel keeps (ADR 0105). Every Hub call
//! made as the Server goes through [`Linker::call`], so one rule decides what a Link the Hub dropped means.
//! Syncing the Watch State of the Users the Kernel Linked both ways is in `watch_states.rs`, their
//! Subtitle and Audio Preferences in `track_preferences.rs`, their Watchlists in `watchlist.rs`,
//! and their plays as Viewing Events in `viewing.rs`. What goes to the Hub waits in a queue per
//! kind, in `queue.rs`.

pub(crate) mod queue;
mod track_preferences;
mod viewing;
mod watch_states;
mod watchlist;

use std::{future::Future, sync::Arc};

use nama_hub_proto::nama::hub::v1::{
    TrackPreferenceChange as TrackPreferenceChangeProto, WatchStateChange as WatchStateChangeProto,
};
use nama_plugin_api::{
    BoxFuture, PluginError,
    automation::Kernel,
    contract::StoreKey,
    job::{JobRunner, RunJob},
    points::{TrackPreference, WatchStateChange},
    server_link::{ServerLinker, WatchlistEntry},
};

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

/// Calls the Hub as the Server, through the Kernel's Link.
pub(crate) struct Linker {
    hub: Hub,
    link: Arc<dyn KernelLink>,
    kernel: Arc<dyn Kernel>,
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
        let Some(credential) = self.link.credential().await? else {
            return Ok(None);
        };
        match call(self.hub.linked(&credential)).await {
            Err(err) if err.is::<LinkDropped>() => {
                self.link.forget(&credential).await?;
                Ok(None)
            }
            called => called.map(Some),
        }
    }

    /// Whether `user_id` is Linked, as the Kernel keeps it.
    pub(crate) async fn user_linked(&self, user_id: i64) -> Result<bool, PluginError> {
        self.link.user_linked(user_id).await
    }
}

impl ServerLinker for Linker {
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

fn store_key(key: &str) -> StoreKey {
    StoreKey {
        key: key.to_owned(),
        ..Default::default()
    }
}
