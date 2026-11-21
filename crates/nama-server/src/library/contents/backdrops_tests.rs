use std::time::Duration;

use nama_plugin_api::metadata::{ExternalId, ExternalIds};

use crate::{
    db,
    library::{
        contents::{
            Change, Decided, ImageRef, Item, Metadata,
            fixture::{decide, tmdb_movie},
            testing::{Fixture, movie},
        },
        libraries::Kind,
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

/// Stores, in the Shows Library `library`, Episode 1 of Season 1 at `path`, Matched to an
/// Episode with a Backdrop named after `title` of a Show with `show_ids`.
async fn store_episode(fx: &Fixture, library: i64, path: &str, title: &str, show_ids: ExternalIds) {
    let item = Item::Episode {
        show: title.to_owned(),
        year: None,
        hint: None,
        season: 1,
        episode: 1,
    };
    let change = Change::Store {
        path: path.to_owned(),
        size: 10,
        modified_ns: 0,
        items: vec![item],
    };
    fx.contents.apply(library, vec![change]).await.unwrap();
    let metadata = Metadata {
        backdrop: ImageRef::stored(Some(format!("/{title}.jpg")), Some("tmdb".to_owned())),
        show_ids,
        ..tmdb_movie(0, title)
    };
    decide(
        &fx.contents,
        library,
        path,
        vec![Decided::Matched(Box::new(metadata))],
    )
    .await;
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

#[tokio::test]
async fn the_library_backdrop_is_the_newest_listed_titles_when_the_newest_episode_has_no_show_id() {
    let fx = Fixture::new().await;
    let folder = tempfile::tempdir().unwrap();
    let root = folder.path().canonicalize().unwrap();
    let root = root.to_str().unwrap().to_owned();
    let shows = (fx.libraries.create("Shows", Kind::Shows, vec![root]).await)
        .unwrap()
        .id;
    let show = ExternalIds::from_iter([ExternalId::tmdb(10)]);
    store_episode(&fx, shows, "/listed.mkv", "Listed", show).await;
    db::advance_clock(Duration::from_secs(1));
    store_episode(
        &fx,
        shows,
        "/unlisted.mkv",
        "Unlisted",
        ExternalIds::default(),
    )
    .await;

    let backdrops = fx.contents.newest_backdrops().await.unwrap();

    assert_eq!(backdrops[&shows].path, "/Listed.jpg");
}

#[tokio::test]
async fn the_library_backdrop_is_the_newest_titles_first_versions() {
    let fx = Fixture::new().await;
    store_with_backdrop(&fx, "/a.mkv", 1, "First").await;
    db::advance_clock(Duration::from_secs(1));
    store_with_backdrop(&fx, "/b.mkv", 1, "Later").await;

    let backdrops = fx.contents.newest_backdrops().await.unwrap();

    assert_eq!(backdrops[&fx.library].path, "/First.jpg");
}
