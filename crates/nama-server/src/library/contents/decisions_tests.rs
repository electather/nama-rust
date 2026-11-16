use std::collections::HashMap;

use tempfile::TempDir;

use super::{
    super::{
        Change, Contents, Pin,
        fixture::tmdb_movie,
        testing::{ALIEN, Fixture, movie},
    },
    Decided,
};
use crate::library::libraries::Kind;

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

/// Another Movies Library beside `fx`'s, on a folder of its own that lives as long as the
/// returned `TempDir`.
async fn other_library(fx: &Fixture) -> (TempDir, i64) {
    let folder = tempfile::tempdir().unwrap();
    let root = folder.path().canonicalize().unwrap();
    let library = (fx.libraries)
        .create(
            "Other",
            Kind::Movies,
            vec![root.to_str().unwrap().to_owned()],
        )
        .await
        .unwrap();
    (folder, library.id)
}

#[tokio::test]
async fn forgetting_unmatched_keeps_matches_pins_and_other_libraries_decisions() {
    let fx = Fixture::new().await;
    let (_folder, other) = other_library(&fx).await;
    let (heat, solaris) = ("/media/Heat (1995).mkv", "/media/Solaris (1972).mkv");
    for (path, title, year) in [
        (ALIEN, "Alien", 1979),
        (heat, "Heat", 1995),
        (solaris, "Solaris", 1972),
    ] {
        fx.store(path, 10, movie(title, year)).await;
    }
    let change = Change::Store {
        path: "/other/Alien (1979).mkv".to_owned(),
        size: 10,
        modified_ns: 0,
        items: vec![movie("Alien", 1979)],
    };
    fx.contents.apply(other, vec![change]).await.unwrap();
    let elsewhere = fx.contents.undecided(other).await.unwrap()[0].0;
    let (alien, pinned) = (fx.item(ALIEN).await, fx.item(heat).await);
    let matched = fx.item(solaris).await;
    let decided = vec![
        (alien, Decided::Unmatched(None)),
        (
            matched,
            Decided::Matched(Box::new(tmdb_movie(593, "Solaris"))),
        ),
        (elsewhere, Decided::Unmatched(None)),
    ];
    fx.contents.record(decided).await.unwrap();
    let named = fx.contents.named_like(pinned).await.unwrap().unwrap();
    let unmatched = vec![(pinned, Decided::Unmatched(None))];
    (fx.contents)
        .pin(named.library, named.name, Pin::Unmatched, unmatched)
        .await
        .unwrap();
    let library = fx.library;

    (fx.contents.db)
        .write(move |tx| Box::pin(Contents::forget_unmatched(tx, library)))
        .await
        .unwrap();

    assert_eq!(fx.undecided().await, [alien]);
    assert_eq!(fx.contents.undecided(other).await.unwrap(), []);
}
