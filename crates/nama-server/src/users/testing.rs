//! A bare User row, which in-file tests across the Server build on, so a change to the `users`
//! columns touches one place.

use crate::db::Database;

/// Inserts a User named `name` with no permissions and returns their id.
pub(crate) async fn insert_user(db: &Database, name: &str) -> i64 {
    let name = name.to_owned();
    db.write(move |tx| {
        Box::pin(async move {
            sqlx::query_scalar(
                "INSERT INTO users (username, display_username, permissions, created_at,
                                    updated_at)
                 VALUES (?, ?, 0, 0, 0) RETURNING id",
            )
            .bind(&name)
            .bind(&name)
            .fetch_one(&mut **tx)
            .await
        })
    })
    .await
    .unwrap()
}
