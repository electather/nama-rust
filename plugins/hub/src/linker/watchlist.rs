//! Syncing Linked Users' Watchlists with the Hub (ADR 0056): the Kernel hands over a User's whole
//! Watchlist, removals included, which the Hub merges into their Account's and answers, for the
//! Kernel to take what is newer than its own. Nothing is queued: what the Hub didn't get goes with
//! the next sync.

use std::cmp::Reverse;

use nama_hub_proto::nama::hub::v1::{TitleKind, WatchlistEntry as Entry};
use nama_plugin_api::{PluginError, server_link::WatchlistEntry};

use super::Linker;
use crate::hub::limits;

impl Linker {
    /// Merges `watchlist`, the User `user_id`'s, into the Account they are Linked to, and answers
    /// the Account's; empty while they or the Server aren't Linked, when it doesn't leave the
    /// Server.
    pub(super) async fn merge_watchlist(
        &self,
        user_id: i64,
        mut watchlist: Vec<WatchlistEntry>,
    ) -> Result<Vec<WatchlistEntry>, PluginError> {
        if !self.user_linked(user_id).await? {
            return Ok(Vec::new());
        }
        watchlist.sort_by_key(|entry| Reverse(entry.changed_at_ms));
        // Past what the Hub takes it would refuse every sync of that User.
        // ponytail: past the most entries the oldest are left out; page the sync if Watchlists
        // grow that long.
        let entries = (watchlist.iter())
            .filter(|entry| {
                limits::takes_external_id(&entry.id)
                    && limits::takes_changed_at(entry.changed_at_ms)
            })
            .take(limits::WATCHLIST_ENTRIES)
            .map(entry_to_proto)
            .collect();
        let synced = self
            .call(|hub| hub.sync_watchlist(user_id, entries))
            .await?;
        Ok((synced.unwrap_or_default().into_iter())
            .filter_map(entry_from_proto)
            .collect())
    }
}

/// `entry`, as the Hub takes it.
fn entry_to_proto(entry: &WatchlistEntry) -> Entry {
    let kind = if entry.show {
        TitleKind::TITLE_KIND_SHOW
    } else {
        TitleKind::TITLE_KIND_MOVIE
    };
    Entry {
        kind: kind.into(),
        id: entry.id.clone(),
        removed: entry.removed,
        changed_at_ms: entry.changed_at_ms,
        ..Default::default()
    }
}

/// An entry the Hub answered, as the Kernel takes it, or `None` for a kind it doesn't know.
fn entry_from_proto(entry: Entry) -> Option<WatchlistEntry> {
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
