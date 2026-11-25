//! What of Linked Users' Watch State, Subtitle and Audio Preferences and Watchlists goes to the Hub
//! and comes back (ADR 0008, ADR 0056, ADR 0092), and their plays as Viewing Events (ADR 0093), in
//! the Hub API's terms. A Watch State or Preference change waits in `hub_sync_queue`, queued in the
//! write that made it, as does a Viewing Event, until [`ServerLink`](super::ServerLink) sends it,
//! so what's made while the Hub is unreachable goes once it's back. A Preference describes a Track,
//! never names one, so each Server matches it against its own (ADR 0035); a Show's or Movie's goes
//! to the Hub once under each of its ids, as the Hub keeps a row under each, and comes back under
//! one. What the Hub wouldn't take never leaves the Server, as it would refuse the whole call.

use std::cmp::Reverse;

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use nama_hub_proto::nama::hub::v1::{
    DeviceKind as HubDeviceKind, TitleKind, TrackPreferenceChange as PreferenceProto,
    UserViewingEvent, ViewingEvent as ViewingEventProto, WatchStateChange as WatchStateProto,
    WatchlistEntry as EntryProto,
};
use nama_plugin_api::{
    contract::DeviceKind,
    points::{TrackDescription, TrackPreference, TrackPreferenceFor, WatchStateChange},
    server_link::WatchlistEntry,
};
use serde::Serialize;
use sqlx::SqliteConnection;

use super::{hub, server};
use crate::db::Database;

/// What a change waiting for the Hub is, as `hub_sync_queue.kind` keeps it. Each kind is sent apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[repr(i64)]
pub(crate) enum Kind {
    WatchState = 0,
    Preference = 1,
    ViewingEvent = 2,
}

/// One play of a Movie or Episode, as the Hub records it under the User's Account (ADR 0093).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ViewingEvent {
    /// The External ID its Watch State is kept under, such as `tmdb-603`.
    pub(crate) id: String,
    /// An Episode's Show's External ID, such as `tmdb-1399`; `None` for a Movie.
    pub(crate) show_id: Option<String>,
    /// When it started, in seconds since the Unix epoch.
    pub(crate) started_at: i64,
    /// The minute of the day it started, in the Server's time zone.
    pub(crate) time_of_day_minutes: i32,
    pub(crate) stop_position_ms: i64,
    pub(crate) duration_ms: i64,
    /// What kind of Device played it.
    pub(crate) device_kind: DeviceKind,
}

/// A change waiting for the Hub.
pub(crate) struct Queued {
    pub(super) id: i64,
    pub(super) user_id: i64,
    /// The Hub API's message, as its JSON.
    pub(crate) change: String,
}

/// Queues the User `user_id`'s Watch State `changes` for the Hub through `conn`, in the write
/// that made them, while the Server is Linked.
pub(crate) async fn queue_watch_states(
    conn: &mut SqliteConnection,
    user_id: i64,
    changes: &[WatchStateChange],
) -> Result<(), sqlx::Error> {
    let to_send = (changes.iter()).filter_map(|change| watch_state_to_proto(user_id, change));
    queue(conn, user_id, Kind::WatchState, to_send).await
}

/// Queues the User `user_id`'s Track `preferences` for the Hub through `conn`, in the write that
/// kept them, while the Server is Linked.
pub(crate) async fn queue_track_preferences(
    conn: &mut SqliteConnection,
    user_id: i64,
    preferences: &[TrackPreference],
) -> Result<(), sqlx::Error> {
    let to_send =
        (preferences.iter()).flat_map(|preference| preference_to_proto(user_id, preference));
    queue(conn, user_id, Kind::Preference, to_send).await
}

/// Queues the User `user_id`'s `viewed` for the Hub through `conn`, while the Server is Linked, each
/// with a key of its own.
pub(crate) async fn queue_viewing_events(
    conn: &mut SqliteConnection,
    user_id: i64,
    viewed: Vec<ViewingEvent>,
) -> Result<(), sqlx::Error> {
    let mut to_send = Vec::new();
    for event in viewed.iter().filter_map(viewing_event_to_proto) {
        let key = random_key().map_err(|err| sqlx::Error::Encode(err.into()))?;
        to_send.push(UserViewingEvent {
            user_id,
            key,
            event: event.into(),
            ..Default::default()
        });
    }
    queue(conn, user_id, Kind::ViewingEvent, to_send.into_iter()).await
}

