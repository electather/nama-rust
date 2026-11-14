use std::sync::Arc;

use tempfile::TempDir;

use super::{Kind, Libraries};
use crate::db::Database;

/// A fresh database (its folder is the first `TempDir`) and a folder a Library can hold.
async fn setup() -> (TempDir, TempDir, Libraries, String) {
    let data = tempfile::tempdir().unwrap();
    let media = tempfile::tempdir().unwrap();
    let folder = media.path().canonicalize().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let folder = folder.to_str().unwrap().to_owned();
    (data, media, Libraries::new(db), folder)
}

fn order(namespaces: &[&str]) -> Vec<String> {
    namespaces.iter().map(|&n| n.to_owned()).collect()
}

#[tokio::test]
async fn create_in_order_stores_and_returns_the_order() {
    let (_data, _media, libraries, folder) = setup().await;
    let created = libraries
        .create_in_order("Shows", Kind::Shows, vec![folder], order(&["tvdb", "tmdb"]))
        .await
        .unwrap();
    assert_eq!(created.title_sources, order(&["tvdb", "tmdb"]));
    let stored = libraries.get(created.id).await.unwrap();
    assert_eq!(stored.title_sources, order(&["tvdb", "tmdb"]));
}

#[tokio::test]
async fn update_with_an_empty_order_keeps_the_old_one() {
    let (_data, _media, libraries, folder) = setup().await;
    let created = libraries
        .create_in_order("Shows", Kind::Shows, vec![folder.clone()], order(&["tvdb"]))
        .await
        .unwrap();
    let updated = libraries
        .update(created.id, "Renamed", vec![folder], Vec::new())
        .await
        .unwrap();
    assert_eq!(updated.title_sources, order(&["tvdb"]));
    let stored = libraries.get(created.id).await.unwrap();
    assert_eq!(stored.title_sources, order(&["tvdb"]));
}

#[tokio::test]
async fn update_with_a_new_order_stores_it() {
    let (_data, _media, libraries, folder) = setup().await;
    let created = libraries
        .create("Shows", Kind::Shows, vec![folder.clone()])
        .await
        .unwrap();
    let updated = libraries
        .update(created.id, "Shows", vec![folder], order(&["tvdb", "tmdb"]))
        .await
        .unwrap();
    assert_eq!(updated.title_sources, order(&["tvdb", "tmdb"]));
    let stored = libraries.get(created.id).await.unwrap();
    assert_eq!(stored.title_sources, order(&["tvdb", "tmdb"]));
}
