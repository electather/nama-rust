//! A title's Similar titles in the Libraries (ADR 0081), on its page and its request page alike,
//! over a real socket with the TMDB Plugin enabled against a fake TMDB, where Alien's are Aliens
//! and Heat, and Dark's are Stranger Things and the US Office.

#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/browsing.rs"]
mod browsing;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use browsing::{ALIEN_1080P, tmdb_id};
use connectrpc::client::HttpClient;
use nama_proto::nama::v1::{
    BrowseServiceClient, GetMovieRequest, GetShowRequest, LibraryKind, LookUpMovieRequest,
    LookUpShowRequest, MovieHit, RequestServiceClient, ShowHit,
};
use server::{
    TestServer,
    tmdb::{ALIEN, DARK, HEAT, THE_OFFICE},
};

const HEAT_FILE: &str = "Heat (1995) {tmdb-949}/Heat (1995).mkv";
const DARK_S01E01: &str = "Dark (2017)/Season 01/Dark - S01E01.mkv";
/// The US Office, by its ID hint, since a search ties it with the UK one.
const THE_OFFICE_S01E01: &str = "The Office {tmdb-2316}/Season 01/The Office - S01E01.mkv";

/// A Server with Alien and Heat in a Movies Library and Dark and the US Office in a Shows Library,
/// browsed and looked up as the Admin.
struct Fixture {
    movies: i64,
    shows: i64,
    browse: BrowseServiceClient<HttpClient>,
    requests: RequestServiceClient<HttpClient>,
    _server: TestServer,
}

impl Fixture {
    async fn start() -> Self {
        let server = TestServer::builder().tmdb().start().await;
        let movies = (server
            .create_library(LibraryKind::LIBRARY_KIND_MOVIES, &[ALIEN_1080P, HEAT_FILE]))
        .await;
        server.scan_and_match(&movies).await;
        let files = [DARK_S01E01, THE_OFFICE_S01E01];
        let shows = server
            .create_library(LibraryKind::LIBRARY_KIND_SHOWS, &files)
            .await;
        server.scan_and_match(&shows).await;
        let config = server.client_config();
        Self {
            movies: movies.id,
            shows: shows.id,
            browse: BrowseServiceClient::new(HttpClient::plaintext(), config.clone()),
            requests: RequestServiceClient::new(HttpClient::plaintext(), config),
            _server: server,
        }
    }
}

/// The Library and TMDB id each of `hits` opens.
fn movies_opened(hits: &[MovieHit]) -> Vec<(Option<i64>, String)> {
    let opened = |hit: &MovieHit| (hit.library_id, hit.id.value.clone());
    hits.iter().map(opened).collect()
}

/// As [`movies_opened`], for Shows.
fn shows_opened(hits: &[ShowHit]) -> Vec<(Option<i64>, String)> {
    let opened = |hit: &ShowHit| (hit.library_id, hit.id.value.clone());
    hits.iter().map(opened).collect()
}

#[tokio::test]
async fn a_movies_page_and_request_page_show_its_similar_titles_in_a_library_alone() {
    let fx = Fixture::start().await;

    let page = fx.browse.get_movie(GetMovieRequest {
        library_id: fx.movies,
        movie_id: tmdb_id(ALIEN).into(),
        ..Default::default()
    });
    let page = page.await.unwrap().into_owned().similar;
    let looked_up = fx.requests.look_up_movie(LookUpMovieRequest {
        movie_id: tmdb_id(ALIEN).into(),
        ..Default::default()
    });
    let looked_up = looked_up.await.unwrap().into_owned().movie.into_option();

    // Aliens is in no Library, so only Heat shows.
    let heat = vec![(Some(fx.movies), HEAT.to_string())];
    assert_eq!(movies_opened(&page), heat);
    assert_eq!(page[0].title, "Heat");
    assert_eq!(movies_opened(&looked_up.unwrap().similar), heat);
}

#[tokio::test]
async fn a_shows_page_and_request_page_show_its_similar_titles_in_a_library_alone() {
    let fx = Fixture::start().await;

    let page = fx.browse.get_show(GetShowRequest {
        library_id: fx.shows,
        show_id: tmdb_id(DARK).into(),
        ..Default::default()
    });
    let page = page.await.unwrap().into_owned().similar;
    let looked_up = fx.requests.look_up_show(LookUpShowRequest {
        show_id: tmdb_id(DARK).into(),
        ..Default::default()
    });
    let looked_up = looked_up.await.unwrap().into_owned().show.into_option();

    // Stranger Things is in no Library, so only the Office shows.
    let the_office = vec![(Some(fx.shows), THE_OFFICE.to_string())];
    assert_eq!(shows_opened(&page), the_office);
    assert_eq!(shows_opened(&looked_up.unwrap().similar), the_office);
}
