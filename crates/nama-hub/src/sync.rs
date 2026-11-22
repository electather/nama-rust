//! Sync (ADR 0008): each Account's merged Watch State, which its Linked Users' Servers push their
//! changes into.

use std::collections::{HashMap, hash_map::Entry};

use sqlx::{PgConnection, PgPool};

use crate::{
    accounts::AccountId,
    links::{LinkedServer, Links},
};

/// The most changes one push may hold.
pub(crate) const MAX_CHANGES: usize = 500;

/// The most External IDs one change may name.
pub(crate) const MAX_IDS: usize = 20;

/// The most characters one External ID may have.
pub(crate) const MAX_ID_LENGTH: usize = 100;

/// The latest time a change may be made at, in milliseconds since the Unix epoch: the end of 9999.
pub(crate) const MAX_CHANGED_AT_MS: i64 = 253_402_300_799_999;

/// One User's Watch State of a Movie or Episode, as a change on their Server left it.
#[derive(Debug, Clone)]
pub(crate) struct WatchStateChange {
    /// The User's id on their Server.
    pub(crate) user_id: i64,
    /// Whether it is an Episode rather than a Movie, whose TMDB ids are a separate id space.
    pub(crate) episode: bool,
    /// Every External ID it is known by, such as `tmdb-348`.
    pub(crate) ids: Vec<String>,
    /// Whether it was played; marking it unplayed leaves it unplayed at 0 of 0.
    pub(crate) played: bool,
    pub(crate) position_ms: i64,
    /// How long it is; 0 if no player said.
    pub(crate) duration_ms: i64,
    /// When the Server made it, in milliseconds since the Unix epoch.
    pub(crate) changed_at_ms: i64,
}

/// The Accounts' merged Watch State.
#[derive(Clone)]
pub(crate) struct WatchStates {
    db: PgPool,
    links: Links,
}

impl WatchStates {
    /// The merged Watch State in `db`, of the Users `links` has Linked.
    pub(crate) fn new(db: PgPool, links: Links) -> Self {
        Self { db, links }
    }

    /// Merges `changes`, made on `server`, into the Accounts its Users are Linked to, in one
    /// transaction. A change of a User who isn't Linked is dropped.
    pub(crate) async fn push(
        &self,
        server: &LinkedServer,
        changes: Vec<WatchStateChange>,
    ) -> Result<(), sqlx::Error> {
        // Looked up before the transaction, which would otherwise hold a connection while asking
        // the pool for another.
        let mut accounts = HashMap::new();
        for change in &changes {
            if let Entry::Vacant(unknown) = accounts.entry(change.user_id) {
                unknown.insert(self.links.linked_account(server, change.user_id).await?);
            }
        }
        let mut tx = self.db.begin().await?;
        for change in changes {
            if let Some(&Some(account)) = accounts.get(&change.user_id) {
                merge(&mut tx, account, &change).await?;
            }
        }
        tx.commit().await
    }
}

/// Keeps `change` as `account`'s Watch State under each of its ids, unless a later change is kept
/// there. Of two made at the same moment, the one pushed last wins, as a Server pushes its own
/// changes in order.
async fn merge(
    conn: &mut PgConnection,
    account: AccountId,
    change: &WatchStateChange,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO watch_states
         (account_id, episode, title_id, played, position_ms, duration_ms, changed_at)
         SELECT $1::uuid, $2::boolean, title_id, $4::boolean, $5::bigint, $6::bigint,
         'epoch'::timestamptz + $7::bigint * interval '1 millisecond'
         FROM (SELECT DISTINCT unnest($3::text[])) AS ids (title_id)
         ON CONFLICT (account_id, episode, title_id) DO UPDATE
         SET played = excluded.played, position_ms = excluded.position_ms,
         duration_ms = excluded.duration_ms, changed_at = excluded.changed_at
         WHERE excluded.changed_at >= watch_states.changed_at",
        account.0,
        change.episode,
        &change.ids,
        change.played,
        change.position_ms,
        change.duration_ms,
        change.changed_at_ms,
    )
    .execute(conn)
    .await?;
    Ok(())
}

#[cfg(test)]
#[path = "sync/sync_tests.rs"]
mod tests;
