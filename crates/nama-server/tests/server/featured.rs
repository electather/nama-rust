//! Featured, the titles at the top of a User's home page (ADR 0072), as `ListFeatured` lists the
//! Kernel's suggestions and the test Plugin's, over a real socket with the TMDB Plugin enabled
//! against a fake TMDB.

use crate::support::{browsing, home_row_plugin, log, server};

use std::time::{Duration, Instant};

use browsing::{ALIEN_1080P, Fixture};
use connectrpc::client::{ClientConfig, HttpClient};
use home_row_plugin::{disable, enable, enable_granting, set_handle};
use log::Log;
use nama_proto::nama::v1::{
    AddToWatchlistRequest, BrowseServiceClient, FeaturedTitle, LibraryKind, ListEpisodesRequest,
    ListFeaturedRequest, PluginPermission, ReportProgressRequest, TitleKind,
    WatchStateServiceClient, WatchlistServiceClient, featured_title::MovieOrShow,
};
use server::{
    TestServer, WAIT,
    tmdb::{ALIEN, DARK, DUNE, HEAT},
};

/// Dark's Season 1 Episodes 1 and 2, in one file, and its Season 2 Episode 1. TMDB has three
/// Seasons of it.
const DARK_S01E01_E02: &str = "Dark (2017)/Season 01/Dark - S01E01-E02.mkv";
const DARK_S02E01: &str = "Dark {tmdb-70523}/Season 02/Dark - S02E01.mkv";
/// A special of Dark, Season 0, and an Episode of its Season 1.
const DARK_S00E01: &str = "Dark (2017)/Specials/Dark - S00E01.mkv";
const DARK_S01E03: &str = "Dark (2017)/Season 01/Dark - S01E03.mkv";

/// Past the 14 days a title counts as newly added.
const FIFTEEN_DAYS: Duration = Duration::from_hours(15 * 24);

/// The Featured titles of the User whose session `config` holds.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn featured(config: ClientConfig) -> Vec<FeaturedTitle> {
    BrowseServiceClient::new(HttpClient::plaintext(), config)
        .list_featured(ListFeaturedRequest::default())
        .await
        .unwrap()
        .into_owned()
        .titles
}

/// A Server with a Library of `kind` holding `files`, scanned and Matched.
async fn library_of(kind: LibraryKind, files: &[&str], builder: server::Builder) -> Fixture {
    let fx = Fixture::start_on(kind, builder).await;
    for file in files {
        fx.write(file, b"video");
    }
    fx.scan_and_match().await;
    fx
}

/// Reports, as the User whose session `config` holds, a minute into the Version `version`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn start(config: ClientConfig, version: i64) {
    let report = ReportProgressRequest {
        version_id: version,
        position_ms: 60_000,
        duration_ms: 3_600_000,
        ..Default::default()
    };
    WatchStateServiceClient::new(HttpClient::plaintext(), config)
        .report_progress(report)
        .await
        .unwrap();
}

/// The id of the Version of Dark's Season `season` Episode `number`.
#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "a test helper fails its test by panicking"
)]
async fn dark_version(fx: &Fixture, season: u32, number: u32) -> i64 {
    let asked = ListEpisodesRequest {
        library_id: fx.library,
        show_id: browsing::tmdb_id(DARK).into(),
        season_number: Some(season),
        ..Default::default()
    };
    let episodes = fx.browse.list_episodes(asked).await.unwrap().into_owned();
    let episode = (episodes.episodes.iter())
        .find(|episode| episode.episode_number == number)
        .unwrap();
    episode.versions[0].id
}

/// Each title's name, the Movie's or the Show's.
fn titles(featured: &[FeaturedTitle]) -> Vec<&str> {
    (featured.iter())
        .map(|title| match &title.movie_or_show {
            Some(MovieOrShow::Movie(movie)) => movie.title.as_str(),
            Some(MovieOrShow::Show(show)) => show.title.as_str(),
            None => "",
        })
        .collect()
}

#[tokio::test]
async fn with_nothing_started_and_nothing_added_in_14_days_it_lists_nothing() {
    let fx = Fixture::alien_on(TestServer::builder().moves_clock()).await;
    let just_added = featured(fx.server.client_config()).await;

    fx.server.advance_clock(FIFTEEN_DAYS);

    assert_eq!(titles(&just_added), ["Alien"]);
    assert_eq!(featured(fx.server.client_config()).await, []);
}

