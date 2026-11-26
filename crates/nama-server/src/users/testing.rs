//! A bare User row for in-file tests across the Server ("One in-file fixture", crate AGENTS.md).

use super::permissions::Permissions;
use crate::db::Database;

/// Inserts a User named `name` with no permissions and returns their id.
pub(crate) async fn insert_user(db: &Database, name: &str) -> i64 {
    insert_user_holding(db, name, Permissions(0)).await
}

/// Inserts a User named `name` holding `permissions` and returns their id.
pub(crate) async fn insert_user_holding(
    db: &Database,
    name: &str,
    permissions: Permissions,
) -> i64 {
    let name = name.to_owned();
    db.write(move |tx| {
        Box::pin(async move {
            sqlx::query_scalar(
                "INSERT INTO users (username, display_username, permissions, created_at,
                                    updated_at)
                 VALUES (?, ?, ?, 0, 0) RETURNING id",
            )
            .bind(&name)
            .bind(&name)
            .bind(permissions)
            .fetch_one(&mut **tx)
            .await
        })
    })
    .await
    .unwrap()
}
