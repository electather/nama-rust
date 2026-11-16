use std::collections::HashMap;

use super::{
    super::testing::{ALIEN, Fixture, movie},
    Decided,
};

#[tokio::test]
async fn undecided_leaves_out_decided_items() {
    let fx = Fixture::new().await;
    fx.store(ALIEN, 10, movie("Alien", 1979)).await;
    fx.store("/media/Heat (1995).mkv", 10, movie("Heat", 1995))
        .await;
    let alien = fx.item(ALIEN).await;
    let heat = fx.item("/media/Heat (1995).mkv").await;

    fx.contents
        .record(vec![(alien, Decided::Unmatched(None))])
        .await
        .unwrap();

    assert_eq!(fx.undecided().await, [heat]);
}

#[tokio::test]
async fn record_skips_an_item_deleted_since_it_was_read() {
    let fx = Fixture::new().await;
    fx.store(ALIEN, 10, movie("Alien", 1979)).await;
    let read = fx.undecided().await;
    // The file changes, which replaces its Item.
    fx.store(ALIEN, 20, movie("Alien", 1979)).await;
    let replacement = fx.item(ALIEN).await;

    let recorded = fx
        .contents
        .record(
            read.iter()
                .map(|&id| (id, Decided::Unmatched(None)))
                .collect::<Vec<_>>(),
        )
        .await;

    assert!(recorded.is_ok(), "{recorded:?}");
    assert_ne!(read, [replacement]);
    // The decision about the deleted Item didn't land on its replacement.
    assert_eq!(fx.undecided().await, [replacement]);
}

#[tokio::test]
async fn paths_of_undecided_gives_each_undecided_items_path() {
    let fx = Fixture::new().await;
    let heat = "/media/Heat (1995).mkv";
    fx.store(ALIEN, 10, movie("Alien", 1979)).await;
    fx.store(heat, 10, movie("Heat", 1995)).await;
    let alien = fx.item(ALIEN).await;
    fx.contents
        .record(vec![(alien, Decided::Unmatched(None))])
        .await
        .unwrap();

    let paths = fx.contents.paths_of_undecided(fx.library).await.unwrap();

    assert_eq!(
        paths,
        HashMap::from([(fx.item(heat).await, heat.to_owned())])
    );
}
