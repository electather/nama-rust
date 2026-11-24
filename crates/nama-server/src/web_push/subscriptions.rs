//! Each User's Web Push subscriptions, one per browser they turned it on in, and pushing their
//! inbox notifications to them.

use futures_util::future;
use nama_proto::nama::v1::{InboxNotification, WebPushMessage};
use tracing::{info, warn};

use super::{Pushed, Pusher, Subscription};
use crate::db::{Database, now};

/// Pushes `user`'s notifications to `subscription` from now on. An endpoint another User
/// subscribed becomes `user`'s, as the browser holding it now signs in as them.
pub(crate) async fn subscribe(
    db: &Database,
    user: i64,
    subscription: Subscription,
) -> Result<(), sqlx::Error> {
    let created_at = now();
    db.write(move |tx| {
        Box::pin(async move {
            sqlx::query!(
                "INSERT INTO push_subscriptions (user_id, endpoint, p256dh, auth, created_at)
                 VALUES (?, ?, ?, ?, ?)
                 ON CONFLICT (endpoint) DO UPDATE SET user_id = excluded.user_id,
                     p256dh = excluded.p256dh, auth = excluded.auth,
                     created_at = excluded.created_at",
                user,
                subscription.endpoint,
                subscription.p256dh,
                subscription.auth,
                created_at
            )
            .execute(&mut **tx)
            .await?;
            Ok(())
        })
    })
    .await
}

/// Stops pushing to `user`'s subscription at `endpoint`, leaving another User's as it is.
pub(crate) async fn unsubscribe(
    db: &Database,
    user: i64,
    endpoint: String,
) -> Result<(), sqlx::Error> {
    db.write(move |tx| {
        Box::pin(async move {
            sqlx::query!(
                "DELETE FROM push_subscriptions WHERE user_id = ? AND endpoint = ?",
                user,
                endpoint
            )
            .execute(&mut **tx)
            .await?;
            Ok(())
        })
    })
    .await
}

/// Pushes each of `stored`, a notification and the User whose inbox it went in, to every
/// subscription of that User's at once, with `link` to the Requests, and deletes each
/// subscription its push service says is gone. A failed push is logged by User id: an endpoint is
/// the subscription's secret.
pub(crate) async fn push(
    db: &Database,
    pusher: &Pusher,
    stored: Vec<(i64, InboxNotification)>,
    link: Option<String>,
) {
    let mut deliveries = Vec::new();
    for (user, notification) in stored {
        let subscriptions = match subscriptions_of(db, user).await {
            Ok(subscriptions) => subscriptions,
            Err(err) => {
                warn!(user, "could not read whom to push a notification to: {err}");
                continue;
            }
        };
        let message = WebPushMessage {
            notification: notification.into(),
            link: link.clone(),
            ..Default::default()
        };
        let message = match serde_json::to_vec(&message) {
            Ok(message) => message,
            Err(err) => {
                warn!(user, "could not write a notification to push: {err}");
                continue;
            }
        };
        deliveries.extend(
            subscriptions
                .into_iter()
                .map(|to| (user, to, message.clone())),
        );
    }
    future::join_all(
        deliveries
            .into_iter()
            .map(|(user, to, message)| async move {
                match pusher.push(&to, &message).await {
                    Ok(Pushed::Delivered) => {}
                    Ok(Pushed::Gone) => forget(db, user, to.endpoint).await,
                    Err(err) => warn!(user, "could not push a notification: {err}"),
                }
            }),
    )
    .await;
}

async fn subscriptions_of(db: &Database, user: i64) -> Result<Vec<Subscription>, sqlx::Error> {
    sqlx::query_as!(
        Subscription,
        "SELECT endpoint, p256dh, auth FROM push_subscriptions WHERE user_id = ? ORDER BY id",
        user
    )
    .fetch_all(db.reader())
    .await
}

/// Deletes `user`'s subscription at `endpoint`, which its push service says is gone.
async fn forget(db: &Database, user: i64, endpoint: String) {
    match unsubscribe(db, user, endpoint).await {
        Ok(()) => info!(
            user,
            "deleted a push subscription its push service says is gone"
        ),
        Err(err) => warn!(
            user,
            "could not delete a push subscription that is gone: {err}"
        ),
    }
}

#[cfg(test)]
#[path = "subscriptions_tests.rs"]
mod tests;