#[tokio::test]
async fn a_started_movie_outranks_one_added_today_and_is_listed_once() {
    let files = [ALIEN_1080P, "Heat (1995) {tmdb-949}/Heat (1995).mkv"];
    let fx = library_of(
        LibraryKind::LIBRARY_KIND_MOVIES,
        &files,
        TestServer::builder(),
    )
    .await;
    start(fx.server.client_config(), fx.version_ids(HEAT).await[0]).await;

    let featured = featured(fx.server.client_config()).await;

    assert_eq!(titles(&featured), ["Heat", "Alien"]);
    let Some(MovieOrShow::Movie(heat)) = &featured[0].movie_or_show else {
        panic!("Heat is a Movie");
    };
    assert_eq!(heat.watch_state.position_ms, 60_000);
    assert_eq!(featured[1].library_id, fx.library);
    assert!(
        featured[0].next_episode.is_unset(),
        "a Movie has no Episode"
    );
}

#[tokio::test]
async fn a_started_show_comes_with_the_episode_resume_starts_and_its_seasons_in_the_library() {
    let files = [DARK_S01E01_E02, DARK_S02E01];
    let fx = library_of(
        LibraryKind::LIBRARY_KIND_SHOWS,
        &files,
        TestServer::builder(),
    )
    .await;
    start(fx.server.client_config(), dark_version(&fx, 2, 1).await).await;

    let featured = featured(fx.server.client_config()).await;

    assert_eq!(titles(&featured), ["Dark"]);
    let Some(MovieOrShow::Show(dark)) = &featured[0].movie_or_show else {
        panic!("Dark is a Show");
    };
    assert_eq!(dark.season_count, 2, "Seasons 1 and 2, not TMDB's third");
    let next = featured[0].next_episode.as_option().unwrap();
    assert_eq!((next.season_number, next.episode_number), (2, 1));
    assert_eq!(next.watch_state.position_ms, 60_000);
}

#[tokio::test]
async fn a_show_nobody_started_plays_its_first_episode_and_counts_no_specials() {
    let files = [DARK_S00E01, DARK_S01E03];
    let fx = library_of(
        LibraryKind::LIBRARY_KIND_SHOWS,
        &files,
        TestServer::builder(),
    )
    .await;

    let featured = featured(fx.server.client_config()).await;

    let Some(MovieOrShow::Show(dark)) = &featured[0].movie_or_show else {
        panic!("Dark is a Show");
    };
    assert_eq!(dark.season_count, 1, "the Specials aren't a Season");
    let next = featured[0].next_episode.as_option().unwrap();
    assert_eq!((next.season_number, next.episode_number), (1, 3));
    assert!(next.watch_state.is_unset());
}

#[tokio::test]
async fn a_title_another_user_started_is_not_listed_for_the_caller() {
    let fx = Fixture::alien_on(TestServer::builder().moves_clock()).await;
    let grace = fx.server.user_with_default_permissions("Grace").await;
    fx.server.advance_clock(FIFTEEN_DAYS);

    start(fx.server.client_config(), fx.version_ids(ALIEN).await[0]).await;

    assert_eq!(
        titles(&featured(fx.server.client_config()).await),
        ["Alien"]
    );
    assert_eq!(featured(grace.client_config()).await, []);
}

