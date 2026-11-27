use std::sync::Arc;

use crate::{
    browse::{Browse, Browsing},
    content_ratings::MaximumAge,
    db::Database,
    library::{
        Found, Metadata,
        fixture::{MatchedMovie, tmdb_movie},
    },
    plugins::ExtensionPoints,
    users::{LibraryAccess, testing::reach_of},
};

#[tokio::test]
async fn search_leaves_out_every_copy_of_a_title_the_oldest_seen_librarys_rating_keeps_out() {
    let folders = [(); 3].map(|()| tempfile::tempdir().unwrap());
    let db = Arc::new(Database::open(folders[0].path()).await.unwrap());
    let rated = |rating: &str| Metadata {
        content_rating: Some(rating.to_owned()),
        ..tmdb_movie(348, "Alien")
    };
    MatchedMovie::new(&db, folders[1].path(), rated("R")).await;
    let newer = MatchedMovie::new(&db, folders[2].path(), rated("PG")).await;
    let every_title = Browsing::new(db, ExtensionPoints::default()).everything();
    let under_13 = |libraries| Browse {
        reach: reach_of(
            libraries,
            MaximumAge {
                age: Some(13),
                allow_unrated: true,
            },
        ),
        ..every_title.clone()
    };
    let only_newer = LibraryAccess::Only([newer.library].into());

    let every_library = under_13(LibraryAccess::All).search("Alien", None, 10).await;
    let newer_only = under_13(only_newer).search("Alien", None, 10).await;

    assert!(every_library.unwrap().is_empty(), "the older copy is R");
    let found: Vec<i64> = newer_only.unwrap().iter().map(Found::library).collect();
    assert_eq!(found, [newer.library]);
}
