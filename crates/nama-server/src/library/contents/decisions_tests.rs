use std::{collections::HashMap, sync::Arc, time::Duration};

use nama_proto::nama::v1::{
    PageRequest, SortDirection,
    UnmatchedItemSort::{self, UNMATCHED_ITEM_SORT_FOUND, UNMATCHED_ITEM_SORT_PATH},
};
use sqlx::{Connection, SqliteConnection, sqlite::SqliteConnectOptions};
use tempfile::TempDir;

use super::{
    super::{
        Change, Contents, Pin,
        fixture::tmdb_movie,
        testing::{ALIEN, Fixture, movie},
    },
    Decided,
};
use crate::{
    db::{self, Database},
    library::{METADATA_VERSION, libraries::Kind},
    page::Page,
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

/// The paths of `fx`'s Library's Unmatched Items, listed in `order` `size` at a time, page by
/// page, with the `total_size` each page carries.
async fn unmatched_pages(
    fx: &Fixture,
    order: (UnmatchedItemSort, SortDirection),
    size: i32,
) -> Vec<(Vec<String>, Option<i32>)> {
    let mut pages = Vec::new();
    let mut token = String::new();
    loop {
        let asked = PageRequest {
            size,
            token,
            ..Default::default()
        };
        let page = Page::read(&asked, b"unmatched").unwrap();
        let (items, paging) = fx
            .contents
            .unmatched(fx.library, order, &page)
            .await
            .unwrap();
        pages.push((
            items.into_iter().map(|item| item.path).collect(),
            paging.total_size,
        ));
        token = paging.next_token;
        if token.is_empty() {
            return pages;
        }
    }
}

#[tokio::test]
async fn more_unmatched_items_than_a_page_list_by_path_in_pages_counted_on_the_first() {
    let fx = Fixture::new().await;
    let paths: Vec<String> = (0..101)
        .map(|n| format!("/media/Film {n:03} (2000).mkv"))
        .collect();
    // Stored out of path order, so the list sorts them.
    for path in paths.iter().rev() {
        fx.store(path, 10, movie("Film", 2000)).await;
    }

    let order = (
        UNMATCHED_ITEM_SORT_PATH,
        SortDirection::SORT_DIRECTION_ASCENDING,
    );
    let pages = unmatched_pages(&fx, order, 100).await;

    assert_eq!(
        pages,
        [
            (paths[..100].to_vec(), Some(101)),
            (paths[100..].to_vec(), None)
        ]
    );
}

const HEAT: &str = "/media/Heat (1995).mkv";

/// The paths of `fx`'s Library's Unmatched Items, Alien's found before Heat's, listed one a page
/// in `sort`'s order descending.
async fn alien_then_heat_descending(sort: UnmatchedItemSort) -> Vec<(Vec<String>, Option<i32>)> {
    let fx = Fixture::new().await;
    fx.store(ALIEN, 10, movie("Alien", 1979)).await;
    db::advance_clock(Duration::from_secs(2));
    fx.store(HEAT, 10, movie("Heat", 1995)).await;
    unmatched_pages(&fx, (sort, SortDirection::SORT_DIRECTION_DESCENDING), 1).await
}

#[tokio::test]
async fn found_descending_lists_the_unmatched_item_a_scan_found_last_first() {
    let pages = alien_then_heat_descending(UNMATCHED_ITEM_SORT_FOUND).await;

    assert_eq!(
        pages,
        [
            (vec![HEAT.to_owned()], Some(2)),
            (vec![ALIEN.to_owned()], None)
        ]
    );
}

#[tokio::test]
async fn path_descending_lists_the_last_path_first() {
    let pages = alien_then_heat_descending(UNMATCHED_ITEM_SORT_PATH).await;

    assert_eq!(
        pages,
        [
            (vec![HEAT.to_owned()], Some(2)),
            (vec![ALIEN.to_owned()], None)
        ]
    );
}

#[tokio::test]
async fn items_of_one_file_found_together_list_one_a_page_in_every_order() {
    let fx = Fixture::new().await;
    // One file, so the three tie on both path and when they were found, and only ids part them.
    let change = Change::Store {
        path: ALIEN.to_owned(),
        size: 10,
        modified_ns: 0,
        items: vec![
            movie("Alien", 1979),
            movie("Aliens", 1986),
            movie("Alien 3", 1992),
        ],
    };
    fx.contents.apply(fx.library, vec![change]).await.unwrap();
    let one = || vec![ALIEN.to_owned()];

    for sort in [UNMATCHED_ITEM_SORT_PATH, UNMATCHED_ITEM_SORT_FOUND] {
        for direction in [
            SortDirection::SORT_DIRECTION_ASCENDING,
            SortDirection::SORT_DIRECTION_DESCENDING,
        ] {
            let pages = unmatched_pages(&fx, (sort, direction), 1).await;

            assert_eq!(
                pages,
                [(one(), Some(3)), (one(), None), (one(), None)],
                "{sort:?} {direction:?}"
            );
        }
    }
}

/// The version of the migration that adds `item_matches.titled_backdrop`.
const TITLED_BACKDROPS: i64 = 20_261_120_123_216;

#[tokio::test]
async fn upgrading_to_titled_backdrops_keeps_each_match_and_has_the_next_scan_fetch_it_again() {
    let data = tempfile::tempdir().unwrap();
    let options = SqliteConnectOptions::new()
        .filename(data.path().join(db::FILE_NAME))
        .create_if_missing(true);
    let mut conn = SqliteConnection::connect_with(&options).await.unwrap();
    let migrations = sqlx::migrate!();
    let before = (migrations.iter().map(|migration| migration.version))
        .filter(|&version| version < TITLED_BACKDROPS)
        .max();
    migrations.run_to(before.unwrap(), &mut conn).await.unwrap();
    // Matched at metadata version 4, the last before titled Backdrops.
    sqlx::raw_sql(
        "INSERT INTO libraries (id, name, kind) VALUES (1, 'Movies', 'movies');
         INSERT INTO library_files (id, library_id, path, size, modified_ns)
         VALUES (1, 1, '/media/Alien (1979).mkv', 10, 0);
         INSERT INTO library_items (id, file_id, kind, title, year)
         VALUES (1, 1, 'movie', 'Alien', 1979);
         INSERT INTO item_matches (item_id, state, plugin, title, backdrop, backdrop_plugin,
                                   decided_at, metadata_version)
         VALUES (1, 'matched', 'tmdb', 'Alien', '/textless.jpg', 'tmdb', 0, 4);
         INSERT INTO item_match_ids (item_id, scope, namespace, value)
         VALUES (1, 'title', 'tmdb', '348');",
    )
    .execute(&mut conn)
    .await
    .unwrap();
    conn.close().await.unwrap();

    let contents = Contents::new(Arc::new(Database::open(data.path()).await.unwrap()));
    let outdated = contents.matched_below(1, METADATA_VERSION).await.unwrap();
    let alien = contents.movies(1, None).await.unwrap().remove(0);

    let outdated: Vec<i64> = outdated.into_iter().map(|(item, ..)| item).collect();
    assert_eq!(outdated, [1]);
    let backdrop = alien.backdrop.map(|image| image.path);
    assert_eq!(backdrop.as_deref(), Some("/textless.jpg"));
    assert_eq!(alien.titled_backdrop, None);
}
