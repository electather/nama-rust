use std::sync::Arc;

use nama_plugin_api::{
    PluginId,
    metadata::{ExternalId, ExternalIds},
};

use super::OnServer;
use crate::{
    db::Database,
    library::{
        Contents, Libraries, LibraryKind, Metadata,
        fixture::{MatchedMovie, tmdb_movie},
    },
};

/// The Library and id each of `found` is found by.
fn found_as(found: &[OnServer]) -> Vec<(i64, ExternalId)> {
    (found.iter())
        .map(|title| match title {
            OnServer::Movie(movie) => (movie.library, movie.id.clone()),
            OnServer::Show(show) => (show.library, show.id.clone()),
        })
        .collect()
}

#[tokio::test]
async fn titles_are_found_in_the_order_named_once_each_in_the_oldest_library() {
    let (data, older, newer) = (
        tempfile::tempdir().unwrap(),
        tempfile::tempdir().unwrap(),
        tempfile::tempdir().unwrap(),
    );
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let alien = MatchedMovie::new(&db, older.path(), tmdb_movie(348, "Alien")).await;
    let heat = MatchedMovie::new(&db, newer.path(), tmdb_movie(949, "Heat")).await;
    let contents = Contents::new(Arc::clone(&db));
    let alien_again = tmdb_movie(348, "Alien");
    MatchedMovie::store(&contents, heat.library, "/media/Alien.mkv", alien_again).await;
    let libraries = Libraries::new(db).list().await.unwrap();
    let named = [949, 1, 348, 949].map(ExternalId::tmdb);

    let found = OnServer::in_order_of(&contents, &libraries, LibraryKind::Movies, &named).await;

    assert_eq!(
        found_as(&found.unwrap()),
        [
            (heat.library, ExternalId::tmdb(949)),
            (alien.library, ExternalId::tmdb(348)),
        ]
    );
}

#[tokio::test]
async fn a_title_matched_under_another_namespace_is_found_by_any_id_it_shares() {
    let (data, media) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let imdb = ExternalId::new("imdb", "tt0078748").unwrap();
    let tvdb = ExternalId::tvdb(1_234);
    // No TMDB id, so the Library lists it under another of its ids than the one asked for.
    let by_another_source = Metadata {
        plugin: PluginId::from_static("another"),
        ids: ExternalIds::from_iter([imdb.clone(), tvdb.clone()]),
        ..tmdb_movie(348, "Alien")
    };
    let alien = MatchedMovie::new(&db, media.path(), by_another_source).await;
    let libraries = Libraries::new(Arc::clone(&db)).list().await.unwrap();
    let contents = Contents::new(db);

    let found = OnServer::in_order_of(&contents, &libraries, LibraryKind::Movies, &[tvdb]).await;

    assert_eq!(found_as(&found.unwrap()), [(alien.library, imdb)]);
}
