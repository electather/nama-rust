//! What the Hub accepts, as the Hub API documents it (`proto/nama/hub/v1/`): past any of these the
//! Hub refuses the whole call. Who reads them is in `plugins/AGENTS.md`.

use std::ops::RangeInclusive;

/// The most External IDs a `WatchStateChange` carries.
pub(crate) const WATCH_STATE_IDS: usize = 20;

/// The most changes a `PushWatchStatesRequest` or a `PushTrackPreferencesRequest` holds.
pub(crate) const PUSH_CHANGES: usize = 500;

/// The most entries a `SyncWatchlistRequest` holds.
pub(crate) const WATCHLIST_ENTRIES: usize = 5000;

/// The most characters a `TrackPreferenceChange.language` has.
pub(crate) const TRACK_LANGUAGE_CHARS: usize = 100;

/// The most characters a `TrackPreferenceChange.title` has.
pub(crate) const TRACK_TITLE_CHARS: usize = 1000;

/// The most Viewing Events a `RecordViewingEventsRequest` holds.
pub(crate) const VIEWING_EVENTS: usize = 1000;

/// The characters an External ID has, such as `tmdb-348`.
const EXTERNAL_ID_CHARS: RangeInclusive<usize> = 1..=100;

/// When a change may say it was made, in milliseconds since the Unix epoch: from 1970 to the end of
/// 9999.
const CHANGED_AT_MS: RangeInclusive<i64> = 0..=253_402_300_799_999;

/// Whether the Hub takes `id` as an External ID.
pub(crate) fn takes_external_id(id: &str) -> bool {
    EXTERNAL_ID_CHARS.contains(&id.chars().count())
}

/// Whether the Hub takes `changed_at_ms` as when a change was made.
pub(crate) fn takes_changed_at(changed_at_ms: i64) -> bool {
    CHANGED_AT_MS.contains(&changed_at_ms)
}
