//! Each Library's last Scan that finished storing, kept on its row in `libraries`. Each one
//! replaces the last: no history is kept.

use std::collections::HashMap;

use sqlx::SqliteConnection;

/// What a Library's last finished Scan stored, and when it finished; its counts are those of
/// that Scan's `ScanFinished`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LastScan {
    /// Seconds since the Unix epoch.
    pub(crate) finished_at: i64,
    pub(crate) added: u64,
    pub(crate) updated: u64,
    pub(crate) removed: u64,
    pub(crate) unchanged: u64,
}

/// The last Scan of the Library `id`, or of every Library when `id` is `None`, by Library id. A
/// Library that has never finished one isn't in it.
pub(super) async fn read(
    conn: &mut SqliteConnection,
    id: Option<i64>,
) -> sqlx::Result<HashMap<i64, LastScan>> {
    let rows = sqlx::query!(
        r#"SELECT id AS "id!", last_scan_at AS "finished_at!",
                  last_scan_added AS "added: u64", last_scan_updated AS "updated: u64",
                  last_scan_removed AS "removed: u64", last_scan_unchanged AS "unchanged: u64"
           FROM libraries WHERE last_scan_at IS NOT NULL AND (?1 IS NULL OR id = ?1)"#,
        id,
    )
    .fetch_all(conn)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            let last = LastScan {
                finished_at: row.finished_at,
                added: row.added,
                updated: row.updated,
                removed: row.removed,
                unchanged: row.unchanged,
            };
            (row.id, last)
        })
        .collect())
}

/// Records `last` as the Library `id`'s last Scan.
pub(super) async fn record(
    conn: &mut SqliteConnection,
    id: i64,
    last: LastScan,
) -> sqlx::Result<()> {
    sqlx::query!(
        "UPDATE libraries SET last_scan_at = ?, last_scan_added = ?, last_scan_updated = ?,
                              last_scan_removed = ?, last_scan_unchanged = ?
         WHERE id = ?",
        last.finished_at,
        // SQLite has no unsigned integers; no count of files comes near `i64::MAX`.
        last.added.cast_signed(),
        last.updated.cast_signed(),
        last.removed.cast_signed(),
        last.unchanged.cast_signed(),
        id,
    )
    .execute(conn)
    .await?;
    Ok(())
}
