//! What the one Account a fake Hub syncs every User with keeps.

use axum::http::HeaderMap;
use connectrpc::ConnectError;
use nama_hub_proto::nama::hub::v1::{
    PullTrackPreferencesRequest, PullTrackPreferencesResponse, PullWatchStatesRequest,
    PullWatchStatesResponse, PushTrackPreferencesRequest, PushTrackPreferencesResponse,
    PushWatchStatesRequest, PushWatchStatesResponse, RecordViewingEventsRequest,
    RecordViewingEventsResponse, SyncWatchlistRequest, SyncWatchlistResponse,
    TrackPreferenceChange, UserViewingEvent, WatchStateChange, WatchlistEntry,
};
use ring::signature::Ed25519KeyPair;

use super::{State, caller, lock};

/// What the one Account the fake syncs every User with keeps: of each title and Preference, the
/// change made last, as the Hub does.
#[derive(Default)]
pub(crate) struct Account {
    pub(crate) watch_states: Vec<WatchStateChange>,
    pub(crate) preferences: Vec<TrackPreferenceChange>,
    pub(crate) watchlist: Vec<WatchlistEntry>,
    /// Each Viewing Event the Server recorded, in the order they came.
    pub(crate) viewing_events: Vec<UserViewingEvent>,
    /// Each Watch State push (`"push"`) and pull (`"pull"`), in the order they came.
    pub(crate) asked: Vec<&'static str>,
}

impl Account {
    /// Keeps `change` unless one made later of the same title is kept.
    pub(crate) fn keep_watch_state(&mut self, change: WatchStateChange) {
        let title = |kept: &WatchStateChange| (kept.episode, kept.ids.clone());
        keep_latest(&mut self.watch_states, change, title, |kept| {
            kept.changed_at_ms
        });
    }

    /// Keeps `change` unless one made later of the same Preference is kept.
    pub(crate) fn keep_preference(&mut self, change: TrackPreferenceChange) {
        let preference =
            |kept: &TrackPreferenceChange| (kept.audio, kept.title_kind, kept.title_id.clone());
        keep_latest(&mut self.preferences, change, preference, |kept| {
            kept.changed_at_ms
        });
    }

    /// Keeps `entry` unless one made later of the same title is kept.
    pub(crate) fn keep_entry(&mut self, entry: WatchlistEntry) {
        let title = |kept: &WatchlistEntry| (kept.kind, kept.id.clone());
        keep_latest(&mut self.watchlist, entry, title, |kept| kept.changed_at_ms);
    }
}

/// Puts `item` in `kept` in place of the one of the same `key`, unless that one was made later.
fn keep_latest<T, K: PartialEq>(
    kept: &mut Vec<T>,
    item: T,
    key: impl Fn(&T) -> K,
    at: impl Fn(&T) -> i64,
) {
    match kept.iter_mut().find(|old| key(old) == key(&item)) {
        Some(old) if at(old) > at(&item) => {}
        Some(old) => *old = item,
        None => kept.push(item),
    }
}

pub(super) fn push_watch_states(
    state: &mut State,
    headers: &HeaderMap,
    request: &PushWatchStatesRequest,
    _: &Ed25519KeyPair,
) -> Result<PushWatchStatesResponse, ConnectError> {
    caller(state, headers)?;
    let mut account = lock(&state.account);
    account.asked.push("push");
    for change in &request.changes {
        account.keep_watch_state(change.clone());
    }
    Ok(PushWatchStatesResponse::default())
}

pub(super) fn pull_watch_states(
    state: &mut State,
    headers: &HeaderMap,
    _: &PullWatchStatesRequest,
    _: &Ed25519KeyPair,
) -> Result<PullWatchStatesResponse, ConnectError> {
    caller(state, headers)?;
    let mut account = lock(&state.account);
    account.asked.push("pull");
    Ok(PullWatchStatesResponse {
        watch_states: account.watch_states.clone(),
        ..Default::default()
    })
}

pub(super) fn push_track_preferences(
    state: &mut State,
    headers: &HeaderMap,
    request: &PushTrackPreferencesRequest,
    _: &Ed25519KeyPair,
) -> Result<PushTrackPreferencesResponse, ConnectError> {
    caller(state, headers)?;
    let mut account = lock(&state.account);
    for change in &request.changes {
        account.keep_preference(change.clone());
    }
    Ok(PushTrackPreferencesResponse::default())
}

pub(super) fn pull_track_preferences(
    state: &mut State,
    headers: &HeaderMap,
    _: &PullTrackPreferencesRequest,
    _: &Ed25519KeyPair,
) -> Result<PullTrackPreferencesResponse, ConnectError> {
    caller(state, headers)?;
    Ok(PullTrackPreferencesResponse {
        preferences: lock(&state.account).preferences.clone(),
        ..Default::default()
    })
}

pub(super) fn sync_watchlist(
    state: &mut State,
    headers: &HeaderMap,
    request: &SyncWatchlistRequest,
    _: &Ed25519KeyPair,
) -> Result<SyncWatchlistResponse, ConnectError> {
    caller(state, headers)?;
    let mut account = lock(&state.account);
    for entry in &request.entries {
        account.keep_entry(entry.clone());
    }
    Ok(SyncWatchlistResponse {
        entries: account.watchlist.clone(),
        ..Default::default()
    })
}

pub(super) fn record_viewing_events(
    state: &mut State,
    headers: &HeaderMap,
    request: &RecordViewingEventsRequest,
    _: &Ed25519KeyPair,
) -> Result<RecordViewingEventsResponse, ConnectError> {
    caller(state, headers)?;
    (lock(&state.account).viewing_events).extend(request.events.iter().cloned());
    Ok(RecordViewingEventsResponse::default())
}
