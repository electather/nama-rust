//! Autoplay: whether the next Episode plays by itself for a User, on every Device. It's on until
//! they turn it off, so only the Users who did have a row in `autoplay_off` (`AGENTS.md`).

use crate::db::Database;

/// Whether the next Episode plays by itself for `user`.
pub(crate) async fn is_on(db: &Database, user: i64) -> Result<bool, sqlx::Error> {
    let off = sqlx::query_scalar!("SELECT 1 FROM autoplay_off WHERE user_id = ?", user)
        .fetch_optional(db.reader())
        .await?;
    Ok(off.is_none())
}

/// Turns autoplay on or off for `user`.
pub(crate) async fn set(db: &Database, user: i64, on: bool) -> Result<(), sqlx::Error> {
    db.write(move |tx| {
        Box::pin(async move {
            if on {
                sqlx::query!("DELETE FROM autoplay_off WHERE user_id = ?", user)
                    .execute(&mut **tx)
                    .await?;
            } else {
                sqlx::query!(
                    "INSERT OR IGNORE INTO autoplay_off (user_id) VALUES (?)",
                    user
                )
                .execute(&mut **tx)
                .await?;
            }
            Ok(())
        })
    })
    .await
}

#[cfg(test)]
#[path = "autoplay_tests.rs"]
mod tests;
