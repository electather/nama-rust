//! Searching every Library with `SearchService.Search` (ADR 0080), over a real socket with the
//! TMDB Plugin enabled against a fake TMDB.

#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/browsing.rs"]
mod browsing;
#[path = "support/refused.rs"]
mod refused;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use browsing::{Fixture, tmdb_id};
use connectrpc::ErrorCode;
use nama_proto::nama::v1::{
    Done, Hit, LibraryKind, RefusalReason, TitleKind, TitleStatus, hit, search_response::Part,
};
use refused::assert_refused;
use server::{
    TestServer,
    search::search,
    tmdb::{ALIEN, DUNE, THE_OFFICE},
};

/// The Office (US) by an ID hint, since TMDB's search fits the UK one too.
const THE_OFFICE_US_S01E02: &str = "The Office {tmdb-2316}/Season 01/The Office - S01E02.mkv";

/// TMDB Matches it to the 2021 Dune.
const DUNE_2021: &str = "Dune (2021)/Dune (2021).mkv";

/// What the Admin's search for `query` of `kind` found in its one batch, best first.
#[expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "a test helper fails its test by panicking"
)]
async fn found(fx: &Fixture, query: &str, kind: TitleKind) -> Vec<hit::Title> {
    let parts = search(&fx.server.client_config(), query, kind).await;
    let [Part::Hits(hits), Part::Done(_)] = &parts.unwrap()[..] else {
        panic!("one batch, then Done");
    };
    (hits.hits.iter())
        .filter_map(|hit| hit.title.clone())
        .collect()
}

/// Each of `found` by its kind and title.
fn kinds_and_titles(found: &[hit::Title]) -> Vec<(&str, &str)> {
    (found.iter())
        .map(|title| match title {
            hit::Title::Movie(movie) => ("movie", movie.title.as_str()),
            hit::Title::Show(show) => ("show", show.title.as_str()),
            hit::Title::Episode(episode) => ("episode", episode.title.as_str()),
        })
        .collect()
}

#[tokio::test]
async fn a_search_by_any_user_streams_one_batch_of_available_library_titles_then_done() {
    let fx = Fixture::alien().await;
    let user = fx.server.user("Grace", &[]).await;

    let parts = search(
        &fx.server.client_config_as(&user.token),
        "alien",
        TitleKind::TITLE_KIND_UNSPECIFIED,
    )
    .await
    .unwrap();

    let [Part::Hits(hits), Part::Done(done)] = &parts[..] else {
        panic!("one batch, then Done: {parts:?}");
    };
    assert_eq!(**done, Done::default());
    let [
        Hit {
            title: Some(hit::Title::Movie(alien)),
            status,
            ..
        },
    ] = &hits.hits[..]
    else {
        panic!("only Alien: {hits:?}");
    };
    assert_eq!(*status, TitleStatus::TITLE_STATUS_AVAILABLE);
    let alien = (
        alien.library_id,
        &*alien.id,
        alien.title.as_str(),
        alien.year,
    );
    assert_eq!(
        alien,
        (Some(fx.library), &tmdb_id(ALIEN), "Alien", Some(1979))
    );
}

#[tokio::test]
async fn a_search_of_one_kind_finds_movies_only_or_shows_with_their_episodes() {
    let fx = Fixture::alien().await;
    let office = (fx.server)
        .create_library(LibraryKind::LIBRARY_KIND_SHOWS, &[THE_OFFICE_US_S01E02])
        .await;
    fx.server.scan_and_match(&office).await;
    let movies = TitleKind::TITLE_KIND_MOVIE;
    let shows = TitleKind::TITLE_KIND_SHOW;

    let alien_movies = found(&fx, "alien", movies).await;
    let alien_shows = found(&fx, "alien", shows).await;
    let office_movies = found(&fx, "office", movies).await;
    let office_shows = found(&fx, "office", shows).await;
    let diversity_shows = found(&fx, "diversity", shows).await;

    assert_eq!(kinds_and_titles(&alien_movies), [("movie", "Alien")]);
    assert!(alien_shows.is_empty(), "{alien_shows:?}");
    assert!(office_movies.is_empty(), "{office_movies:?}");
    assert_eq!(kinds_and_titles(&office_shows), [("show", "The Office")]);
    assert_eq!(
        kinds_and_titles(&diversity_shows),
        [("episode", "Diversity Day")]
    );
}

#[tokio::test]
async fn a_search_finds_a_show_by_its_title_and_an_episode_with_its_show_and_season() {
    let fx = Fixture::start_on(LibraryKind::LIBRARY_KIND_SHOWS, TestServer::builder()).await;
    fx.write(THE_OFFICE_US_S01E02, b"video");
    fx.scan_and_match().await;
    let all = TitleKind::TITLE_KIND_UNSPECIFIED;

    let office = found(&fx, "office", all).await;
    let diversity = found(&fx, "diversity", all).await;

    let [hit::Title::Show(show)] = &office[..] else {
        panic!("only The Office: {office:?}");
    };
    let show = (show.library_id, &*show.id, show.title.as_str());
    let office_us = tmdb_id(THE_OFFICE);
    assert_eq!(show, (Some(fx.library), &office_us, "The Office"));
    let [hit::Title::Episode(episode)] = &diversity[..] else {
        panic!("only Diversity Day: {diversity:?}");
    };
    let numbers = (episode.season_number, episode.episode_number);
    let episode = (
        episode.library_id,
        &*episode.show_id,
        episode.show_title.as_str(),
        numbers,
        episode.title.as_str(),
    );
    assert_eq!(
        episode,
        (
            fx.library,
            &office_us,
            "The Office",
            (1, 2),
            "Diversity Day"
        )
    );
}

#[tokio::test]
async fn a_movie_a_scan_matches_is_found_right_after_it_and_not_once_its_file_is_removed() {
    let fx = Fixture::start_on(LibraryKind::LIBRARY_KIND_MOVIES, TestServer::builder()).await;
    fx.write(DUNE_2021, b"video");
    fx.scan_and_match().await;
    let all = TitleKind::TITLE_KIND_UNSPECIFIED;

    let found_dune = found(&fx, "dune 2021", all).await;

    let [hit::Title::Movie(dune)] = &found_dune[..] else {
        panic!("only Dune: {found_dune:?}");
    };
    let dune = (dune.library_id, &*dune.id, dune.title.as_str());
    assert_eq!(dune, (Some(fx.library), &tmdb_id(DUNE), "Dune"));
    fx.remove_file(DUNE_2021);
    fx.scan_and_match().await;
    assert_eq!(found(&fx, "dune", all).await, []);
}

#[tokio::test]
async fn a_blank_search_is_refused_naming_the_query() {
    let fx = Fixture::alien().await;

    let blank = search(
        &fx.server.client_config(),
        " ",
        TitleKind::TITLE_KIND_UNSPECIFIED,
    )
    .await;

    let refusal = assert_refused(
        blank,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
    );
    assert_eq!(refusal.field, "query");
}