/// The names of the Featured titles of the User whose session `config` holds, once they include
/// `title`, which a refresh in the background stores.
async fn featured_once_it_lists(config: ClientConfig, title: &str) -> Vec<String> {
    let started = Instant::now();
    loop {
        let featured = featured(config.clone()).await;
        let names: Vec<String> = titles(&featured).into_iter().map(str::to_owned).collect();
        if names.iter().any(|name| name == title) {
            return names;
        }
        assert!(started.elapsed() < WAIT, "{title} never listed: {names:?}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn a_plugins_suggestions_rank_by_weight_among_the_kernels_for_their_user_alone() {
    let files = [
        ALIEN_1080P,
        "Heat (1995) {tmdb-949}/Heat (1995).mkv",
        "Dune (2021) {tmdb-438631}/Dune (2021).mkv",
    ];
    let fx = library_of(
        LibraryKind::LIBRARY_KIND_MOVIES,
        &files,
        TestServer::builder().moves_clock(),
    )
    .await;
    let ada = fx.server.user_with_default_permissions("Ada").await;
    let bea = fx.server.user_with_default_permissions("Bea").await;
    fx.server.install_test_plugin("home-rows").await;
    enable(&fx.server).await;
    // For any handle, the test Plugin suggests Heat at 1.0, Alien at 0.0 and a Movie no title
    // source has at 1.0.
    set_handle(&ada, "ada").await;
    fx.server.advance_clock(FIFTEEN_DAYS);
    start(ada.client_config(), fx.version_ids(DUNE).await[0]).await;

    let adas = featured_once_it_lists(ada.client_config(), "Heat").await;

    assert_eq!(
        adas,
        ["Heat", "Dune", "Alien"],
        "Heat at 1.0 over Dune started at 0.8, Alien at 0.0 with fewer than 5 others, and the \
         Movie no Library has dropped"
    );
    assert_eq!(featured(bea.client_config()).await, [], "Bea set no handle");
}

#[tokio::test]
async fn disabling_the_plugin_drops_its_suggestions() {
    let files = [ALIEN_1080P, "Heat (1995) {tmdb-949}/Heat (1995).mkv"];
    let fx = library_of(
        LibraryKind::LIBRARY_KIND_MOVIES,
        &files,
        TestServer::builder().moves_clock(),
    )
    .await;
    fx.server.install_test_plugin("home-rows").await;
    enable(&fx.server).await;
    let ada = fx.server.user_with_default_permissions("Ada").await;
    set_handle(&ada, "ada").await;
    fx.server.advance_clock(FIFTEEN_DAYS);
    featured_once_it_lists(ada.client_config(), "Heat").await;

    disable(&fx.server).await;

    assert_eq!(featured(ada.client_config()).await, []);
}

#[tokio::test]
async fn starting_a_title_lists_a_plugin_reading_watch_states_old_suggestions_as_it_asks_again() {
    // Before the Server logs, since a line logged with no subscriber yet is never logged again.
    let log = Log::global();
    let files = [ALIEN_1080P, "Heat (1995) {tmdb-949}/Heat (1995).mkv"];
    let fx = library_of(
        LibraryKind::LIBRARY_KIND_MOVIES,
        &files,
        TestServer::builder().moves_clock(),
    )
    .await;
    fx.server.install_test_plugin("home-rows").await;
    let granted = [PluginPermission::PLUGIN_PERMISSION_READ_WATCH_STATES];
    enable_granting(&fx.server, &granted).await;
    // A handle of its own, as the log counts every Server's asks in this binary.
    let gus = fx.server.user_with_default_permissions("Gus").await;
    set_handle(&gus, "gus").await;
    fx.server.advance_clock(FIFTEEN_DAYS);
    featured_once_it_lists(gus.client_config(), "Heat").await;
    let asked = ["test-plugin suggests gus's Featured"];

    start(gus.client_config(), fx.version_ids(ALIEN).await[0]).await;

    let listed = featured(gus.client_config()).await;
    assert!(
        titles(&listed).contains(&"Heat"),
        "the Plugin's stale suggestions are still listed: {:?}",
        titles(&listed)
    );
    log.wait_for(&asked, 2).await;
}

#[tokio::test]
async fn a_title_on_the_callers_watchlist_says_so() {
    let fx = Fixture::alien_on(TestServer::builder()).await;
    let config = fx.server.client_config();
    let on_watchlist = |featured: &[FeaturedTitle]| match &featured[0].movie_or_show {
        Some(MovieOrShow::Movie(movie)) => movie.on_watchlist,
        _ => panic!("Featured lists Alien first"),
    };
    let before = featured(config.clone()).await;

    WatchlistServiceClient::new(HttpClient::plaintext(), config.clone())
        .add_to_watchlist(AddToWatchlistRequest {
            kind: TitleKind::TITLE_KIND_MOVIE.into(),
            id: browsing::tmdb_id(ALIEN).into(),
            ..Default::default()
        })
        .await
        .unwrap();

    assert!(!on_watchlist(&before));
    assert!(on_watchlist(&featured(config).await));
}
