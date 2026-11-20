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
    AddToWatchlistRequest, BrowseServiceClient, ExternalId, GetMovieRequest, GetShowRequest,
    LibraryKind, ListContinueWatchingRequest, ListEpisodesRequest, RemoveFromWatchlistRequest,
    ReportProgressRequest, TitleKind, WatchStateServiceClient, WatchlistServiceClient,
    continue_watching_entry::MovieOrEpisode,
};
use server::{
    TestServer,
    requests::title_id,
    tmdb::{ALIEN, DARK, DARK_TVDB, HEAT, UNLISTED},
};

const DARK_S01E01: &str = "Dark (2017)/Season 01/Dark - S01E01.mkv";
const HEAT_FILE: &str = "Heat (1995) {tmdb-949}/Heat (1995).mkv";
/// Alien's IMDb id, which its Match holds beside its TMDB id.
const ALIEN_IMDB: &str = "tt0078748";

/// A Server with Alien and Heat in a Movies Library and Dark in a Shows Library, and their ids.
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
        Self::matching_into(TestServer::builder().tmdb().start().await).await
    }

    /// `server` with Libraries Matching Alien, Heat and Dark added to it.
    async fn matching_into(server: TestServer) -> Self {
        let movies = LibraryKind::LIBRARY_KIND_MOVIES;
        let movies = (server.matched_library(movies, &[ALIEN_1080P, HEAT_FILE])).await;
        let shows = (server.matched_library(LibraryKind::LIBRARY_KIND_SHOWS, &[DARK_S01E01])).await;
        Self {
            movies: movies.id,
            shows: shows.id,
            server,
        }
    }

    /// Whether Alien and Dark are on the Watchlist of whoever `who` calls as.
    async fn on_watchlist(&self, who: ClientConfig) -> (bool, bool) {
        let alien = self.movie_on_watchlist(who.clone(), ALIEN).await;
        let browse = BrowseServiceClient::new(HttpClient::plaintext(), who);
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
        (alien, show.on_watchlist)
    }

    /// Whether the Movie with the TMDB id `tmdb` is on the Watchlist of whoever `who` calls as.
    async fn movie_on_watchlist(&self, who: ClientConfig, tmdb: i64) -> bool {
        let browse = BrowseServiceClient::new(HttpClient::plaintext(), who);
        let asked = GetMovieRequest {
            library_id: self.movies,
            movie_id: tmdb_id(tmdb).into(),
            ..Default::default()
        };
        let movie = browse.get_movie(asked).await.unwrap().into_owned().movie;
        movie.on_watchlist
    }

    /// The Admin's Watchlist.
    fn watchlist(&self) -> WatchlistServiceClient<HttpClient> {
        WatchlistServiceClient::new(HttpClient::plaintext(), self.server.client_config())
    }
}

/// Adds the title of `kind` with the id `id` to the Watchlist `watchlist` is of.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn add(watchlist: &WatchlistServiceClient<HttpClient>, kind: TitleKind, id: ExternalId) {
    watchlist
        .add_to_watchlist(AddToWatchlistRequest {
            kind: kind.into(),
            id: id.into(),
            ..Default::default()
        })
        .await
        .unwrap();
}

