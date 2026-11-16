//! The fanart.tv Installed Plugin, bundled in the Server, driven end to end through the WASM
//! adapter against a fake fanart.tv: a Library that lists it after TMDB gets its Title Logo for a
//! Movie TMDB has none for.

#[path = "support/fake_fanart.rs"]
mod fake_fanart;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/http.rs"]
mod http;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use connectrpc::client::HttpClient;
use fake_fanart::FakeFanart;
use nama_proto::nama::v1::{BrowseServiceClient, LibraryKind, ListMoviesRequest, Movie};
use server::{TestServer, tmdb::HEAT};

/// Heat, tagged with its TMDB id since the fake TMDB finds no Heat by search; its TMDB images
/// have no logo.
const HEAT_FILE: &str = "Heat (1995) {tmdb-949}/Heat (1995).mkv";

/// Heat, as Browse lists it in a Movie Library searching `title_sources` with fanart.tv enabled.
#[expect(clippy::unwrap_used, reason = "a test fails by panicking")]
async fn heat_searching(title_sources: &[&str]) -> (TestServer, FakeFanart, Movie) {
    let server = TestServer::builder().tmdb().start().await;
    let fanart = FakeFanart::start().await;
    fanart.enable_on(&server).await;
    let library = server
        .create_library_searching(
            LibraryKind::LIBRARY_KIND_MOVIES,
            &[HEAT_FILE],
            title_sources,
        )
        .await;
    server.scan_and_match(&library).await;
    let movies = BrowseServiceClient::new(HttpClient::plaintext(), server.client_config())
        .list_movies(ListMoviesRequest {
            library_id: library.id,
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .movies;
    assert_eq!(movies.len(), 1, "{movies:?}");
    let heat = movies.into_iter().next().unwrap();
    assert_eq!(heat.id.value, HEAT.to_string());
    (server, fanart, heat)
}

#[tokio::test]
async fn a_library_listing_fanart_after_tmdb_gets_fanarts_title_logo_for_a_movie_tmdb_has_none_for()
{
    let (server, _fanart, heat) = heat_searching(&["tmdb", "fanart"]).await;

    let logo = heat.title_logo_url.expect("Heat has no Title Logo");
    assert!(logo.starts_with("/media/artwork/fanart/"), "{logo}");
    let fetched = http::fetch(&server, &logo, Some(&server.admin_token)).await;
    assert_eq!(fetched.status, 200);
    assert_eq!(fetched.body, fake_fanart::asset());
}

#[tokio::test]
async fn a_library_searching_tmdb_alone_has_no_title_logo_for_a_movie_tmdb_has_none_for() {
    let (_server, _fanart, heat) = heat_searching(&["tmdb"]).await;

    assert_eq!(heat.title_logo_url, None);
}
