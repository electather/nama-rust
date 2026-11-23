//! Syncing Linked Users' Subtitle and Audio Preferences with the Hub (ADR 0092), as Watch State
//! syncs: each change the Kernel tells of is queued in the Plugin's Store, the Sync Job sends the
//! queue, and the Kernel pulls their Account's back. A Preference describes a Track, never names
//! one, so each Server matches it against its own (ADR 0035).

use std::sync::Arc;

use nama_hub_proto::nama::hub::v1::{TitleKind, TrackPreferenceChange as Change};
use nama_plugin_api::{
    BoxFuture, Observer, PluginError,
    automation::functions::StoreDelete,
    points::{
        TrackDescription, TrackPreference, TrackPreferenceChange, TrackPreferenceChanged,
        TrackPreferenceFor,
    },
    server_link::Link,
};

use super::{Linker, store_key, watch_states::MAX_ID_LENGTH};

/// The Store key of the queue: the changes not yet sent, oldest first.
// ponytail: one value, under the Store's 64 KiB while about 400 Preferences with short Track titles
// wait, one per User and title however often each changes, and past it a change fails to queue;
// page it as Watch State's queue is if Hub outages or titles grow that long.
const QUEUE_KEY: &str = "track-preference-queue";

/// The most changes one push holds, as the Hub takes them.
const PUSH_CHANGES: usize = 500;

/// The most characters a language and a Track's title may have, as the Hub takes them, as
/// [`MAX_ID_LENGTH`] is an id's: past one it would refuse the push, which would hold back every
/// later one.
const MAX_LANGUAGE_LENGTH: usize = 100;
const MAX_TITLE_LENGTH: usize = 1000;

/// Queues Linked Users' Track Preference changes as the Kernel tells of them, for the Sync Job to
/// send.
pub(crate) struct TrackPreferenceSync(pub(crate) Arc<Linker>);

impl Observer<TrackPreferenceChanged> for TrackPreferenceSync {
    fn observe<'a>(
        &'a self,
        event: &'a TrackPreferenceChange,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        Box::pin(self.0.queue_preference(event))
    }
}

impl Linker {
    /// Queues `changed`, if its User is Linked, asking the Hub as Watch State changes do.
    async fn queue_preference(&self, changed: &TrackPreferenceChange) -> Result<(), PluginError> {
        let Some(Link::Linked { .. }) = self.current_user_link(changed.user_id).await? else {
            return Ok(());
        };
        let Some(change) = change_to_proto(changed) else {
            return Ok(());
        };
        let _queue = self.queue.lock().await;
        let mut queued: Vec<Change> = self.stored_json(QUEUE_KEY).await?.unwrap_or_default();
        enqueue(&mut queued, change);
        self.put_json(QUEUE_KEY.to_owned(), &queued).await
    }

    /// Sends the queue to the Hub and forgets it. Without the Server's Link, the queue has
    /// nowhere to go, and is forgotten.
    pub(super) async fn send_queued_preferences(&self) -> Result<(), PluginError> {
        let _queue = self.queue.lock().await;
        let Some(queued) = self.stored_json::<Vec<Change>>(QUEUE_KEY).await? else {
            return Ok(());
        };
        if let Some(kept) = self.kept().await? {
            // A chunk sent again after a later one fails is merged the same way twice.
            for chunk in queued.chunks(PUSH_CHANGES) {
                (self.hub)
                    .push_track_preferences(&kept.credential, chunk.to_vec())
                    .await?;
            }
        }
        self.kernel.ask::<StoreDelete>(store_key(QUEUE_KEY)).await?;
        Ok(())
    }

    /// The Track Preferences of the Account `user_id` is Linked to, once the Hub has every change
    /// queued here, so what it answers is never older than this Server's own; empty without the
    /// Server's Link.
    pub(super) async fn pull_preferences(
        &self,
        user_id: i64,
    ) -> Result<Vec<TrackPreference>, PluginError> {
        self.send_queued_preferences().await?;
        let Some(kept) = self.kept().await? else {
            return Ok(Vec::new());
        };
        let pulled = (self.hub)
            .pull_track_preferences(&kept.credential, user_id)
            .await?;
        Ok(pulled.into_iter().filter_map(change_from_proto).collect())
    }
}

/// Adds `change` to `queue`, in place of a queued change of the same User's same Preference,
/// unless that one was made later, as when two changes are told out of order.
fn enqueue(queue: &mut Vec<Change>, change: Change) {
    let same_preference = |queued: &Change| {
        (
            queued.user_id,
            queued.audio,
            queued.title_kind,
            &queued.title_id,
        ) == (
            change.user_id,
            change.audio,
            change.title_kind,
            &change.title_id,
        )
    };
    if (queue.iter())
        .any(|queued| same_preference(queued) && queued.changed_at_ms > change.changed_at_ms)
    {
        return;
    }
    queue.retain(|queued| !same_preference(queued));
    queue.push(change);
}

/// `changed`, as the Hub takes it, or `None` if its title's id or its language is longer than the
/// Hub takes. A Track's title is cut short there instead, as it only ranks Tracks that match.
fn change_to_proto(changed: &TrackPreferenceChange) -> Option<Change> {
    let preference = &changed.preference;
    let (title_kind, title_id) = match &preference.kept_for {
        TrackPreferenceFor::User => (TitleKind::TITLE_KIND_UNSPECIFIED, ""),
        TrackPreferenceFor::Movie(id) => (TitleKind::TITLE_KIND_MOVIE, id.as_str()),
        TrackPreferenceFor::Show(id) => (TitleKind::TITLE_KIND_SHOW, id.as_str()),
    };
    let like = preference.like.clone().unwrap_or_default();
    let language = like.language.unwrap_or_default();
    if title_id.chars().count() > MAX_ID_LENGTH || language.chars().count() > MAX_LANGUAGE_LENGTH {
        return None;
    }
    Some(Change {
        user_id: changed.user_id,
        audio: preference.audio,
        title_kind: title_kind.into(),
        title_id: title_id.to_owned(),
        off: preference.like.is_none(),
        language,
        forced: like.forced,
        sdh: like.sdh,
        title: (like.title.unwrap_or_default().chars())
            .take(MAX_TITLE_LENGTH)
            .collect(),
        foreign_audio_only: preference.foreign_audio_only,
        changed_at_ms: preference.changed_at_ms,
        ..Default::default()
    })
}

/// A change the Hub answered, as the Kernel takes it; `None` for one kept for a kind of title
/// this Plugin doesn't know.
fn change_from_proto(change: Change) -> Option<TrackPreference> {
    let kept_for = match change.title_kind.as_known()? {
        TitleKind::TITLE_KIND_UNSPECIFIED => TrackPreferenceFor::User,
        TitleKind::TITLE_KIND_MOVIE => TrackPreferenceFor::Movie(change.title_id),
        TitleKind::TITLE_KIND_SHOW => TrackPreferenceFor::Show(change.title_id),
    };
    let tagged = |tag: String| (!tag.is_empty()).then_some(tag);
    Some(TrackPreference {
        audio: change.audio,
        kept_for,
        like: (!change.off).then(|| TrackDescription {
            language: tagged(change.language),
            forced: change.forced,
            sdh: change.sdh,
            title: tagged(change.title),
        }),
        foreign_audio_only: change.foreign_audio_only,
        changed_at_ms: change.changed_at_ms,
    })
}
