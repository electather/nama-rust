use std::sync::Arc;

use crate::{
    browse::{Browse, tests::ada},
    db::Database,
    library::{
        Contents,
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
