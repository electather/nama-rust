//! Sync (ADR 0008): each Account's merged Watch State, which its Linked Users' Servers push their
//! changes into and pull from.

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

    /// The Watch State of the Account `server`'s User `user_id` is Linked to, each title's under
    /// each of its ids apart, as `user_id`'s changes; empty if they aren't Linked. A Server merges
    /// a title's ids itself, as only it knows which ids its titles have.
    // ponytail: one answer holds the Account's whole Watch State; page it once one nears the
    // largest message a client takes.
    pub(crate) async fn pull(
        &self,
        server: &LinkedServer,
        user_id: i64,
    ) -> Result<Vec<WatchStateChange>, sqlx::Error> {
        let Some(account) = self.links.linked_account(server, user_id).await? else {
            return Ok(Vec::new());
        };
        let rows = sqlx::query!(
            r#"SELECT episode, title_id, played, position_ms, duration_ms,
                      (extract(epoch FROM changed_at) * 1000)::bigint AS "changed_at_ms!"
               FROM watch_states WHERE account_id = $1"#,
            account.0,
        )
        .fetch_all(&self.db)
        .await?;
        Ok((rows.into_iter())
            .map(|row| WatchStateChange {
                user_id,
                episode: row.episode,
                ids: vec![row.title_id],
                played: row.played,
                position_ms: row.position_ms,
                duration_ms: row.duration_ms,
                changed_at_ms: row.changed_at_ms,
            })
            .collect())
    }
}

/// Keeps `change` as `account`'s Watch State under each of its ids, unless a later change is kept
/// there. Of two made at the same moment, the one pushed last wins, as a Server pushes its own
/// changes in order.
// ponytail: each id's row merges alone, as the Hub learns a title's ids only from the changes
// naming them. A Server knowing `tmdb-348` alone can leave `imdb-tt0078748`'s row behind;
// keep each row's sibling ids and merge across them if Servers' Matches start to differ.
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
