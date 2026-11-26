use std::collections::BTreeMap;

use nama_plugin_api::PluginId;
use tempfile::TempDir;

use super::{delete, get, recipients, renew, replace};
use crate::db::Database;

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

fn setting(key: &str, value: &str, secret: bool) -> (String, String, bool) {
    (key.to_owned(), value.to_owned(), secret)
}

#[tokio::test]
async fn storing_replacing_and_deleting_keep_only_the_last_values() {
    let (_dir, db) = open().await;
    let ada = insert_user(&db, "ada").await;
    let ntfy = PluginId::from_static("ntfy");

    replace(
        &db,
        ada,
        &ntfy,
        vec![
            setting("topic", "ada-films", false),
            setting("token", "tk_1", true),
        ],
    )
    .await
    .unwrap();
    let stored = get(&db, ada, &ntfy).await.unwrap();
    replace(&db, ada, &ntfy, vec![setting("topic", "ada-shows", false)])
        .await
        .unwrap();
    let replaced = get(&db, ada, &ntfy).await.unwrap();
    delete(&db, ada, &ntfy).await.unwrap();

    assert_eq!(
        stored,
        BTreeMap::from([
            ("token".to_owned(), ("tk_1".to_owned(), true)),
            ("topic".to_owned(), ("ada-films".to_owned(), false)),
        ])
    );
    assert_eq!(
        replaced,
        BTreeMap::from([("topic".to_owned(), ("ada-shows".to_owned(), false))])
    );
    assert!(get(&db, ada, &ntfy).await.unwrap().is_empty());
}

#[tokio::test]
async fn recipients_are_each_user_with_values_for_the_plugin_with_their_secrets_apart() {
    let (_dir, db) = open().await;
    let (ada, grace) = (
        insert_user(&db, "ada").await,
        insert_user(&db, "grace").await,
    );
    let ntfy = PluginId::from_static("ntfy");
    let discord = PluginId::from_static("discord");
    replace(
        &db,
        ada,
        &ntfy,
        vec![
            setting("topic", "ada-films", false),
            setting("token", "tk_1", true),
        ],
    )
    .await
    .unwrap();
    replace(
        &db,
        grace,
        &discord,
        vec![setting("webhook", "https://d/1", true)],
    )
    .await
    .unwrap();

    let told: Vec<_> = recipients(&db, &ntfy)
        .await
        .unwrap()
        .into_iter()
        .map(|(user, values, secrets)| (user.id, values, secrets))
        .collect();

    assert_eq!(
        told,
        [(
            ada,
            BTreeMap::from([
                ("token".to_owned(), "tk_1".to_owned()),
                ("topic".to_owned(), "ada-films".to_owned()),
            ]),
            vec!["tk_1".to_owned()],
        )]
    );
}

#[tokio::test]
async fn deleting_a_user_deletes_their_personal_settings() {
    let (_dir, db) = open().await;
    let ada = insert_user(&db, "ada").await;
    let ntfy = PluginId::from_static("ntfy");
    replace(&db, ada, &ntfy, vec![setting("topic", "ada-films", false)])
        .await
        .unwrap();

    db.write(move |tx| {
        Box::pin(async move {
            sqlx::query("DELETE FROM users WHERE id = ?")
                .bind(ada)
                .execute(&mut **tx)
                .await
                .map(drop)
        })
    })
    .await
    .unwrap();

    assert!(get(&db, ada, &ntfy).await.unwrap().is_empty());
    assert!(recipients(&db, &ntfy).await.unwrap().is_empty());
}

#[tokio::test]
async fn renewing_writes_only_secrets_the_user_still_has() {
    let (_dir, db) = open().await;
    let (ada, grace) = (
        insert_user(&db, "ada").await,
        insert_user(&db, "grace").await,
    );
    let trakt = PluginId::from_static("trakt");
    let ada_linked = vec![
        setting("access_token", "access-1", true),
        setting("username", "ada", false),
    ];
    replace(&db, ada, &trakt, ada_linked).await.unwrap();
    let grace_linked = vec![setting("access_token", "access-1", true)];
    replace(&db, grace, &trakt, grace_linked).await.unwrap();
    delete(&db, grace, &trakt).await.unwrap();
    let renewed = |token: &str| {
        [
            ("access_token".to_owned(), token.to_owned()),
            ("username".to_owned(), "mallory".to_owned()),
            ("refresh_token".to_owned(), "refresh-2".to_owned()),
        ]
    };

    renew(&db, ada, &trakt, renewed("access-2")).await.unwrap();
    renew(&db, grace, &trakt, renewed("access-3"))
        .await
        .unwrap();

    assert_eq!(
        get(&db, ada, &trakt).await.unwrap(),
        BTreeMap::from([
            ("access_token".to_owned(), ("access-2".to_owned(), true)),
            ("username".to_owned(), ("ada".to_owned(), false)),
        ])
    );
    assert!(
        get(&db, grace, &trakt).await.unwrap().is_empty(),
        "unlinked"
    );
}
