//! Syncing Linked Users' Watch State with the Hub (ADR 0008): each change the Kernel tells of is
//! queued in the Plugin's Store, and the Sync Job sends the queue, in order, so changes made
//! while the Hub is unreachable go once it is back. The Kernel pulls their Account's back. A User
//! whose Link the Plugin never learned has nothing queued until the Hub can be asked.

use std::{hash::Hash, sync::Arc};

use nama_hub_proto::nama::hub::v1::WatchStateChange as Change;
use nama_plugin_api::{
    BoxFuture, Observer, PluginError,
    job::Job,
    points::{WatchStateChange, WatchStateChanges, WatchStatesChanged},
};

use super::{Linker, queue::Queued};
use crate::hub::limits;

/// The id of the Job that sends the queue.
pub(super) const SYNC: &str = "sync";

impl Queued for Change {
    // An older Hub Plugin's pages carry over: it kept them under the same keys, as the same JSON.
    const STORE_KEY: &'static str = "watch-state-queue";
    const PER_CALL: usize = limits::PUSH_CHANGES;

    fn key(&self) -> impl Eq + Hash + '_ {
        (self.user_id, self.episode, &self.ids)
    }

    fn changed_at_ms(&self) -> i64 {
        self.changed_at_ms
    }
}

/// The Sync Job, which the Kernel runs every minute.
pub(crate) fn sync_job() -> Job {
    Job {
        id: SYNC.to_owned(),
        name: "Sync with the Hub".to_owned(),
        description: "Sends Linked Users' Watch State and Subtitle and Audio Preference changes \
                      to the Hub. Those made while it is unreachable are kept and sent, in \
                      order, once it is back."
            .to_owned(),
        schedule: Some("* * * * *".to_owned()),
    }
}

/// Queues Linked Users' Watch State changes as the Kernel tells of them, for the Sync Job to send.
pub(crate) struct WatchStateSync(pub(crate) Arc<Linker>);

impl Observer<WatchStatesChanged> for WatchStateSync {
    fn observe<'a>(
        &'a self,
        event: &'a WatchStateChanges,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        Box::pin(self.0.queue_changes(event))
    }
}

impl Linker {
    /// Queues `changed`, if its User is Linked.
    async fn queue_changes(&self, changed: &WatchStateChanges) -> Result<(), PluginError> {
        if !self.user_linked(changed.user_id).await? {
            return Ok(());
        }
        let to_send = (changed.changes.iter())
            .filter_map(|change| change_to_proto(changed.user_id, change))
            .collect();
        self.watch_states.add(to_send).await
    }

    /// Sends the queue to the Hub, a page per push, forgetting each page the Hub took. Without the
    /// Server's Link, such as once the Hub dropped it, the queue has nowhere to go, and is
    /// forgotten.
    pub(super) async fn send_queued(&self) -> Result<(), PluginError> {
        let push =
            |page| -> BoxFuture<'_, _> { Box::pin(self.call(|hub| hub.push_watch_states(page))) };
        self.watch_states.send(push).await
    }

    /// The Watch State of the Account `user_id` is Linked to, once the Hub has every change queued
    /// here; empty without the Server's Link.
    pub(super) async fn pull(&self, user_id: i64) -> Result<Vec<WatchStateChange>, PluginError> {
        self.send_queued().await?;
        let pulled = self.call(|hub| hub.pull_watch_states(user_id)).await?;
        Ok((pulled.unwrap_or_default().into_iter())
            .map(change_from_proto)
            .collect())
    }
}

/// The User `user_id`'s `change`, as the Hub takes it, or `None` if none of its ids fits or the
/// Hub wouldn't take when it was made.
fn change_to_proto(user_id: i64, change: &WatchStateChange) -> Option<Change> {
    let ids: Vec<_> = (change.ids.iter())
        .filter(|id| limits::takes_external_id(id))
        .take(limits::WATCH_STATE_IDS)
        .cloned()
        .collect();
    (!ids.is_empty() && limits::takes_changed_at(change.changed_at_ms)).then(|| Change {
        user_id,
        episode: change.episode,
        ids,
        played: change.played,
        position_ms: change.position_ms,
        duration_ms: change.duration_ms,
        changed_at_ms: change.changed_at_ms,
        ..Default::default()
    })
}

/// A change the Hub answered, as the Kernel takes it.
fn change_from_proto(change: Change) -> WatchStateChange {
    WatchStateChange {
        episode: change.episode,
        ids: change.ids,
        played: change.played,
        position_ms: change.position_ms,
        duration_ms: change.duration_ms,
        changed_at_ms: change.changed_at_ms,
    }
}
