//! Syncing Linked Users' Subtitle and Audio Preferences with the Hub (ADR 0092), as Watch State
//! syncs: each change the Kernel tells of is queued in the Plugin's Store, the Sync Job sends the
//! queue, and the Kernel pulls their Account's back. A Preference describes a Track, never names
//! one, so each Server matches it against its own (ADR 0035). A Show's or Movie's goes to the Hub
//! once under each of its ids, as the Hub keeps a row under each, and comes back under one.

use std::{hash::Hash, sync::Arc};

use nama_hub_proto::nama::hub::v1::{TitleKind, TrackPreferenceChange as Change};
use nama_plugin_api::{
    BoxFuture, Observer, PluginError,
    automation::Kernel,
    points::{
        TrackDescription, TrackPreference, TrackPreferenceChange, TrackPreferenceChanged,
        TrackPreferenceFor,
    },
    server_link::Link,
};

use super::{
    Linker,
    queue::{Queue, Queued},
};
use crate::hub::{Hub, limits};

impl Queued for Change {
    const STORE_KEY: &'static str = "track-preferences";
    const PER_CALL: usize = limits::PUSH_CHANGES;
    const OLD_STORE_KEY: Option<&'static str> = Some("track-preference-queue");

    fn key(&self) -> impl Eq + Hash + '_ {
        (self.user_id, self.audio, self.title_kind, &self.title_id)
    }

    fn changed_at_ms(&self) -> i64 {
        self.changed_at_ms
    }
}

/// The queue of Track Preference changes in `kernel`'s Store, which pushes them to `hub`.
pub(super) fn queue(hub: &Hub, kernel: Arc<dyn Kernel>) -> Queue<Change> {
    let hub = hub.clone();
    Queue::new(kernel, move |credential, page| {
        let hub = hub.clone();
        Box::pin(async move { hub.push_track_preferences(&credential, page).await })
    })
}

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
        self.preferences.add(changes_to_proto(changed)).await
    }

    /// The Track Preferences of the Account `user_id` is Linked to, once the Hub has every change
    /// queued here; empty without the Server's Link.
    pub(super) async fn pull_preferences(
        &self,
        user_id: i64,
    ) -> Result<Vec<TrackPreference>, PluginError> {
        let pulled = (self.preferences)
            .pull(async |credential| self.hub.pull_track_preferences(credential, user_id).await)
            .await?;
        Ok(pulled.into_iter().filter_map(change_from_proto).collect())
    }
}

/// `changed`, as the Hub takes it: a Show's or Movie's once under each id that fits, and none if
/// its language is longer than the Hub takes or the Hub wouldn't take when it was made. A Track's
/// title is cut short there instead, as it only ranks Tracks that match.
fn changes_to_proto(changed: &TrackPreferenceChange) -> Vec<Change> {
    let preference = &changed.preference;
    let (title_kind, title_ids) = match &preference.kept_for {
        TrackPreferenceFor::User => (TitleKind::TITLE_KIND_UNSPECIFIED, None),
        TrackPreferenceFor::Movie(ids) => (TitleKind::TITLE_KIND_MOVIE, Some(ids)),
        TrackPreferenceFor::Show(ids) => (TitleKind::TITLE_KIND_SHOW, Some(ids)),
    };
    let title_ids: Vec<&str> = match title_ids {
        None => vec![""],
        Some(ids) => (ids.iter().map(String::as_str))
            .filter(|id| limits::takes_external_id(id))
            .collect(),
    };
    let like = preference.like.clone().unwrap_or_default();
    let language = like.language.unwrap_or_default();
    if language.chars().count() > limits::TRACK_LANGUAGE_CHARS
        || !limits::takes_changed_at(preference.changed_at_ms)
    {
        return Vec::new();
    }
    let change = Change {
        user_id: changed.user_id,
        audio: preference.audio,
        title_kind: title_kind.into(),
        off: preference.like.is_none(),
        language,
        forced: like.forced,
        sdh: like.sdh,
        title: (like.title.unwrap_or_default().chars())
            .take(limits::TRACK_TITLE_CHARS)
            .collect(),
        foreign_audio_only: preference.foreign_audio_only,
        changed_at_ms: preference.changed_at_ms,
        ..Default::default()
    };
    (title_ids.into_iter())
        .map(|title_id| Change {
            title_id: title_id.to_owned(),
            ..change.clone()
        })
        .collect()
}

/// A change the Hub answered, as the Kernel takes it; `None` for one kept for a kind of title
/// this Plugin doesn't know.
fn change_from_proto(change: Change) -> Option<TrackPreference> {
    let kept_for = match change.title_kind.as_known()? {
        TitleKind::TITLE_KIND_UNSPECIFIED => TrackPreferenceFor::User,
        TitleKind::TITLE_KIND_MOVIE => TrackPreferenceFor::Movie(vec![change.title_id]),
        TitleKind::TITLE_KIND_SHOW => TrackPreferenceFor::Show(vec![change.title_id]),
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
