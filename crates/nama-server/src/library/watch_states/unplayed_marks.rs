//! When each User last marked a Movie or Episode unplayed, under each of its ids, in
//! `forgotten_watch_states`. Marking unplayed deletes its Watch State rows, so this is all that
//! keeps a change made elsewhere before it, pulled or sent offline, from bringing it back
//! (#2324). The title's next stored row is newer, so storing one drops the mark.

use sqlx::SqliteConnection;

/// Keeps `at`, in seconds since the Unix epoch, as when the title under every id in the JSON array
/// `ids` was last marked unplayed, unless a later mark is kept.
pub(super) async fn record(
    conn: &mut SqliteConnection,
    user: i64,
    kind: &'static str,
    ids: &str,
    at: i64,
) -> Result<(), sqlx::Error> {
    // `WHERE true` keeps SQLite from reading the upsert's `ON` as a join's.
    sqlx::query!(
        "INSERT INTO forgotten_watch_states (user_id, kind, title_id, forgotten_at)
         SELECT ?1, ?2, value, ?4 FROM json_each(?3) WHERE true
         ON CONFLICT (user_id, kind, title_id) DO UPDATE
         SET forgotten_at = max(forgotten_at, excluded.forgotten_at)",
        user,
        kind,
        ids,
        at,
    )
    .execute(conn)
    .await
    .map(drop)
}

/// Drops the title's marks under every id in the JSON array `ids`.
pub(super) async fn clear(
    conn: &mut SqliteConnection,
    user: i64,
    kind: &'static str,
    ids: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "DELETE FROM forgotten_watch_states
         WHERE user_id = ? AND kind = ? AND title_id IN (SELECT value FROM json_each(?))",
        user,
        kind,
        ids,
    )
    .execute(conn)
    .await
    .map(drop)
}

#[cfg(test)]
#[path = "unplayed_marks_tests.rs"]
mod tests;
