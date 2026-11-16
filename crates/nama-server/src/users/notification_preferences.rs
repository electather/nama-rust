//! Notification preferences: the kinds of Request notification each User turned off, on every
//! Notifier. A User gets every kind until they turn it off, so a new User has nothing stored.

use buffa::Enumeration;
use nama_proto::nama::v1::{NotificationKind, Permission};

use crate::{db::Database, users::Permissions};

/// Each kind a User holding `permissions` may get, in the order of its values: new Pending only
/// with `PERMISSION_MANAGE_REQUESTS`, as `crate::events` tells it.
pub(crate) fn offered(permissions: Permissions) -> impl Iterator<Item = NotificationKind> {
    let manages_requests = permissions.allow(Permission::ManageRequests);
    NotificationKind::values()
        .iter()
        .copied()
        .filter(move |&kind| {
            kind != NotificationKind::Unspecified
                && (kind != NotificationKind::NewPending || manages_requests)
        })
}

/// The kinds `user` turned off.
pub(crate) async fn turned_off(
    db: &Database,
    user: i64,
) -> Result<Vec<NotificationKind>, sqlx::Error> {
    let kinds = sqlx::query_scalar!("SELECT kind FROM notifications_off WHERE user_id = ?", user)
        .fetch_all(db.reader())
        .await?;
    Ok(kinds.into_iter().filter_map(stored_kind).collect())
}

/// Every User's turned-off kinds, by User id.
// ponytail: reads the whole table, at most seven rows a User; filter by kind if Users number in
// the thousands.
pub(crate) async fn every_turned_off(
    db: &Database,
) -> Result<Vec<(i64, NotificationKind)>, sqlx::Error> {
    let rows = sqlx::query!("SELECT user_id, kind FROM notifications_off")
        .fetch_all(db.reader())
        .await?;
    Ok(rows
        .into_iter()
        .filter_map(|row| Some((row.user_id, stored_kind(row.kind)?)))
        .collect())
}

/// Turns `kind` on or off for `user`.
pub(crate) async fn set(
    db: &Database,
    user: i64,
    kind: NotificationKind,
    on: bool,
) -> Result<(), sqlx::Error> {
    let kind = kind as i32;
    db.write(move |tx| {
        Box::pin(async move {
            if on {
                sqlx::query!(
                    "DELETE FROM notifications_off WHERE user_id = ? AND kind = ?",
                    user,
                    kind
                )
                .execute(&mut **tx)
                .await?;
            } else {
                sqlx::query!(
                    "INSERT OR IGNORE INTO notifications_off (user_id, kind) VALUES (?, ?)",
                    user,
                    kind
                )
                .execute(&mut **tx)
                .await?;
            }
            Ok(())
        })
    })
    .await
}

/// The kind `notifications_off.kind` holds as `stored`, or `None` for a number this Server no
/// longer knows.
fn stored_kind(stored: i64) -> Option<NotificationKind> {
    NotificationKind::from_i32(i32::try_from(stored).ok()?)
}
