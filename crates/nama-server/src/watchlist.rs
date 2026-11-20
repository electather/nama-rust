//! The Watchlist (ADR 0056): the Movies and Shows each User means to watch, kept by kind and
//! external id whether or not the Server has them. Only this file reads or writes `watchlist`
//! (`AGENTS.md`).

use nama_plugin_api::metadata::{ExternalId, ExternalIds};

use crate::{db::Database, library::TitleKind};

/// Puts the title of `kind` with the id `id` on the User `user`'s Watchlist, if it isn't already.
pub(crate) async fn add(
    db: &Database,
    user: i64,
    kind: TitleKind,
    id: &ExternalId,
) -> Result<(), sqlx::Error> {
    let (kind, id) = (stored(kind), id.to_string());
    db.write(move |tx| {
        Box::pin(async move {
            sqlx::query!(
                "INSERT OR IGNORE INTO watchlist (user_id, kind, title_id) VALUES (?, ?, ?)",
                user,
                kind,
                id
            )
            .execute(&mut **tx)
            .await
            .map(drop)
        })
    })
    .await
}

/// Takes the title of `kind` added under the id `id` off the User `user`'s Watchlist, if it's
/// there.
// ponytail: a title added under another of its ids stays on; remove under every id the Server
// knows it by once Libraries' title source orders, or Home rows' ids, differ in practice (#1292).
pub(crate) async fn remove(
    db: &Database,
    user: i64,
    kind: TitleKind,
    id: &ExternalId,
) -> Result<(), sqlx::Error> {
    let (kind, id) = (stored(kind), id.to_string());
    db.write(move |tx| {
        Box::pin(async move {
            sqlx::query!(
                "DELETE FROM watchlist WHERE user_id = ? AND kind = ? AND title_id = ?",
                user,
                kind,
                id
            )
            .execute(&mut **tx)
            .await
            .map(drop)
        })
    })
    .await
}

/// Whether the title of `kind` known by `ids` is on the User `user`'s Watchlist under any of them.
pub(crate) async fn has(
    db: &Database,
    user: i64,
    kind: TitleKind,
    ids: &ExternalIds,
) -> Result<bool, sqlx::Error> {
    let ids: Vec<String> = ids.iter().map(|id| id.to_string()).collect();
    let (kind, ids) = (stored(kind), serde_json::Value::from(ids).to_string());
    sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM watchlist
                          WHERE user_id = ? AND kind = ?
                            AND title_id IN (SELECT value FROM json_each(?))) AS "on: bool""#,
        user,
        kind,
        ids
    )
    .fetch_one(db.reader())
    .await
}

/// The User `user`'s Watchlist, the title added first first, each by the id it was added under.
/// A row whose kind or id doesn't parse is skipped.
pub(crate) async fn list(
    db: &Database,
    user: i64,
) -> Result<Vec<(TitleKind, ExternalId)>, sqlx::Error> {
    let rows = sqlx::query!(
        "SELECT kind, title_id FROM watchlist WHERE user_id = ? ORDER BY rowid",
        user
    )
    .fetch_all(db.reader())
    .await?;
    Ok((rows.into_iter())
        .filter_map(|row| {
            let kind = match row.kind.as_str() {
                "movie" => TitleKind::Movie,
                "show" => TitleKind::Show,
                _ => return None,
            };
            Some((kind, row.title_id.parse().ok()?))
        })
        .collect())
}

/// `kind` as `watchlist` stores it.
const fn stored(kind: TitleKind) -> &'static str {
    match kind {
        TitleKind::Movie => "movie",
        TitleKind::Show => "show",
    }
}

#[cfg(test)]
#[path = "watchlist_tests.rs"]
mod tests;
