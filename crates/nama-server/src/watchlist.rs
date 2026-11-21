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
            let ids = json_ids(matched_ids(tx, kind, &id).await?);
            let kind = stored(kind);
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
    let (kind, ids) = (stored(kind), json_ids(ids.iter()));
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

/// The User `user`'s Watchlist, each title by the id it was added under. A row whose kind or id
/// doesn't parse is skipped.
pub(crate) async fn list(
    db: &Database,
    user: i64,
) -> Result<Vec<(TitleKind, ExternalId)>, sqlx::Error> {
    let rows = sqlx::query!(
        "SELECT kind, title_id FROM watchlist WHERE user_id = ?",
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
