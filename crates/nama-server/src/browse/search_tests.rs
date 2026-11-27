use std::{path::Path, sync::Arc};

use nama_plugin_api::metadata::{ExternalId, ExternalIds};

use crate::{
    browse::{Browse, Browsing},
    content_ratings::MaximumAge,
    db::Database,
    library::{
        Change, Contents, Decided, Found, Item, Libraries, LibraryKind, Metadata, TitleKind,
        fixture::{MatchedMovie, decide, tmdb_movie},
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
    let only_newer = LibraryAccess::Only([newer.library].into());

    let every_library = under_13(&db, LibraryAccess::All)
        .search("Alien", None, 10)
        .await;
    let newer_only = under_13(&db, only_newer).search("Alien", None, 10).await;

    assert!(every_library.unwrap().is_empty(), "the older copy is R");
    let found: Vec<i64> = newer_only.unwrap().iter().map(Found::library).collect();
    assert_eq!(found, [newer.library]);
}

/// A Browse of `db` that sees everything.
fn admin(db: &Arc<Database>) -> Browse {
    Browsing::new(Arc::clone(db), ExtensionPoints::default()).everything()
}

/// A Browse of `db` for a User under 13 who sees `libraries`.
fn under_13(db: &Arc<Database>, libraries: LibraryAccess) -> Browse {
    let max_age = MaximumAge {
        age: Some(13),
        allow_unrated: true,
    };
    Browse {
        reach: reach_of(libraries, max_age),
        ..admin(db)
    }
}

/// The Library and title of each of `found`.
fn libraries_and_titles(found: &[Found]) -> Vec<(i64, &str)> {
    (found.iter())
        .map(|found| {
            let title = match found {
                Found::Movie(movie) => &movie.title,
                Found::Show(show) => &show.title,
                Found::Episode(episode) => &episode.title,
            };
            (found.library(), title.as_str())
        })
        .collect()
}

#[tokio::test]
async fn search_finds_a_title_both_copies_reach_once_in_the_oldest_library_and_fills_the_page() {
    let folders = [(); 3].map(|()| tempfile::tempdir().unwrap());
    let db = Arc::new(Database::open(folders[0].path()).await.unwrap());
    let rated = |tmdb, title: &str| Metadata {
        content_rating: Some("PG".to_owned()),
        ..tmdb_movie(tmdb, title)
    };
    let older = MatchedMovie::new(&db, folders[1].path(), rated(348, "Alien")).await;
    let newer = MatchedMovie::new(&db, folders[2].path(), rated(348, "Alien")).await;
    let path = folders[2]
        .path()
        .canonicalize()
        .unwrap()
        .join("Alien 3.mkv");
    let contents = Contents::new(Arc::clone(&db));
    let alien_3 = rated(8_077, "Alien 3");
    MatchedMovie::store(&contents, newer.library, path.to_str().unwrap(), alien_3).await;

    let as_admin = admin(&db).search("Alien", None, 2).await.unwrap();
    let as_under_13 = (under_13(&db, LibraryAccess::All))
        .search("Alien", None, 2)
        .await
        .unwrap();

    let expected = [(older.library, "Alien"), (newer.library, "Alien 3")];
    assert_eq!(libraries_and_titles(&as_admin), expected);
    assert_eq!(libraries_and_titles(&as_under_13), expected);
}

#[tokio::test]
async fn search_finds_a_shared_show_and_its_episodes_once_unless_the_oldest_copy_is_kept_out() {
    let folders = [(); 5].map(|()| tempfile::tempdir().unwrap());
    let db = Arc::new(Database::open(folders[0].path()).await.unwrap());
    let (dark, lost) = ((70_523, "Dark"), (4_607, "Lost"));
    let older_dark = rated_show(&db, folders[1].path(), dark, "PG").await;
    rated_show(&db, folders[2].path(), dark, "PG").await;
    rated_show(&db, folders[3].path(), lost, "R").await;
    rated_show(&db, folders[4].path(), lost, "PG").await;
    let browse = under_13(&db, LibraryAccess::All);

    let shows = browse.search("Dark", Some(TitleKind::Show), 10).await;
    let episodes = browse.search("Secrets", Some(TitleKind::Show), 10).await;
    let kept_out = browse.search("Lost", Some(TitleKind::Show), 10).await;

    assert_eq!(
        libraries_and_titles(&shows.unwrap()),
        [(older_dark, "Dark")]
    );
    // Lost's Episodes share the title, but the older Lost is R.
    assert_eq!(
        libraries_and_titles(&episodes.unwrap()),
        [(older_dark, "Secrets")]
    );
    assert!(kept_out.unwrap().is_empty(), "the older Lost is R");
}

/// Creates a Shows Library on `folder` holding Episode 1, called "Secrets", of the TMDB Show `show`
/// by id and title, rated `rating`. Returns its id.
async fn rated_show(db: &Arc<Database>, folder: &Path, show: (i64, &str), rating: &str) -> i64 {
    let (show_tmdb, show) = show;
    let root = folder.canonicalize().unwrap();
    let root = root.to_str().unwrap();
    let library = (Libraries::new(Arc::clone(db)))
        .create(root, LibraryKind::Shows, vec![root.to_owned()])
        .await
        .unwrap()
        .id;
    let path = format!("{root}/{show} - S01E01.mkv");
    let file = Change::Store {
        edition: None,
        path: path.clone(),
        size: 10,
        modified_ns: 0,
        items: vec![Item::Episode {
            show: show.to_owned(),
            year: None,
            hint: None,
            season: 1,
            episode: 1,
        }],
    };
    let contents = Contents::new(Arc::clone(db));
    contents.apply(library, vec![file]).await.unwrap();
    let episode = Metadata {
        show_ids: ExternalIds::from_iter([ExternalId::tmdb(show_tmdb)]),
        show_title: Some(show.to_owned()),
        content_rating: Some(rating.to_owned()),
        ..tmdb_movie(show_tmdb * 10, "Secrets")
    };
    decide(
        &contents,
        library,
        &path,
        vec![Decided::Matched(Box::new(episode))],
    )
    .await;
    library
}
