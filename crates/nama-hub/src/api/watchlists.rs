//! The Watchlist's wire form: `LinkedServerService.SyncWatchlist`'s entries to and from the
//! Hub's, the refusal of one that breaks a limit, and `ListMostWatchlistedTitles`' titles.

use connectrpc::ConnectError;
use nama_hub_proto::nama::hub::v1::{self as proto, RefusalReason};

use super::refuse;
use crate::sync::{
    MAX_CHANGED_AT_MS, MAX_ID_LENGTH, MAX_WATCHLIST_ENTRIES, WatchlistEntry, WatchlistedTitle,
};

/// `entries` as `sync` takes them, or the refusal of the first limit they break.
pub(super) fn watchlist_from_proto(
    entries: Vec<proto::WatchlistEntry>,
) -> Result<Vec<WatchlistEntry>, ConnectError> {
    if entries.len() > MAX_WATCHLIST_ENTRIES {
        return Err(invalid_watchlist(
            "entries",
            format!("a Watchlist holds up to {MAX_WATCHLIST_ENTRIES} entries"),
        ));
    }
    (entries.into_iter().enumerate())
        .map(|(nth, entry)| watchlist_entry_from_proto(nth, entry))
        .collect()
}

/// The `nth` entry of a Watchlist, as `sync` takes it.
fn watchlist_entry_from_proto(
    nth: usize,
    entry: proto::WatchlistEntry,
) -> Result<WatchlistEntry, ConnectError> {
    let field = |name: &str| format!("entries[{nth}].{name}");
    let show = match entry.kind.as_known() {
        Some(proto::TitleKind::TITLE_KIND_MOVIE) => false,
        Some(proto::TitleKind::TITLE_KIND_SHOW) => true,
        Some(proto::TitleKind::TITLE_KIND_UNSPECIFIED) | None => {
            return Err(invalid_watchlist(
                &field("kind"),
                "an entry is a Movie or a Show",
            ));
        }
    };
    if !(1..=MAX_ID_LENGTH).contains(&entry.id.chars().count()) {
        return Err(invalid_watchlist(
            &field("id"),
            format!("an entry's External ID has 1 to {MAX_ID_LENGTH} characters"),
        ));
    }
    if !(0..=MAX_CHANGED_AT_MS).contains(&entry.changed_at_ms) {
        return Err(invalid_watchlist(
            &field("changed_at_ms"),
            "an entry is dated from 1970 to the end of 9999",
        ));
    }
    Ok(WatchlistEntry {
        show,
        id: entry.id,
        removed: entry.removed,
        changed_at_ms: entry.changed_at_ms,
    })
}

/// `entry`, as a Linked Server takes it.
pub(super) fn watchlist_entry_to_proto(entry: WatchlistEntry) -> proto::WatchlistEntry {
    proto::WatchlistEntry {
        kind: kind_to_proto(entry.show).into(),
        id: entry.id,
        removed: entry.removed,
        changed_at_ms: entry.changed_at_ms,
        ..Default::default()
    }
}

/// Refuses a Watchlist sync for an entry at `field` that breaks a limit, which `message` names.
fn invalid_watchlist(field: &str, message: impl Into<String>) -> ConnectError {
    refuse(RefusalReason::REFUSAL_REASON_WATCHLIST_INVALID, message)
        .field(field)
        .param("maxEntries", &MAX_WATCHLIST_ENTRIES)
        .param("maxIdLength", &MAX_ID_LENGTH)
        .into()
}

/// `watchlisted` as a Server reads it, or `None` for an External ID not written
/// `namespace-value`, which no Server sends.
pub(super) fn watchlisted_to_proto(
    watchlisted: &WatchlistedTitle,
) -> Option<proto::WatchlistedTitle> {
    let (namespace, value) = watchlisted.external_id.split_once('-')?;
    Some(proto::WatchlistedTitle {
        kind: kind_to_proto(watchlisted.show).into(),
        id: proto::ExternalId {
            namespace: namespace.to_owned(),
            value: value.to_owned(),
            ..Default::default()
        }
        .into(),
        weight: watchlisted.weight,
        ..Default::default()
    })
}

/// A Show's kind when `show`, else a Movie's.
const fn kind_to_proto(show: bool) -> proto::TitleKind {
    if show {
        proto::TitleKind::TITLE_KIND_SHOW
    } else {
        proto::TitleKind::TITLE_KIND_MOVIE
    }
}

#[cfg(test)]
#[path = "watchlists_tests.rs"]
mod tests;
