//! Matching through a Library's title sources, in its order, over a real socket, with the TMDB
//! Plugin enabled against a fake TMDB.

#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use connectrpc::client::HttpClient;
use nama_proto::nama::v1::{
    BrowseServiceClient, ExternalId, LibraryKind, LibraryServiceClient, ListMoviesRequest,
    UpdateLibraryRequest,
};
use server::{Library, TestServer};

/// A file naming a Movie the fake TMDB finds by searching.
const ALIEN: &str = "Alien (1979)/Alien (1979).mkv";

/// The ids of the Movies `library` lists.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn movies(server: &TestServer, library: &Library) -> Vec<ExternalId> {
    let browse = BrowseServiceClient::new(HttpClient::plaintext(), server.client_config());
    let movies = browse
        .list_movies(ListMoviesRequest {
            library_id: library.id,
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .movies;
    movies
        .into_iter()
        .filter_map(|movie| movie.id.into_option())
        .collect()
}

fn tmdb(id: i64) -> ExternalId {
    ExternalId {
        namespace: "tmdb".to_owned(),
        value: id.to_string(),
        ..Default::default()
    }
}

#[tokio::test]
async fn a_library_asks_only_its_own_title_sources() {
    let server = TestServer::builder().tmdb().start().await;
    let library = server
        .create_library_searching(LibraryKind::LIBRARY_KIND_MOVIES, &[ALIEN], &["tvdb"])
        .await;

    server.scan_and_match(&library).await;
    let unasked = (movies(&server, &library).await, server.tmdb().requests());
    let libraries = LibraryServiceClient::new(HttpClient::plaintext(), server.client_config());
    libraries
        .update_library(UpdateLibraryRequest {
            id: library.id,
            name: "Media".to_owned(),
            folders: vec![library.folder().to_str().unwrap().to_owned()],
            title_sources: vec!["tmdb".to_owned()],
            ..Default::default()
        })
        .await
        .unwrap();
    server.scan_and_match(&library).await;

    assert_eq!(unasked, (vec![], vec![]));
    assert_eq!(movies(&server, &library).await, [tmdb(348)]);
}

#[tokio::test]
async fn a_curly_tag_no_title_source_looks_up_by_is_searched_for() {
    let server = TestServer::builder().tmdb().start().await;
    let tagged = "Alien (1979) {x264-GROUP}/Alien (1979) {x264-GROUP}.mkv";

    let library = server
        .matched_library(LibraryKind::LIBRARY_KIND_MOVIES, &[tagged])
        .await;

    assert_eq!(movies(&server, &library).await, [tmdb(348)]);
}
