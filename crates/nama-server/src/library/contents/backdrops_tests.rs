use std::time::Duration;

use crate::{
    db,
    library::contents::{
        Decided, ImageRef, Metadata,
        fixture::tmdb_movie,
        testing::{Fixture, movie},
    },
};

/// Stores a file at `path` holding one Movie, Matched to `tmdb` called `title`, with a Backdrop
/// named after it.
async fn store_with_backdrop(fx: &Fixture, path: &str, tmdb: i64, title: &str) {
    fx.store(path, 10, movie(title, 2000)).await;
    let item = fx.item(path).await;
    let metadata = Metadata {
        backdrop: ImageRef::stored(Some(format!("/{title}.jpg")), Some("tmdb".to_owned())),
        ..tmdb_movie(tmdb, title)
    };
    fx.contents
        .record(vec![(item, Decided::Matched(Box::new(metadata)))])
        .await
        .unwrap();
}

#[tokio::test]
async fn the_library_backdrop_is_the_most_recently_added_titles() {
    let fx = Fixture::new().await;
    store_with_backdrop(&fx, "/old.mkv", 1, "Old").await;
    db::advance_clock(Duration::from_secs(1));
    store_with_backdrop(&fx, "/new.mkv", 2, "New").await;
    db::advance_clock(Duration::from_secs(1));
    store_with_backdrop(&fx, "/old (copy).mkv", 1, "Old").await;

    let backdrops = fx.contents.newest_backdrops().await.unwrap();

    assert_eq!(backdrops[&fx.library].path, "/New.jpg");
}
