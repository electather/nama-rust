use std::sync::Arc;

use nama_plugin_api::metadata::{ExternalId, ExternalIds};

use crate::{
    browse::{Browse, tests::ada},
    db::Database,
    library::{
        Contents, Libraries, Metadata,
        fixture::{MatchedMovie, movies_library, tmdb_movie},
    },
};

#[tokio::test]
async fn it_never_lists_more_than_five_titles() {
    let (data, folder) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let library = movies_library(&db, folder.path()).await;
    let root = folder.path().canonicalize().unwrap();
    let contents = Contents::new(Arc::clone(&db));
    for tmdb in 1..=6 {
        let path = root.join(format!("{tmdb}.mkv"));
        let matched = tmdb_movie(tmdb, &format!("Movie {tmdb}"));
        MatchedMovie::store(&contents, library, path.to_str().unwrap(), matched).await;
    }

    let featured = Browse::new(Arc::clone(&db))
        .featured(ada(&db).await)
        .await
        .unwrap();

    assert_eq!(featured.len(), 5, "six added just now");
}

#[tokio::test]
async fn a_title_two_libraries_know_by_different_ids_is_listed_once() {
    let data = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let contents = Contents::new(Arc::clone(&db));
    let ids = ExternalIds::from_iter([ExternalId::tmdb(1), ExternalId::tvdb(9)]);
    let folders = [tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap()];
    for (folder, sources) in folders.iter().zip([["tmdb", "tvdb"], ["tvdb", "tmdb"]]) {
        let library = movies_library(&db, folder.path()).await;
        let sources = sources.map(str::to_owned).to_vec();
        Libraries::new(Arc::clone(&db))
            .set_title_sources(library, sources)
            .await
            .unwrap();
        let path = folder.path().canonicalize().unwrap().join("Alien.mkv");
        let matched = Metadata {
            ids: ids.clone(),
            ..tmdb_movie(1, "Alien")
        };
        MatchedMovie::store(&contents, library, path.to_str().unwrap(), matched).await;
    }

    let featured = Browse::new(Arc::clone(&db))
        .featured(ada(&db).await)
        .await
        .unwrap();

    assert_eq!(
        featured.len(),
        1,
        "one Library knows it by TMDB's id, the other by TVDB's"
    );
}
