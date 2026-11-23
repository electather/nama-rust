use std::sync::Arc;

use nama_plugin_api::{
    automation::{Permission, functions::ListVersionTitles},
    contract::{VersionTitles, VersionTitlesAsked},
    fulfiller::TitleId,
};

use super::super::testing::{
    ALIEN_TMDB_ID, DARK_TMDB_ID, LIES_TMDB_ID, SECRETS_TMDB_ID, episode_ref, kernel, movie_ref,
    two_episodes_in_one_file,
};
use crate::{
    db::Database,
    library::{
        Change, Contents, Decided, Item,
        fixture::{self, MatchedMovie, movies_library, tmdb_movie},
    },
};

fn asked(version_id: i64) -> VersionTitlesAsked {
    VersionTitlesAsked {
        version_id,
        ..VersionTitlesAsked::default()
    }
}

fn tmdb(id: i64) -> TitleId {
    TitleId {
        namespace: "tmdb".to_owned(),
        value: id.to_string(),
        ..TitleId::default()
    }
}

#[tokio::test]
async fn a_version_answers_its_movie_with_no_show_and_a_gone_one_nothing() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let library = movies_library(&db, folder.path()).await;
    let path = folder.path().canonicalize().unwrap().join("alien.mkv");
    let alien = MatchedMovie::store(
        &Contents::new(Arc::clone(&db)),
        library,
        path.to_str().unwrap(),
        tmdb_movie(ALIEN_TMDB_ID, "Alien"),
    )
    .await;
    let (_automation, kernel) = kernel(&db, data.path(), Permission::PERMISSION_READ_LIBRARIES);

    let titles = kernel.ask::<ListVersionTitles>(asked(alien.version)).await;
    let gone = kernel
        .ask::<ListVersionTitles>(asked(alien.version + 1))
        .await;

    let titles = titles.unwrap();
    assert_eq!(titles.titles, [movie_ref(library, ALIEN_TMDB_ID)]);
    assert_eq!(titles.show_ids, [TitleId::default()]);
    assert_eq!(gone.unwrap(), VersionTitles::default());
}

#[tokio::test]
async fn a_file_of_two_episodes_answers_both_in_order_each_with_its_show() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let (library, path) = two_episodes_in_one_file(&db, folder.path()).await;
    let version = Contents::new(Arc::clone(&db))
        .stored(library)
        .await
        .unwrap()[&path]
        .id;
    let (_automation, kernel) = kernel(&db, data.path(), Permission::PERMISSION_READ_LIBRARIES);

    let titles = kernel.ask::<ListVersionTitles>(asked(version)).await;

    let titles = titles.unwrap();
    assert_eq!(
        titles.titles,
        [
            episode_ref(library, SECRETS_TMDB_ID),
            episode_ref(library, LIES_TMDB_ID)
        ]
    );
    assert_eq!(titles.show_ids, [tmdb(DARK_TMDB_ID), tmdb(DARK_TMDB_ID)]);
}

#[tokio::test]
async fn a_version_whose_item_is_unmatched_answers_nothing() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let library = movies_library(&db, folder.path()).await;
    let path = folder.path().canonicalize().unwrap().join("home video.mkv");
    let path = path.to_str().unwrap();
    let contents = Contents::new(Arc::clone(&db));
    let file = Change::Store {
        edition: None,
        path: path.to_owned(),
        size: 10,
        modified_ns: 0,
        items: vec![Item::Movie {
            title: "home video".to_owned(),
            year: None,
            hint: None,
        }],
    };
    contents.apply(library, vec![file]).await.unwrap();
    let decided = vec![Decided::Unmatched(None)];
    let (version, _items) = fixture::decide(&contents, library, path, decided).await;
    let (_automation, kernel) = kernel(&db, data.path(), Permission::PERMISSION_READ_LIBRARIES);

    let titles = kernel.ask::<ListVersionTitles>(asked(version)).await;

    assert_eq!(titles.unwrap(), VersionTitles::default());
}
