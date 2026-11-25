use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use axum::http::StatusCode;
use nama_proto::nama::v1::{InboxNotification, NotificationKind, WebPushMessage};
use tempfile::TempDir;
use url::Url;

use super::{push, subscribe, subscriptions_of, unsubscribe};
use crate::{
    db::Database,
    web_push::{
        Pusher,
        testing::{assert_valid_vapid, browser, new_key, push_service, silent_push_service},
    },
};

async fn open() -> (TempDir, Database) {
    let data_dir = tempfile::tempdir().unwrap();
    let db = Database::open(data_dir.path()).await.unwrap();
    (data_dir, db)
}

async fn insert_user(db: &Database, name: &str) -> i64 {
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

async fn endpoints_of(db: &Database, user: i64) -> Vec<String> {
    let subscriptions = subscriptions_of(db, user).await.unwrap();
    subscriptions.into_iter().map(|to| to.endpoint).collect()
}

#[tokio::test]
async fn unsubscribing_removes_only_the_callers_and_another_subscribing_takes_the_endpoint() {
    let (_dir, db) = open().await;
    let (ada, lin) = (insert_user(&db, "ada").await, insert_user(&db, "lin").await);
    let (laptop, phone) = (
        "https://push.example.net/laptop",
        "https://push.example.net/phone",
    );
    subscribe(&db, ada, browser(laptop)).await.unwrap();
    subscribe(&db, ada, browser(phone)).await.unwrap();

    subscribe(&db, lin, browser(phone)).await.unwrap();
    unsubscribe(&db, ada, phone.to_owned()).await.unwrap();

    assert_eq!(endpoints_of(&db, ada).await, [laptop]);
    assert_eq!(endpoints_of(&db, lin).await, [phone]);
    unsubscribe(&db, lin, phone.to_owned()).await.unwrap();
    assert_eq!(endpoints_of(&db, lin).await, Vec::<String>::new());
}

#[tokio::test]
async fn deleting_a_user_deletes_their_subscriptions() {
    let (_dir, db) = open().await;
    let ada = insert_user(&db, "ada").await;
    subscribe(&db, ada, browser("https://push.example.net/1"))
        .await
        .unwrap();

    db.write(move |tx| {
        Box::pin(async move {
            sqlx::query("DELETE FROM users WHERE id = ?")
                .bind(ada)
                .execute(&mut **tx)
                .await
        })
    })
    .await
    .unwrap();

    assert_eq!(endpoints_of(&db, ada).await, Vec::<String>::new());
}

#[tokio::test]
async fn each_subscription_is_pushed_to_and_a_gone_one_deleted_while_a_silent_one_waits() {
    let (_dir, db) = open().await;
    let db = Arc::new(db);
    let ada = insert_user(&db, "ada").await;
    let (laptop, laptop_got) = push_service(StatusCode::CREATED).await;
    let (phone, phone_got) = push_service(StatusCode::CREATED).await;
    let (gone, _) = push_service(StatusCode::GONE).await;
    // Subscribed first, so a push waiting on it would hold up the others.
    subscribe(&db, ada, browser(&silent_push_service().await))
        .await
        .unwrap();
    for endpoint in [&laptop, &phone, &gone] {
        subscribe(&db, ada, browser(endpoint)).await.unwrap();
    }
    let key = new_key();
    let public_key = key.public_key().to_vec();
    let pusher = Pusher::new(key).unwrap();
    let notification = InboxNotification {
        id: 9,
        kind: NotificationKind::Available.into(),
        title: "Dune".to_owned(),
        request_id: 4,
        ..Default::default()
    };
    let link = Some("https://nama.example.com/?settings=requests".to_owned());

    let pushing = tokio::spawn({
        let (db, notification) = (Arc::clone(&db), notification.clone());
        async move { push(&db, &pusher, vec![(ada, notification)], link).await }
    });

    let started = Instant::now();
    let all_answered =
        || !laptop_got.lock().unwrap().is_empty() && !phone_got.lock().unwrap().is_empty();
    while endpoints_of(&db, ada).await.len() == 4 || !all_answered() {
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "a silent push service held up the rest"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    pushing.abort();
    for (endpoint, got) in [(&laptop, &laptop_got), (&phone, &phone_got)] {
        let (headers, body) = got.lock().unwrap().pop().unwrap();
        let origin = Url::parse(endpoint).unwrap().origin().ascii_serialization();
        assert_valid_vapid(
            headers["authorization"].to_str().unwrap(),
            &origin,
            &public_key,
        );
        // The message itself, in the JSON mapping, padded by the record's delimiter and tag.
        let message = WebPushMessage {
            notification: notification.clone().into(),
            link: Some("https://nama.example.com/?settings=requests".to_owned()),
            ..Default::default()
        };
        let plaintext = serde_json::to_vec(&message).unwrap();
        assert_eq!(body.len(), 86 + plaintext.len() + 17);
    }
    assert!(!endpoints_of(&db, ada).await.contains(&gone));
}
