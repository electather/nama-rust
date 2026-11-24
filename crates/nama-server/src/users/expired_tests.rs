use std::{sync::Arc, time::Duration};

use super::{super::*, ExpiredDeleted};
use crate::plugins::ExtensionPoints;

fn device(name: &str) -> NewDevice {
    NewDevice {
        name: name.to_owned(),
        ip_address: "192.0.2.7".parse().unwrap(),
        user_agent: None,
    }
}

/// Signs a Device named `name` in as `user`, answering its id.
async fn sign_in(users: &Users, user: i64, name: &str) -> i64 {
    let device = device(name);
    let token_hash = hash_token(name);
    users
        .db
        .write(move |tx| {
            Box::pin(async move { insert_device(tx, user, None, device, &token_hash).await })
        })
        .await
        .unwrap()
        .id
}

/// Moves the expiry of the row `id` to a second ago, with `update`, which binds the time and then
/// the id.
async fn expire(users: &Users, update: &'static str, id: i64) {
    let past = now() - 1;
    users
        .db
        .write(move |tx| {
            Box::pin(async move {
                sqlx::query(update)
                    .bind(past)
                    .bind(id)
                    .execute(&mut **tx)
                    .await
            })
        })
        .await
        .unwrap();
}

async fn stored_ids(users: &Users, select: &'static str) -> Vec<i64> {
    sqlx::query_scalar(select)
        .fetch_all(users.db.reader())
        .await
        .unwrap()
}

/// How long the test waits for the expired Device's sign-out to be heard before failing.
const WAIT: Duration = Duration::from_secs(10);

#[tokio::test]
async fn deleting_expired_ones_deletes_them_signs_devices_out_and_keeps_the_rest() {
    let data_dir = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data_dir.path()).await.unwrap());
    let admin = db
        .write(|tx| Box::pin(async move { insert_user(tx, "Root", Permissions::ADMIN).await }))
        .await
        .unwrap();
    let users = Users::new(db, ExtensionPoints::default());
    let expired_device = sign_in(&users, admin.id, "Old phone").await;
    let kept_device = sign_in(&users, admin.id, "Laptop").await;
    let invite = async |note| {
        let created = users.create_invite(&admin, note, Permissions::NEW_USER);
        created.await.unwrap().invite.id
    };
    let expired_invite = invite("Mum").await;
    let kept_invite = invite("Dad").await;
    expire(
        &users,
        "UPDATE devices SET expires_at = ? WHERE id = ?",
        expired_device,
    )
    .await;
    expire(
        &users,
        "UPDATE invites SET expires_at = ? WHERE id = ?",
        expired_invite,
    )
    .await;
    let mut signed_out = std::pin::pin!(users.signed_out(expired_device, admin.id));
    // Polled first, so it is past looking the Device up and only a sign-out can end it.
    let before = tokio::time::timeout(Duration::from_millis(200), &mut signed_out).await;
    assert!(before.is_err(), "the wait ended before the delete");

    let deleted = users.delete_expired().await.unwrap();

    assert_eq!(
        deleted,
        ExpiredDeleted {
            devices: 1,
            invites: 1
        }
    );
    assert_eq!(
        stored_ids(&users, "SELECT id FROM devices").await,
        [kept_device]
    );
    assert_eq!(
        stored_ids(&users, "SELECT id FROM invites").await,
        [kept_invite]
    );
    let heard = tokio::time::timeout(WAIT, signed_out).await;
    assert!(heard.is_ok(), "the expired Device's waiter wasn't told");
}