/// Removes the title of `kind` with the id `id` from the Watchlist `watchlist` is of.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn remove(watchlist: &WatchlistServiceClient<HttpClient>, kind: TitleKind, id: ExternalId) {
    watchlist
        .remove_from_watchlist(RemoveFromWatchlistRequest {
            kind: kind.into(),
            id: id.into(),
            ..Default::default()
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn adding_a_movie_and_a_show_puts_them_on_the_callers_watchlist_only() {
    let fx = Fixture::start().await;
    let bea = fx.server.user_with_default_permissions("bea").await;

    add(&fx.watchlist(), TitleKind::TITLE_KIND_MOVIE, tmdb_id(ALIEN)).await;
    add(&fx.watchlist(), TitleKind::TITLE_KIND_SHOW, tmdb_id(DARK)).await;

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

    add(&watchlist, TitleKind::TITLE_KIND_MOVIE, tmdb_id(ALIEN)).await;
    add(&watchlist, TitleKind::TITLE_KIND_MOVIE, tmdb_id(ALIEN)).await;
    remove(&watchlist, TitleKind::TITLE_KIND_SHOW, tmdb_id(DARK)).await;
    assert_eq!(
        fx.on_watchlist(fx.server.client_config()).await,
        (true, false)
    );

    remove(&watchlist, TitleKind::TITLE_KIND_MOVIE, tmdb_id(ALIEN)).await;
    remove(&watchlist, TitleKind::TITLE_KIND_MOVIE, tmdb_id(ALIEN)).await;
    assert_eq!(
        fx.on_watchlist(fx.server.client_config()).await,
        (false, false)
    );
}

#[tokio::test]
async fn a_title_added_under_one_of_its_ids_comes_off_under_another() {
    let fx = Fixture::start().await;
    let watchlist = fx.watchlist();
    add(
        &watchlist,
        TitleKind::TITLE_KIND_MOVIE,
        title_id("imdb", ALIEN_IMDB),
    )
    .await;
    add(
        &watchlist,
        TitleKind::TITLE_KIND_SHOW,
        title_id("tvdb", &DARK_TVDB.to_string()),
    )
    .await;
    assert_eq!(
        fx.on_watchlist(fx.server.client_config()).await,
        (true, true)
    );

    remove(&watchlist, TitleKind::TITLE_KIND_MOVIE, tmdb_id(ALIEN)).await;
    remove(&watchlist, TitleKind::TITLE_KIND_SHOW, tmdb_id(DARK)).await;

    assert_eq!(
        fx.on_watchlist(fx.server.client_config()).await,
        (false, false)
    );
}

#[tokio::test]
async fn a_title_added_while_missing_comes_off_under_the_id_it_is_matched_by() {
    let server = TestServer::builder().tmdb().start().await;
    let watchlist = WatchlistServiceClient::new(HttpClient::plaintext(), server.client_config());
    add(
        &watchlist,
        TitleKind::TITLE_KIND_MOVIE,
        title_id("imdb", ALIEN_IMDB),
    )
    .await;
    let fx = Fixture::matching_into(server).await;

    remove(&watchlist, TitleKind::TITLE_KIND_MOVIE, tmdb_id(ALIEN)).await;

    assert_eq!(
        fx.on_watchlist(fx.server.client_config()).await,
        (false, false)
    );
}

#[tokio::test]
async fn removing_keeps_other_titles_and_other_users_watchlists() {
    let fx = Fixture::start().await;
    let bea = fx.server.user_with_default_permissions("bea").await;
    let beas = WatchlistServiceClient::new(HttpClient::plaintext(), bea.client_config());
    add(
        &fx.watchlist(),
        TitleKind::TITLE_KIND_MOVIE,
        title_id("imdb", ALIEN_IMDB),
    )
    .await;
    add(&fx.watchlist(), TitleKind::TITLE_KIND_MOVIE, tmdb_id(HEAT)).await;
    add(&fx.watchlist(), TitleKind::TITLE_KIND_SHOW, tmdb_id(DARK)).await;
    add(
        &beas,
        TitleKind::TITLE_KIND_MOVIE,
        title_id("imdb", ALIEN_IMDB),
    )
    .await;

    remove(&fx.watchlist(), TitleKind::TITLE_KIND_MOVIE, tmdb_id(ALIEN)).await;

    let admin = fx.server.client_config();
    assert_eq!(fx.on_watchlist(admin.clone()).await, (false, true));
    assert!(fx.movie_on_watchlist(admin, HEAT).await);
    assert_eq!(fx.on_watchlist(bea.client_config()).await, (true, false));
}

#[tokio::test]
async fn adding_and_removing_a_title_the_server_doesnt_have_succeed() {
    let fx = Fixture::start().await;

    add(
        &fx.watchlist(),
        TitleKind::TITLE_KIND_MOVIE,
        tmdb_id(UNLISTED),
    )
    .await;
    remove(
        &fx.watchlist(),
        TitleKind::TITLE_KIND_MOVIE,
        tmdb_id(UNLISTED),
    )
    .await;
}

#[tokio::test]
async fn a_show_on_the_watchlist_doesnt_put_the_movie_with_its_id_on_it() {
    let fx = Fixture::start().await;

    add(&fx.watchlist(), TitleKind::TITLE_KIND_SHOW, tmdb_id(ALIEN)).await;

    assert_eq!(
        fx.on_watchlist(fx.server.client_config()).await,
        (false, false)
    );
}

#[tokio::test]
async fn continue_watching_says_its_movie_and_show_are_on_the_watchlist() {
    let fx = Fixture::start().await;
    let config = fx.server.client_config();
    let browse = BrowseServiceClient::new(HttpClient::plaintext(), config.clone());
    let alien = GetMovieRequest {
        library_id: fx.movies,
        movie_id: tmdb_id(ALIEN).into(),
        ..Default::default()
    };
    let movie = browse.get_movie(alien).await.unwrap().into_owned().movie;
    let dark_s01 = ListEpisodesRequest {
        library_id: fx.shows,
        show_id: tmdb_id(DARK).into(),
        season_number: 1,
        ..Default::default()
    };
    let episodes = browse.list_episodes(dark_s01).await.unwrap().into_owned();
    let watch_states = WatchStateServiceClient::new(HttpClient::plaintext(), config);
    for version_id in [movie.versions[0].id, episodes.episodes[0].versions[0].id] {
        let report = ReportProgressRequest {
            version_id,
            position_ms: 60_000,
            duration_ms: 6_000_000,
            ..Default::default()
        };
        watch_states.report_progress(report).await.unwrap();
    }

    add(&fx.watchlist(), TitleKind::TITLE_KIND_MOVIE, tmdb_id(ALIEN)).await;
    add(&fx.watchlist(), TitleKind::TITLE_KIND_SHOW, tmdb_id(DARK)).await;

    let asked = ListContinueWatchingRequest::default();
    let listed = browse
        .list_continue_watching(asked)
        .await
        .unwrap()
        .into_owned();
    let mut on = (listed.entries.iter())
        .map(|entry| match &entry.movie_or_episode {
            Some(MovieOrEpisode::Movie(started)) => ("movie", started.on_watchlist),
            Some(MovieOrEpisode::Episode(started)) => ("show", started.show.on_watchlist),
            None => panic!("an entry with neither a Movie nor an Episode: {entry:?}"),
        })
        .collect::<Vec<_>>();
    on.sort_unstable();
    assert_eq!(on, [("movie", true), ("show", true)]);
}
