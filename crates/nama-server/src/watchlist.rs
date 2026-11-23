//! The Watchlist (ADR 0056): the Movies and Shows each User means to watch, kept by kind and
//! external id whether or not the Server has them. Only this file reads or writes `watchlist`
//! (`AGENTS.md`). A title taken off stays as a removal, with when each change was made, so the
//! enabled Plugin that links the Server syncs it across a Linked User's Servers, the change made
//! last winning.

use nama_plugin_api::{
    metadata::{ExternalId, ExternalIds},
    server_link::WatchlistEntry,
};
use sqlx::SqliteConnection;

use crate::{
    db::{self, Database},
    library::{TitleKind, matched_ids},
};

/// Puts the title of `kind` with the id `id` on the User `user`'s Watchlist, if it isn't already.
pub(crate) async fn add(
    db: &Database,
    user: i64,
    kind: TitleKind,
    id: &ExternalId,
) -> Result<(), sqlx::Error> {
    change(db, user, kind, id, false).await
}

/// Takes the title of `kind` with the id `id` off the User `user`'s Watchlist, under every id the
/// Server's Matches give it, since [`has`] finds it under any of them.
pub(crate) async fn remove(
    db: &Database,
    user: i64,
    kind: TitleKind,
    id: &ExternalId,
) -> Result<(), sqlx::Error> {
    change(db, user, kind, id, true).await
}

/// Puts the title of `kind` with the id `id` on the User `user`'s Watchlist, or takes it off, now.
async fn change(
    db: &Database,
    user: i64,
    kind: TitleKind,
    id: &ExternalId,
    removed: bool,
) -> Result<(), sqlx::Error> {
    let entry = WatchlistEntry {
        show: kind == TitleKind::Show,
        id: id.to_string(),
        removed,
        changed_at_ms: db::now_ms(),
    };
    let id = id.clone();
    db.write(move |tx| {
        Box::pin(async move {
            let ids = json_ids(matched_ids(tx, kind, &id).await?);
            set(tx, user, &entry, &ids).await
        })
    })
    .await
}

/// The User `user`'s whole Watchlist, as the enabled Plugin that links the Server syncs it: each
/// title by the id it was added under, those taken off included.
pub(crate) async fn entries(db: &Database, user: i64) -> Result<Vec<WatchlistEntry>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"SELECT kind, title_id, removed AS "removed: bool", changed_at_ms
           FROM watchlist WHERE user_id = ?"#,
        user
    )
    .fetch_all(db.reader())
    .await?;
    Ok((rows.into_iter())
        .filter_map(|row| {
            Some(WatchlistEntry {
                show: kind_of(&row.kind)? == TitleKind::Show,
                id: row.title_id,
                removed: row.removed,
                changed_at_ms: row.changed_at_ms,
            })
        })
        .collect())
}

/// Takes each of `synced`, the User `user`'s Watchlist as the enabled Plugin that links the Server
/// answered it, that changed after anything the Server has of its title.
pub(crate) async fn take_synced(
    db: &Database,
    user: i64,
    mut synced: Vec<WatchlistEntry>,
) -> Result<(), sqlx::Error> {
    // Oldest first, so of one title's entries under different ids, the latest is taken last, and
    // of two made at once, the removal, as `take` breaks the tie.
    synced.sort_by_key(|entry| (entry.changed_at_ms, entry.removed));
    db.write(move |tx| {
        Box::pin(async move {
            for entry in &synced {
                take(tx, user, entry).await?;
            }
            Ok(())
        })
    })
    .await
}

/// Takes `entry` into the User `user`'s Watchlist if it changed after anything the Server has of
/// its title under any id the Server's Matches give it, or at the same moment and is a removal.
/// One whose id doesn't parse is skipped.
async fn take(
    conn: &mut SqliteConnection,
    user: i64,
    entry: &WatchlistEntry,
) -> Result<(), sqlx::Error> {
    let Ok(id) = entry.id.parse::<ExternalId>() else {
        return Ok(());
    };
    let kind = title_kind(entry);
    let ids = json_ids(matched_ids(conn, kind, &id).await?);
    let stored_kind = stored(kind);
    let latest = sqlx::query_scalar!(
        "SELECT max(changed_at_ms) FROM watchlist
         WHERE user_id = ? AND kind = ? AND title_id IN (SELECT value FROM json_each(?))",
        user,
        stored_kind,
        ids
    )
    .fetch_one(&mut *conn)
    .await?;
    // A removal wins a tie, here and on the Hub, or two Servers could each keep their own.
    let newer_kept = latest.is_some_and(|latest| {
        latest > entry.changed_at_ms || (latest == entry.changed_at_ms && !entry.removed)
    });
    if newer_kept {
        return Ok(());
    }
    set(conn, user, entry, &ids).await
}

