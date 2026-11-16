use std::sync::Arc;

use tempfile::TempDir;

use super::{Kind, Libraries, Order};
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

/// An [`Order`] of `title_sources` alone, keeping the Recognisers.
fn sources(namespaces: &[&str]) -> Order {
    Order {
        title_sources: order(namespaces),
        recognisers: None,
    }
}

/// An [`Order`] setting the Recognisers `ids`, keeping the title sources.
fn recognisers(ids: &[&str]) -> Order {
    Order {
        title_sources: Vec::new(),
        recognisers: Some(order(ids)),
    }
}

#[tokio::test]
async fn create_in_order_stores_and_returns_the_order() {
    let (_data, _media, libraries, folder) = setup().await;
    let created = libraries
        .create_in_order(
            "Shows",
            Kind::Shows,
            vec![folder],
            sources(&["tvdb", "tmdb"]),
        )
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
        .create_in_order(
            "Shows",
            Kind::Shows,
            vec![folder.clone()],
            sources(&["tvdb"]),
        )
        .await
        .unwrap();
    let updated = libraries
        .update(created.id, "Renamed", vec![folder], Order::default())
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
        .update(
            created.id,
            "Shows",
            vec![folder],
            sources(&["tvdb", "tmdb"]),
        )
        .await
        .unwrap();
    assert_eq!(updated.title_sources, order(&["tvdb", "tmdb"]));
    let stored = libraries.get(created.id).await.unwrap();
    assert_eq!(stored.title_sources, order(&["tvdb", "tmdb"]));
}

#[tokio::test]
async fn a_new_library_asks_no_recognisers() {
    let (_data, _media, libraries, folder) = setup().await;
    let created = libraries
        .create("Shows", Kind::Shows, vec![folder])
        .await
        .unwrap();
    assert_eq!(created.recognisers, Vec::<String>::new());
    let stored = libraries.get(created.id).await.unwrap();
    assert_eq!(stored.recognisers, Vec::<String>::new());
}

#[tokio::test]
async fn create_in_order_stores_and_returns_the_recognisers() {
    let (_data, _media, libraries, folder) = setup().await;
    let created = libraries
        .create_in_order("Shows", Kind::Shows, vec![folder], recognisers(&["b", "a"]))
        .await
        .unwrap();
    assert_eq!(created.recognisers, order(&["b", "a"]));
    let listed = libraries.list().await.unwrap();
    assert_eq!(listed[0].recognisers, order(&["b", "a"]));
}

#[tokio::test]
async fn update_without_recognisers_keeps_them_and_with_none_clears_them() {
    let (_data, _media, libraries, folder) = setup().await;
    let created = libraries
        .create_in_order(
            "Shows",
            Kind::Shows,
            vec![folder.clone()],
            recognisers(&["a"]),
        )
        .await
        .unwrap();

    let kept = libraries
        .update(
            created.id,
            "Shows",
            vec![folder.clone()],
            sources(&["tvdb"]),
        )
        .await
        .unwrap();
    assert_eq!(kept.recognisers, order(&["a"]));
    assert_eq!(
        libraries.get(created.id).await.unwrap().recognisers,
        order(&["a"])
    );

    let cleared = libraries
        .update(created.id, "Shows", vec![folder], recognisers(&[]))
        .await
        .unwrap();
    assert_eq!(cleared.recognisers, Vec::<String>::new());
    let stored = libraries.get(created.id).await.unwrap();
    assert_eq!(stored.recognisers, Vec::<String>::new());
    assert_eq!(stored.title_sources, order(&["tvdb"]));
}

#[tokio::test]
async fn recording_the_recognisers_read_says_whether_they_changed_since_the_last_scan() {
    let (_data, _media, libraries, folder) = setup().await;
    let id = libraries
        .create("Shows", Kind::Shows, vec![folder])
        .await
        .unwrap()
        .id;
    let record = |read: &'static str| {
        (libraries.db).write(move |tx| Box::pin(Libraries::record_recognisers_read(tx, id, read)))
    };

    let none_yet = record("").await.unwrap();
    let listed = record("a,b").await.unwrap();
    let again = record("a,b").await.unwrap();
    let reordered = record("b,a").await.unwrap();

    assert_eq!(
        (none_yet, listed, again, reordered),
        (false, true, false, true)
    );
}
