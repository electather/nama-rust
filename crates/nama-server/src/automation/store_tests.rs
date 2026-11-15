use nama_plugin_api::PluginId;
use tempfile::TempDir;

use super::{Error, delete, get, put};
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

#[tokio::test]
async fn values_round_trip_plugin_wide_and_per_user_apart() {
    let (_dir, db) = open().await;
    let ada = insert_user(&db, "ada").await;
    let cleanup = PluginId::from_static("cleanup");

    put(&db, &cleanup, None, "seen".into(), b"all".to_vec())
        .await
        .unwrap();
    put(&db, &cleanup, Some(ada), "seen".into(), b"ada".to_vec())
        .await
        .unwrap();
    put(&db, &cleanup, None, "seen".into(), b"all again".to_vec())
        .await
        .unwrap();
    let plugin_wide = get(&db, &cleanup, None, "seen").await.unwrap();
    let adas = get(&db, &cleanup, Some(ada), "seen").await.unwrap();
    delete(&db, &cleanup, Some(ada), "seen".into())
        .await
        .unwrap();

    assert_eq!(plugin_wide.as_deref(), Some(&b"all again"[..]));
    assert_eq!(adas.as_deref(), Some(&b"ada"[..]));
    assert_eq!(get(&db, &cleanup, Some(ada), "seen").await.unwrap(), None);
    assert!(get(&db, &cleanup, None, "seen").await.unwrap().is_some());
}

#[tokio::test]
async fn one_plugin_cant_read_anothers_entries() {
    let (_dir, db) = open().await;
    put(
        &db,
        &PluginId::from_static("cleanup"),
        None,
        "seen".into(),
        b"1".to_vec(),
    )
    .await
    .unwrap();

    let other = get(&db, &PluginId::from_static("other"), None, "seen")
        .await
        .unwrap();

    assert_eq!(other, None);
}

#[tokio::test]
async fn a_long_key_or_a_large_value_is_refused() {
    let (_dir, db) = open().await;
    let cleanup = PluginId::from_static("cleanup");

    let key = put(&db, &cleanup, None, "k".repeat(257), Vec::new()).await;
    let value = put(&db, &cleanup, None, "k".into(), vec![0; 64 * 1024 + 1]).await;
    let largest = put(&db, &cleanup, None, "k".repeat(256), vec![0; 64 * 1024]).await;

    assert!(matches!(key, Err(Error::KeyTooLong(257))), "{key:?}");
    assert_eq!(
        key.unwrap_err().to_string(),
        "a Store key can't be over 256 bytes, and this one is 257"
    );
    assert!(
        matches!(value, Err(Error::ValueTooLarge(65_537))),
        "{value:?}"
    );
    largest.unwrap();
}

#[tokio::test]
async fn deleting_a_user_deletes_their_entries() {
    let (_dir, db) = open().await;
    let ada = insert_user(&db, "ada").await;
    let cleanup = PluginId::from_static("cleanup");
    put(&db, &cleanup, Some(ada), "seen".into(), b"1".to_vec())
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
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM plugin_store")
        .fetch_one(db.reader())
        .await
        .unwrap();

    assert_eq!(left, 0);
}

#[tokio::test]
async fn an_entry_for_no_user_is_refused() {
    let (_dir, db) = open().await;

    let put = put(
        &db,
        &PluginId::from_static("cleanup"),
        Some(42),
        "seen".into(),
        Vec::new(),
    )
    .await;

    assert!(matches!(put, Err(Error::NoUser(42))), "{put:?}");
}
