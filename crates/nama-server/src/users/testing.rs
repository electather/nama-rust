//! A bare User row, and a Device signed in as one, for in-file tests across the Server ("One
//! in-file fixture", crate AGENTS.md).

use super::permissions::Permissions;
use crate::{db::Database, server_settings::PlaybackLimit};

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

/// Inserts a Device signed in as the User `user`, acting as them, and returns its id.
pub(crate) async fn insert_device(db: &Database, user: i64) -> i64 {
    db.write(move |tx| {
        Box::pin(async move {
            let device = sqlx::query_scalar(
                "INSERT INTO devices (user_id, name, token_hash, ip_address, created_at,
                                      updated_at, last_seen_at, expires_at)
                 VALUES (?, 'Laptop', randomblob(32), '127.0.0.1', 0, 0, 0, ?) RETURNING id",
            )
            .bind(user)
            .bind(i64::MAX)
            .fetch_one(&mut **tx)
            .await?;
            sqlx::query(
                "INSERT INTO device_users (device_id, user_id, authenticated_at) VALUES (?, ?, 0)",
            )
            .bind(device)
            .bind(user)
            .execute(&mut **tx)
            .await?;
            Ok::<i64, sqlx::Error>(device)
        })
    })
    .await
    .unwrap()
}

/// Gives the User `user` the cap for remote playback `cap`.
pub(crate) async fn cap_remote_playback(db: &Database, user: i64, cap: PlaybackLimit) {
    db.write(move |tx| {
        Box::pin(async move {
            sqlx::query("UPDATE users SET remote_playback_limit = ? WHERE id = ?")
                .bind(cap.name())
                .bind(user)
                .execute(&mut **tx)
                .await
        })
    })
    .await
    .unwrap();
}

/// Makes the Device `device` act as the User `user`, signing them in to it if they aren't, or as
/// none, as a switch or a sign-out does.
pub(crate) async fn act_as(db: &Database, device: i64, user: Option<i64>) {
    db.write(move |tx| {
        Box::pin(async move {
            sqlx::query(
                "INSERT OR IGNORE INTO device_users (device_id, user_id, authenticated_at)
                 SELECT ?1, ?2, 0 WHERE ?2 IS NOT NULL",
            )
            .bind(device)
            .bind(user)
            .execute(&mut **tx)
            .await?;
            sqlx::query("UPDATE devices SET user_id = ? WHERE id = ?")
                .bind(user)
                .bind(device)
                .execute(&mut **tx)
                .await
        })
    })
    .await
    .unwrap();
}