/// Takes `entry` as the User `user`'s latest change of its title, known by `ids`, the JSON array
/// of its ids: on, under `entry`'s id unless it's on under another, or taken off under every one.
async fn set(
    conn: &mut SqliteConnection,
    user: i64,
    entry: &WatchlistEntry,
    ids: &str,
) -> Result<(), sqlx::Error> {
    let (kind, changed_at_ms) = (stored(title_kind(entry)), entry.changed_at_ms);
    if entry.removed {
        // Kept under every id, even one it was never on under, so the removal syncs under each: a
        // Server lacking the title can't tell they name one title, and may have it on under any.
        sqlx::query!(
            "INSERT INTO watchlist (user_id, kind, title_id, removed, changed_at_ms)
             SELECT ?, ?, value, 1, ? FROM json_each(?) WHERE true
             ON CONFLICT (user_id, kind, title_id) DO UPDATE
             SET removed = 1, changed_at_ms = excluded.changed_at_ms",
            user,
            kind,
            changed_at_ms,
            ids
        )
        .execute(&mut *conn)
        .await?;
        return Ok(());
    }
    // Deleted, not put back, so the title lists as newly added.
    sqlx::query!(
        "DELETE FROM watchlist
         WHERE user_id = ? AND kind = ? AND removed AND title_id IN (SELECT value FROM json_each(?))",
        user,
        kind,
        ids
    )
    .execute(&mut *conn)
    .await?;
    let on = sqlx::query!(
        "UPDATE watchlist SET changed_at_ms = ?
         WHERE user_id = ? AND kind = ? AND title_id IN (SELECT value FROM json_each(?))",
        changed_at_ms,
        user,
        kind,
        ids
    )
    .execute(&mut *conn)
    .await?;
    if on.rows_affected() == 0 {
        sqlx::query!(
            "INSERT INTO watchlist (user_id, kind, title_id, changed_at_ms) VALUES (?, ?, ?, ?)",
            user,
            kind,
            entry.id,
            changed_at_ms
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

/// The kind of `entry`'s title.
const fn title_kind(entry: &WatchlistEntry) -> TitleKind {
    if entry.show {
        TitleKind::Show
    } else {
        TitleKind::Movie
    }
}

/// Whether the title of `kind` known by `ids` is on the User `user`'s Watchlist under any of them.
pub(crate) async fn has(
    db: &Database,
    user: i64,
    kind: TitleKind,
    ids: &ExternalIds,
) -> Result<bool, sqlx::Error> {
    let (kind, ids) = (stored(kind), json_ids(ids.iter()));
    sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM watchlist
                          WHERE user_id = ? AND kind = ? AND NOT removed
                            AND title_id IN (SELECT value FROM json_each(?))) AS "on: bool""#,
        user,
        kind,
        ids
    )
    .fetch_one(db.reader())
    .await
}

/// Whether the title of `kind` with the id `id` is on the User `user`'s Watchlist, under `id` or
/// any other id the Server's Matches give it: just the ids [`remove`] clears, so a title this
/// finds, `remove` with `id` takes off.
pub(crate) async fn has_matched(
    db: &Database,
    user: i64,
    kind: TitleKind,
    id: &ExternalId,
) -> Result<bool, sqlx::Error> {
    let ids = {
        let mut conn = db.reader().acquire().await?;
        ExternalIds::from_iter(matched_ids(&mut conn, kind, id).await?)
    };
    has(db, user, kind, &ids).await
}

/// `ids` as the JSON array of their text form that `json_each` reads.
fn json_ids(ids: impl IntoIterator<Item = ExternalId>) -> String {
    let ids: Vec<String> = ids.into_iter().map(|id| id.to_string()).collect();
    serde_json::Value::from(ids).to_string()
}

/// The User `user`'s Watchlist, newest added first, each title by the id it was added under. A
/// row whose kind or id doesn't parse is skipped.
pub(crate) async fn list(
    db: &Database,
    user: i64,
) -> Result<Vec<(TitleKind, ExternalId)>, sqlx::Error> {
    // A new row's rowid is one past the largest left, so it orders by when each was added. A
    // Backup's VACUUM INTO may renumber rowids, but copies the rows in rowid order; a migration
    // rebuilding the table must copy them `ORDER BY rowid` too, or the order is lost.
    let rows = sqlx::query!(
        "SELECT kind, title_id FROM watchlist WHERE user_id = ? AND NOT removed ORDER BY rowid DESC",
        user
    )
    .fetch_all(db.reader())
    .await?;
    Ok((rows.into_iter())
        .filter_map(|row| Some((kind_of(&row.kind)?, row.title_id.parse().ok()?)))
        .collect())
}

/// `kind` as `watchlist` stores it.
const fn stored(kind: TitleKind) -> &'static str {
    match kind {
        TitleKind::Movie => "movie",
        TitleKind::Show => "show",
    }
}

/// The kind `watchlist` stores as `stored`, undoing [`stored`].
fn kind_of(stored: &str) -> Option<TitleKind> {
    match stored {
        "movie" => Some(TitleKind::Movie),
        "show" => Some(TitleKind::Show),
        _ => None,
    }
}

#[cfg(test)]
#[path = "watchlist_tests.rs"]
mod tests;
