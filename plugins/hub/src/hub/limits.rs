//! What the Hub accepts, as the Hub API documents it (`proto/nama/hub/v1/`): past any of these the
//! Hub refuses the whole call. Who reads them is in `plugins/AGENTS.md`.

use std::ops::RangeInclusive;

/// The most Viewing Events a `RecordViewingEventsRequest` holds.
pub(crate) const VIEWING_EVENTS: usize = 1000;

/// The characters an External ID has, such as `tmdb-348`.
const EXTERNAL_ID_CHARS: RangeInclusive<usize> = 1..=100;

/// Whether the Hub takes `id` as an External ID.
pub(crate) fn takes_external_id(id: &str) -> bool {
    EXTERNAL_ID_CHARS.contains(&id.chars().count())
}
