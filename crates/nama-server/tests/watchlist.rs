//! The Watchlist (ADR 0056), over a real socket with the TMDB Plugin enabled against a fake TMDB.

#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/browsing.rs"]
mod browsing;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use browsing::{ALIEN_1080P, tmdb_id};
use connectrpc::client::{ClientConfig, HttpClient};
use nama_proto::nama::v1::{
    AddToWatchlistRequest, BrowseServiceClient, GetMovieRequest, GetShowRequest, LibraryKind,
    ListContinueWatchingRequest, RemoveFromWatchlistRequest, ReportProgressRequest, TitleKind,
    WatchStateServiceClient, WatchlistServiceClient, continue_watching_entry::MovieOrEpisode,
};
use server::{
    TestServer,
    tmdb::{ALIEN, DARK, UNLISTED},
};

const DARK_S01E01: &str = "Dark (2017)/Season 01/Dark - S01E01.mkv";

/// A Server with Alien in a Movies Library and Dark in a Shows Library, and their ids.
struct Fixture {
    server: TestServer,
    movies: i64,
    shows: i64,
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
impl Fixture {
    async fn start() -> Self {
        let server = TestServer::builder().tmdb().start().await;
        let movies =
            (server.matched_library(LibraryKind::LIBRARY_KIND_MOVIES, &[ALIEN_1080P])).await;
        let shows = (server.matched_library(LibraryKind::LIBRARY_KIND_SHOWS, &[DARK_S01E01])).await;
        Self {
            movies: movies.id,
            shows: shows.id,
            server,
        }
    }

    /// Whether Alien and Dark are on the Watchlist of whoever `who` calls as.
    async fn on_watchlist(&self, who: ClientConfig) -> (bool, bool) {
        let browse = BrowseServiceClient::new(HttpClient::plaintext(), who);
        let movie = browse
            .get_movie(GetMovieRequest {
                library_id: self.movies,
                movie_id: tmdb_id(ALIEN).into(),
                ..Default::default()
            })
            .await
            .unwrap()
            .into_owned()
            .movie;
        let show = browse
            .get_show(GetShowRequest {
                library_id: self.shows,
                show_id: tmdb_id(DARK).into(),
                ..Default::default()
            })
            .await
            .unwrap()
            .into_owned()
            .show;
        (movie.on_watchlist, show.on_watchlist)
    }

    /// The Admin's Watchlist.
    fn watchlist(&self) -> WatchlistServiceClient<HttpClient> {
        WatchlistServiceClient::new(HttpClient::plaintext(), self.server.client_config())
    }
}

/// Adds the title of `kind` with the TMDB id `tmdb` to the Watchlist `watchlist` is of.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn add(watchlist: &WatchlistServiceClient<HttpClient>, kind: TitleKind, tmdb: i64) {
    watchlist
        .add_to_watchlist(AddToWatchlistRequest {
            kind: kind.into(),
            id: tmdb_id(tmdb).into(),
            ..Default::default()
        })
        .await
        .unwrap();
}

/// Removes the title of `kind` with the TMDB id `tmdb` from the Watchlist `watchlist` is of.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn remove(watchlist: &WatchlistServiceClient<HttpClient>, kind: TitleKind, tmdb: i64) {
    watchlist
        .remove_from_watchlist(RemoveFromWatchlistRequest {
            kind: kind.into(),
            id: tmdb_id(tmdb).into(),
            ..Default::default()
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn adding_a_movie_and_a_show_puts_them_on_the_callers_watchlist_only() {
    let fx = Fixture::start().await;
    let bea = fx.server.user_with_default_permissions("bea").await;

    add(&fx.watchlist(), TitleKind::TITLE_KIND_MOVIE, ALIEN).await;
    add(&fx.watchlist(), TitleKind::TITLE_KIND_SHOW, DARK).await;

    assert_eq!(
        fx.on_watchlist(fx.server.client_config()).await,
        (true, true)
    );
    assert_eq!(fx.on_watchlist(bea.client_config()).await, (false, false));
}

#[tokio::test]
async fn adding_twice_and_removing_what_isnt_there_change_nothing() {
    let fx = Fixture::start().await;
    let watchlist = fx.watchlist();

    add(&watchlist, TitleKind::TITLE_KIND_MOVIE, ALIEN).await;
    add(&watchlist, TitleKind::TITLE_KIND_MOVIE, ALIEN).await;
    remove(&watchlist, TitleKind::TITLE_KIND_SHOW, DARK).await;
    assert_eq!(
        fx.on_watchlist(fx.server.client_config()).await,
        (true, false)
    );

    remove(&watchlist, TitleKind::TITLE_KIND_MOVIE, ALIEN).await;
    remove(&watchlist, TitleKind::TITLE_KIND_MOVIE, ALIEN).await;
    assert_eq!(
        fx.on_watchlist(fx.server.client_config()).await,
        (false, false)
    );
}

#[tokio::test]
async fn a_title_the_server_doesnt_have_can_be_added_and_removed() {
    let fx = Fixture::start().await;

    add(&fx.watchlist(), TitleKind::TITLE_KIND_MOVIE, UNLISTED).await;
    remove(&fx.watchlist(), TitleKind::TITLE_KIND_MOVIE, UNLISTED).await;
}

#[tokio::test]
async fn a_show_on_the_watchlist_doesnt_put_the_movie_with_its_id_on_it() {
    let fx = Fixture::start().await;

    add(&fx.watchlist(), TitleKind::TITLE_KIND_SHOW, ALIEN).await;

    assert_eq!(
        fx.on_watchlist(fx.server.client_config()).await,
        (false, false)
    );
}

#[tokio::test]
async fn continue_watching_says_its_movie_is_on_the_watchlist() {
    let fx = Fixture::start().await;
    let config = fx.server.client_config();
    let browse = BrowseServiceClient::new(HttpClient::plaintext(), config.clone());
    let alien = GetMovieRequest {
        library_id: fx.movies,
        movie_id: tmdb_id(ALIEN).into(),
        ..Default::default()
    };
    let movie = browse.get_movie(alien).await.unwrap().into_owned().movie;
    let report = ReportProgressRequest {
        version_id: movie.versions[0].id,
        position_ms: 60_000,
        duration_ms: 6_000_000,
        ..Default::default()
    };
    let watch_states = WatchStateServiceClient::new(HttpClient::plaintext(), config);
    watch_states.report_progress(report).await.unwrap();

    add(&fx.watchlist(), TitleKind::TITLE_KIND_MOVIE, ALIEN).await;

    let asked = ListContinueWatchingRequest::default();
    let listed = browse
        .list_continue_watching(asked)
        .await
        .unwrap()
        .into_owned();
    let Some(MovieOrEpisode::Movie(listed_movie)) = &listed.entries[0].movie_or_episode else {
        panic!("expected Alien, got {listed:?}");
    };
    assert!(listed_movie.on_watchlist);
}
