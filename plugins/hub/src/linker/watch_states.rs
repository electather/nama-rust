//! Syncing Linked Users' Watch State with the Hub (ADR 0008): each change the Kernel tells of is
//! queued in the Plugin's Store, and the Sync Job sends the queue, in order, so changes made
//! while the Hub is unreachable go once it is back. The Kernel pulls their Account's back. A User
//! whose Link the Plugin never learned has nothing queued until the Hub can be asked.

use std::sync::Arc;

use nama_hub_proto::nama::hub::v1::WatchStateChange as Change;
use nama_plugin_api::{
    BoxFuture, Observer, PluginError,
    automation::functions::StoreDelete,
    job::Job,
    points::{WatchStateChange, WatchStateChanges, WatchStatesChanged},
};
use serde::{Deserialize, Serialize};

use super::{Linker, store_key};

/// The id of the Job that sends the queue.
pub(super) const SYNC: &str = "sync";

/// The Store key of the [`Queue`].
const QUEUE_KEY: &str = "watch-state-queue";

/// The most changes a page of the queue holds, and so one push.
// ponytail: counted, not measured, so a page stays under the Store's 64 KiB a value only while a
// title has about 6 short ids, as Servers' Matches do; measure a page's bytes if that changes.
const PAGE_CHANGES: usize = 100;

/// The most ids a change sends, and the most characters each may have, as the Hub takes them: past
/// either it would refuse the page, which would hold back every later one.
const MAX_IDS: usize = 20;
pub(super) const MAX_ID_LENGTH: usize = 100;

/// Which pages of queued changes the Store keeps: `first..next`, oldest first.
#[derive(Default, Serialize, Deserialize)]
struct Queue {
    first: u64,
    next: u64,
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
        let _queue = self.queue.lock().await;
        let mut queue: Queue = self.stored_json(QUEUE_KEY).await?.unwrap_or_default();
        // The last page fills up before another starts.
        let mut page = if queue.first < queue.next {
            queue.next -= 1;
            self.page(queue.next).await?
        } else {
            Vec::new()
        };
        for change in &changed.changes {
            if let Some(change) = change_to_proto(changed.user_id, change) {
                enqueue(&mut page, change);
            }
        }
        for chunk in page.chunks(PAGE_CHANGES) {
            self.put_json(page_key(queue.next), &chunk).await?;
            queue.next += 1;
        }
        self.put_json(QUEUE_KEY.to_owned(), &queue).await
    }

    /// Sends the queue to the Hub, a page per push, forgetting each page the Hub took. Without the
    /// Server's Link, such as once the Hub dropped it, the queue has nowhere to go, and is
    /// forgotten.
    pub(super) async fn send_queued(&self) -> Result<(), PluginError> {
        let _queue = self.queue.lock().await;
        let Some(mut queue) = self.stored_json::<Queue>(QUEUE_KEY).await? else {
            return Ok(());
        };
        while queue.first < queue.next {
            let page = self.page(queue.first).await?;
            self.call(|hub| hub.push_watch_states(page)).await?;
            self.kernel
                .ask::<StoreDelete>(store_key(&page_key(queue.first)))
                .await?;
            queue.first += 1;
            self.put_json(QUEUE_KEY.to_owned(), &queue).await?;
        }
        Ok(())
    }

    /// The Watch State of the Account `user_id` is Linked to, once the Hub has every change queued
    /// here, so what it answers is never older than this Server's own; empty without the Server's
    /// Link.
    pub(super) async fn pull(&self, user_id: i64) -> Result<Vec<WatchStateChange>, PluginError> {
        self.send_queued().await?;
        let pulled = self.call(|hub| hub.pull_watch_states(user_id)).await?;
        Ok((pulled.unwrap_or_default().into_iter())
            .map(change_from_proto)
            .collect())
    }

    /// The changes queued in page `nth`.
    async fn page(&self, nth: u64) -> Result<Vec<Change>, PluginError> {
        Ok(self.stored_json(&page_key(nth)).await?.unwrap_or_default())
    }
}

/// Adds `change` to `page`, in place of a queued change of the same User and title, which it
/// supersedes: only the latest of a title's changes needs to reach the Hub. One made before a
/// change queued already, as when two writes are told out of order, is dropped.
fn enqueue(page: &mut Vec<Change>, change: Change) {
    let same_title = |queued: &Change| {
        (queued.user_id, queued.episode, &queued.ids)
            == (change.user_id, change.episode, &change.ids)
    };
    if (page.iter()).any(|queued| same_title(queued) && queued.changed_at_ms > change.changed_at_ms)
    {
        return;
    }
    page.retain(|queued| !same_title(queued));
    page.push(change);
}

/// The User `user_id`'s `change`, as the Hub takes it, or `None` if none of its ids fits.
fn change_to_proto(user_id: i64, change: &WatchStateChange) -> Option<Change> {
    let fits = |id: &&String| id.chars().count() <= MAX_ID_LENGTH;
    let ids: Vec<_> = change
        .ids
        .iter()
        .filter(fits)
        .take(MAX_IDS)
        .cloned()
        .collect();
    (!ids.is_empty()).then(|| Change {
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

/// The Store key of the queue's page `nth`.
fn page_key(nth: u64) -> String {
    format!("{QUEUE_KEY}/{nth}")
}
