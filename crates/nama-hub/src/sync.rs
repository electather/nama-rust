//! Sync (ADR 0008): each Account's merged Watch State, which its Linked Users' Servers push their
//! changes into and pull from, and its merged Watchlist (ADR 0056), which they sync whole.

use std::collections::{HashMap, hash_map::Entry};

use sqlx::{PgConnection, PgPool};

use crate::{
    accounts::AccountId,
    links::{LinkedServer, Links},
    viewing::{MIN_POOLED_ACCOUNTS, Viewing},
};

/// The most changes one push may hold.
pub(crate) const MAX_CHANGES: usize = 500;

/// The most External IDs one change may name.
pub(crate) const MAX_IDS: usize = 20;

/// The most characters one External ID may have.
pub(crate) const MAX_ID_LENGTH: usize = 100;

/// The latest time a change may be made at, in milliseconds since the Unix epoch: the end of 9999.
pub(crate) const MAX_CHANGED_AT_MS: i64 = 253_402_300_799_999;

/// The most entries one Watchlist sync may hold.
pub(crate) const MAX_WATCHLIST_ENTRIES: usize = 5000;

/// The most titles [`Watchlists::most_watchlisted`] answers: as many as a Home row holds (ADR 0058).
const MAX_WATCHLISTED: i64 = 200;

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

/// A title on a User's Watchlist, or taken off it, as the change made last left it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WatchlistEntry {
    /// Whether it is a Show rather than a Movie, whose TMDB ids are a separate id space.
    pub(crate) show: bool,
    /// The External ID it was added under, such as `tmdb-603`.
    pub(crate) id: String,
    pub(crate) removed: bool,
    /// When the Server made the change, in milliseconds since the Unix epoch.
    pub(crate) changed_at_ms: i64,
}

/// A title on many Accounts' Watchlists.
#[derive(Debug)]
pub(crate) struct WatchlistedTitle {
    /// Whether it is a Show rather than a Movie.
    pub(crate) show: bool,
    /// Such as `tmdb-603`.
    pub(crate) external_id: String,
    /// How watchlisted it is, from 0.0 to 1.0: its Accounts against the most watchlisted's.
    pub(crate) weight: f32,
}

/// The Accounts' merged Watchlists.
#[derive(Clone)]
pub(crate) struct Watchlists {
    db: PgPool,
    links: Links,
    viewing: Viewing,
}

impl Watchlists {
    /// The merged Watchlists in `db`, of the Users `links` has Linked, counted across Accounts
    /// only as `viewing` lets them pool.
    pub(crate) fn new(db: PgPool, links: Links, viewing: Viewing) -> Self {
        Self { db, links, viewing }
    }

    /// Merges `entries`, `server`'s User `user_id`'s whole Watchlist, into the Account they are
    /// Linked to, and returns the Account's, each title's under each of its ids apart; empty, and
    /// nothing kept, if they aren't Linked. Per title and id, the change made last wins; of two
    /// made at the same moment, the removal, as the Server breaks the tie the same way.
    // ponytail: one call holds a User's whole Watchlist, removals included, both ways; page it, or
    // send only what changed since the last sync, once one nears `MAX_WATCHLIST_ENTRIES`.
    pub(crate) async fn sync(
        &self,
        server: &LinkedServer,
        user_id: i64,
        entries: Vec<WatchlistEntry>,
    ) -> Result<Vec<WatchlistEntry>, sqlx::Error> {
        let Some(account) = self.links.linked_account(server, user_id).await? else {
            return Ok(Vec::new());
        };
        let mut tx = self.db.begin().await?;
        merge_watchlist(&mut tx, account, entries).await?;
        let rows = sqlx::query!(
            r#"SELECT show, title_id, removed,
                      (extract(epoch FROM changed_at) * 1000)::bigint AS "changed_at_ms!"
               FROM watchlists WHERE account_id = $1"#,
            account.0,
        )
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok((rows.into_iter())
            .map(|row| WatchlistEntry {
                show: row.show,
                id: row.title_id,
                removed: row.removed,
                changed_at_ms: row.changed_at_ms,
            })
            .collect())
    }

    /// The Movies and Shows on most Accounts' Watchlists, at most [`MAX_WATCHLISTED`], most
    /// watchlisted first, by the pooled counts' rules (ADR 0093): a title is read only once
    /// [`MIN_POOLED_ACCOUNTS`] Accounts have it, and an Account that turned pooling off isn't
    /// counted.
    // ponytail: counts every Watchlist on each call, which each Linked Server makes about once a
    // day; keep counts as Watchlists change once that's slow.
    pub(crate) async fn most_watchlisted(&self) -> Result<Vec<WatchlistedTitle>, sqlx::Error> {
        let not_pooling: Vec<_> = (self.viewing.not_pooling().await?)
            .into_iter()
            .map(|account| account.0)
            .collect();
        // An Account keeps one row per title and id, so each counts once.
        sqlx::query_as!(
            WatchlistedTitle,
            r#"SELECT show, title_id AS external_id,
             (count(*)::float8 / max(count(*)) OVER ())::real AS "weight!"
             FROM watchlists WHERE NOT removed AND account_id <> ALL($1)
             GROUP BY show, title_id HAVING count(*) >= $2
             ORDER BY count(*) DESC, show, title_id LIMIT $3"#,
            &not_pooling,
            MIN_POOLED_ACCOUNTS,
            MAX_WATCHLISTED,
        )
        .fetch_all(&self.db)
        .await
    }
}

/// Merges `entries` into `account`'s Watchlist. Per title and id, the change made last wins; of
/// two made at the same moment, the removal.
async fn merge_watchlist(
    conn: &mut PgConnection,
    account: AccountId,
    entries: Vec<WatchlistEntry>,
) -> Result<(), sqlx::Error> {
    let (mut shows, mut ids, mut removed, mut changed_at_ms) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for entry in entries {
        shows.push(entry.show);
        ids.push(entry.id);
        removed.push(entry.removed);
        changed_at_ms.push(entry.changed_at_ms);
    }
    // DISTINCT ON, as an upsert may touch each row once: of one title's entries, the latest.
    // A removal wins a tie, here and on the Server, or two Servers could each keep their own.
    sqlx::query!(
        "INSERT INTO watchlists (account_id, show, title_id, removed, changed_at)
         SELECT DISTINCT ON (show, title_id) $1::uuid, show, title_id, removed,
         'epoch'::timestamptz + changed_at_ms * interval '1 millisecond'
         FROM unnest($2::boolean[], $3::text[], $4::boolean[], $5::bigint[])
         AS entries (show, title_id, removed, changed_at_ms)
         ORDER BY show, title_id, changed_at_ms DESC, removed DESC
         ON CONFLICT (account_id, show, title_id) DO UPDATE
         SET removed = excluded.removed, changed_at = excluded.changed_at
         WHERE (excluded.changed_at, excluded.removed)
             > (watchlists.changed_at, watchlists.removed)",
        account.0,
        &shows,
        &ids,
        &removed,
        &changed_at_ms,
    )
    .execute(conn)
    .await?;
    Ok(())
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