async fn queue(
    conn: &mut SqliteConnection,
    user_id: i64,
    kind: Kind,
    changes: impl Iterator<Item = impl Serialize>,
) -> Result<(), sqlx::Error> {
    // A Server that isn't Linked has nowhere to send them, and Linking pushes a User's whole Watch
    // State and Preferences anyway.
    if !server::is_linked(conn).await? {
        return Ok(());
    }
    // ponytail: a title queued again waits twice, as the Hub keeps the change made last anyway;
    // replace a title's older change instead if queues grow long while the Hub is down.
    for change in changes {
        let change =
            serde_json::to_string(&change).map_err(|err| sqlx::Error::Encode(err.into()))?;
        sqlx::query!(
            "INSERT INTO hub_sync_queue (kind, user_id, change) VALUES (?, ?, ?)",
            kind,
            user_id,
            change
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

/// The oldest [`hub::PUSH_CHANGES`] changes of `kind` waiting, oldest first: fewer Viewing Events
/// than one call takes.
pub(crate) async fn waiting(db: &Database, kind: Kind) -> Result<Vec<Queued>, sqlx::Error> {
    let per_call = i64::try_from(hub::PUSH_CHANGES).unwrap_or(i64::MAX);
    sqlx::query_as!(
        Queued,
        "SELECT id, user_id, change FROM hub_sync_queue WHERE kind = ? ORDER BY id LIMIT ?",
        kind,
        per_call
    )
    .fetch_all(db.reader())
    .await
}

/// Forgets the changes of `kind` waiting up to and including `last`, once the Hub took them or they
/// can't go.
pub(super) async fn forget_through(
    db: &Database,
    kind: Kind,
    last: i64,
) -> Result<(), sqlx::Error> {
    db.write(move |tx| {
        Box::pin(async move {
            sqlx::query!(
                "DELETE FROM hub_sync_queue WHERE kind = ? AND id <= ?",
                kind,
                last
            )
            .execute(&mut **tx)
            .await?;
            Ok(())
        })
    })
    .await
}

/// Forgets every change waiting, once the Server isn't Linked: they have nowhere to go.
pub(super) async fn forget_all(db: &Database) -> Result<(), sqlx::Error> {
    db.write(|tx| {
        Box::pin(async move {
            sqlx::query!("DELETE FROM hub_sync_queue")
                .execute(&mut **tx)
                .await?;
            Ok(())
        })
    })
    .await
}

/// The User `user_id`'s `change`, as the Hub takes it, or `None` if none of its ids fits or the Hub
/// wouldn't take when it was made.
fn watch_state_to_proto(user_id: i64, change: &WatchStateChange) -> Option<WatchStateProto> {
    let ids: Vec<_> = (change.ids.iter())
        .filter(|id| hub::takes_external_id(id))
        .take(hub::WATCH_STATE_IDS)
        .cloned()
        .collect();
    (!ids.is_empty() && hub::takes_changed_at(change.changed_at_ms)).then(|| WatchStateProto {
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
pub(super) fn watch_state_from_proto(change: WatchStateProto) -> WatchStateChange {
    WatchStateChange {
        episode: change.episode,
        ids: change.ids,
        played: change.played,
        position_ms: change.position_ms,
        duration_ms: change.duration_ms,
        changed_at_ms: change.changed_at_ms,
    }
}

/// The User `user_id`'s `preference`, as the Hub takes it: a Show's or Movie's once under each id
/// that fits, and none if its language is longer than the Hub takes or the Hub wouldn't take when
/// it was made. A Track's title is cut short there instead, as it only ranks Tracks that match.
fn preference_to_proto(user_id: i64, preference: &TrackPreference) -> Vec<PreferenceProto> {
    let (title_kind, title_ids) = match &preference.kept_for {
        TrackPreferenceFor::User => (TitleKind::TITLE_KIND_UNSPECIFIED, None),
        TrackPreferenceFor::Movie(ids) => (TitleKind::TITLE_KIND_MOVIE, Some(ids)),
        TrackPreferenceFor::Show(ids) => (TitleKind::TITLE_KIND_SHOW, Some(ids)),
    };
    let title_ids: Vec<&str> = match title_ids {
        None => vec![""],
        Some(ids) => (ids.iter().map(String::as_str))
            .filter(|id| hub::takes_external_id(id))
            .collect(),
    };
    let like = preference.like.clone().unwrap_or_default();
    let language = like.language.unwrap_or_default();
    if language.chars().count() > hub::TRACK_LANGUAGE_CHARS
        || !hub::takes_changed_at(preference.changed_at_ms)
    {
        return Vec::new();
    }
    let change = PreferenceProto {
        user_id,
        audio: preference.audio,
        title_kind: title_kind.into(),
        off: preference.like.is_none(),
        language,
        forced: like.forced,
        sdh: like.sdh,
        title: (like.title.unwrap_or_default().chars())
            .take(hub::TRACK_TITLE_CHARS)
            .collect(),
        foreign_audio_only: preference.foreign_audio_only,
        changed_at_ms: preference.changed_at_ms,
        ..Default::default()
    };
    (title_ids.into_iter())
        .map(|title_id| PreferenceProto {
            title_id: title_id.to_owned(),
            ..change.clone()
        })
        .collect()
}

/// A Preference the Hub answered, as the Kernel takes it; `None` for one kept for a kind of title
/// the Server doesn't know.
pub(super) fn preference_from_proto(change: PreferenceProto) -> Option<TrackPreference> {
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

/// `watchlist`, as the Hub takes it: the entries it would take, newest first.
// ponytail: past the most entries the oldest are left out; page the sync if Watchlists grow that
// long.
pub(super) fn watchlist_to_proto(mut watchlist: Vec<WatchlistEntry>) -> Vec<EntryProto> {
    watchlist.sort_by_key(|entry| Reverse(entry.changed_at_ms));
    (watchlist.into_iter())
        .filter(|entry| {
            hub::takes_external_id(&entry.id) && hub::takes_changed_at(entry.changed_at_ms)
        })
        .take(hub::WATCHLIST_ENTRIES)
        .map(|entry| {
            let kind = if entry.show {
                TitleKind::TITLE_KIND_SHOW
            } else {
                TitleKind::TITLE_KIND_MOVIE
            };
            EntryProto {
                kind: kind.into(),
                id: entry.id,
                removed: entry.removed,
                changed_at_ms: entry.changed_at_ms,
                ..Default::default()
            }
        })
        .collect()
}

/// An entry the Hub answered, as the Kernel takes it, or `None` for a kind it doesn't know.
pub(super) fn entry_from_proto(entry: EntryProto) -> Option<WatchlistEntry> {
    let show = match entry.kind.as_known()? {
        TitleKind::TITLE_KIND_MOVIE => false,
        TitleKind::TITLE_KIND_SHOW => true,
        TitleKind::TITLE_KIND_UNSPECIFIED => return None,
    };
    Some(WatchlistEntry {
        show,
        id: entry.id,
        removed: entry.removed,
        changed_at_ms: entry.changed_at_ms,
    })
}

/// `viewed` as the Hub takes it, or `None` if it wouldn't take one of its ids or its duration.
fn viewing_event_to_proto(viewed: &ViewingEvent) -> Option<ViewingEventProto> {
    let ids = std::iter::once(&viewed.id).chain(&viewed.show_id);
    if !ids.into_iter().all(|id| hub::takes_external_id(id)) || viewed.duration_ms <= 0 {
        return None;
    }
    let stop_position_ms = viewed.stop_position_ms.clamp(0, viewed.duration_ms);
    Some(ViewingEventProto {
        episode: viewed.show_id.is_some(),
        external_id: viewed.id.clone(),
        show_external_id: viewed.show_id.clone().unwrap_or_default(),
        started_at: viewed.started_at,
        time_of_day_minutes: viewed.time_of_day_minutes,
        stop_position_ms,
        duration_ms: viewed.duration_ms,
        // As the Server marks a title played.
        finished: i128::from(stop_position_ms) * 10 > i128::from(viewed.duration_ms) * 9,
        device_kind: hub_device_kind(viewed.device_kind).into(),
        ..Default::default()
    })
}

fn hub_device_kind(kind: DeviceKind) -> HubDeviceKind {
    match kind {
        DeviceKind::DEVICE_KIND_COMPUTER => HubDeviceKind::DEVICE_KIND_COMPUTER,
        DeviceKind::DEVICE_KIND_PHONE => HubDeviceKind::DEVICE_KIND_PHONE,
        DeviceKind::DEVICE_KIND_TABLET => HubDeviceKind::DEVICE_KIND_TABLET,
        DeviceKind::DEVICE_KIND_TV => HubDeviceKind::DEVICE_KIND_TV,
        DeviceKind::DEVICE_KIND_UNSPECIFIED => HubDeviceKind::DEVICE_KIND_UNSPECIFIED,
    }
}

/// A key naming a Viewing Event apart from every other, so the Hub records one sent twice once.
fn random_key() -> Result<String, getrandom::Error> {
    let mut key = [0; 16];
    getrandom::fill(&mut key)?;
    Ok(URL_SAFE_NO_PAD.encode(key))
}

#[cfg(test)]
#[path = "sync_tests.rs"]
mod tests;
