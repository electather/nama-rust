//! The Watchlist (ADR 0056): the Movies and Shows each User means to watch, kept by kind and
//! external id whether or not the Server has them. Only this file reads or writes `watchlist`
//! (`AGENTS.md`).

use nama_plugin_api::metadata::{ExternalId, ExternalIds};

use crate::{
    db::Database,
    library::{TitleKind, matched_ids},
};

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

/// Takes the title of `kind` with the id `id` off the User `user`'s Watchlist, under every id the
/// Server's Matches give it, since [`has`] finds it under any of them.
pub(crate) async fn remove(
    db: &Database,
    user: i64,
    kind: TitleKind,
    id: &ExternalId,
) -> Result<(), sqlx::Error> {
    let id = id.clone();
    db.write(move |tx| {
        Box::pin(async move {
            let mut ids = matched_ids(tx, kind, &id).await?;
            ids.push(id);
            let ids: Vec<String> = ids.iter().map(ExternalId::to_string).collect();
            let (kind, ids) = (stored(kind), serde_json::Value::from(ids).to_string());
            sqlx::query!(
                "DELETE FROM watchlist
                 WHERE user_id = ? AND kind = ? AND title_id IN (SELECT value FROM json_each(?))",
                user,
                kind,
                ids
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
