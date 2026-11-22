use std::sync::Arc;

use nama_plugin_api::{
    automation::{Permission, functions::ListVersionTitles},
    contract::{VersionTitles, VersionTitlesAsked},
};

use super::super::testing::{
    ALIEN_TMDB_ID, LIES_TMDB_ID, SECRETS_TMDB_ID, episode_ref, kernel, movie_ref,
    two_episodes_in_one_file,
};
use crate::{
    db::Database,
    library::{
        Contents,
        fixture::{MatchedMovie, movies_library, tmdb_movie},
    },
};

fn asked(version_id: i64) -> VersionTitlesAsked {
    VersionTitlesAsked {
        version_id,
        ..VersionTitlesAsked::default()
    }
}

#[tokio::test]
async fn a_version_answers_its_movie_and_a_gone_one_nothing() {
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

    assert_eq!(titles.unwrap().titles, [movie_ref(library, ALIEN_TMDB_ID)]);
    assert_eq!(gone.unwrap(), VersionTitles::default());
}

#[tokio::test]
async fn a_file_of_two_episodes_answers_both_in_order() {
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

    assert_eq!(
        titles.unwrap().titles,
        [
            episode_ref(library, SECRETS_TMDB_ID),
            episode_ref(library, LIES_TMDB_ID)
        ]
    );
}
