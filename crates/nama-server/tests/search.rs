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
use buffa::EnumValue;
use connectrpc::{ErrorCode, client::HttpClient};
use nama_proto::nama::v1::{
    CreateRequestRequest, Done, Hit, Hits, LibraryKind, Permission, RefusalReason,
    RequestServiceClient, TitleKind, TitleStatus, hit, search_response::Part,
};
use refused::assert_refused;
use server::{
    TestServer,
    requests::Requester,
    search::search,
    tmdb::{ALIEN, DUNE, THE_OFFICE},
};

/// The Office (US) by an ID hint, since TMDB's search fits the UK one too.
const THE_OFFICE_US_S01E02: &str = "The Office {tmdb-2316}/Season 01/The Office - S01E02.mkv";

/// Dark's own Poster, as TMDB has it.
const DARK_POSTER: &str = "/media/artwork/tmdb/%2FapbrbWs8M9lyOpJYU5WXrpFbk1Z.jpg";

/// TMDB Matches it to the 2021 Dune.
const DUNE_2021: &str = "Dune (2021)/Dune (2021).mkv";

/// What the Admin's search for `query` of `kind` found in the Libraries' batch, best first.
#[expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "a test helper fails its test by panicking"
)]
async fn found(fx: &Fixture, query: &str, kind: TitleKind) -> Vec<hit::Title> {
    let parts = search(&fx.server.client_config(), query, kind).await;
    let Some(Part::Hits(hits)) = parts.unwrap().into_iter().next() else {
        panic!("the Libraries' batch first");
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
async fn a_search_by_one_who_may_not_request_streams_only_the_librarys_available_titles_then_done()
{
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

/// TMDB has a Still for Dark's "Secrets" and none for "Beginnings and Endings".
#[tokio::test]
async fn an_episode_is_found_with_its_still_and_its_shows_poster() {
    let fx = Fixture::start_on(LibraryKind::LIBRARY_KIND_SHOWS, TestServer::builder()).await;
    fx.write("Dark {tmdb-70523}/Season 01/Dark - S01E01.mkv", b"video");
    fx.write("Dark {tmdb-70523}/Season 02/Dark - S02E01.mkv", b"video");
    fx.scan_and_match().await;
    let shows = TitleKind::TITLE_KIND_SHOW;

    let secrets = found(&fx, "secrets", shows).await;
    let beginnings = found(&fx, "beginnings", shows).await;

    let artwork = |found: &[hit::Title]| match found {
        [hit::Title::Episode(episode)] => {
            (episode.still_url.clone(), episode.show_poster_url.clone())
        }
        _ => panic!("one Episode: {found:?}"),
    };
    let dark_poster = Some(DARK_POSTER.to_owned());
    let secrets_still = "/media/artwork/tmdb/%2FgNnJBvHy3VBkGrsl9ffpkf6ucP3.jpg";
    assert_eq!(
        artwork(&secrets),
        (Some(secrets_still.to_owned()), dark_poster.clone())
    );
    assert_eq!(artwork(&beginnings), (None, dark_poster));
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

/// Each Movie of `hits` by its title and status.
fn movies_and_statuses(hits: &Hits) -> Vec<(&str, EnumValue<TitleStatus>)> {
    (hits.hits.iter())
        .filter_map(|hit| match &hit.title {
            Some(hit::Title::Movie(movie)) => Some((movie.title.as_str(), hit.status)),
            _ => None,
        })
        .collect()
}

const AVAILABLE: TitleStatus = TitleStatus::TITLE_STATUS_AVAILABLE;
const REQUESTED: TitleStatus = TitleStatus::TITLE_STATUS_REQUESTED;
const NOT_ON_THIS_SERVER: TitleStatus = TitleStatus::TITLE_STATUS_NOT_ON_THIS_SERVER;

#[tokio::test]
async fn a_search_by_one_who_may_request_streams_the_movies_no_library_sent_after_the_librarys() {
    let fx = Fixture::alien().await;
    let user = fx.server.user("Ada", &[Permission::RequestMovie]).await;

    let parts = search(
        &user.client_config(),
        "Alien",
        TitleKind::TITLE_KIND_UNSPECIFIED,
    )
    .await
    .unwrap();

    let [
        Part::Hits(library),
        Part::Hits(to_request),
        Part::Done(done),
    ] = &parts[..]
    else {
        panic!("the Libraries' batch, the title source's, then Done: {parts:?}");
    };
    assert_eq!(movies_and_statuses(library), [("Alien", AVAILABLE.into())]);
    assert_eq!(
        movies_and_statuses(to_request),
        [
            ("Alien: Romulus", NOT_ON_THIS_SERVER.into()),
            ("Aliens", NOT_ON_THIS_SERVER.into())
        ]
    );
    assert_eq!(**done, Done::default());
}

#[tokio::test]
async fn a_movie_with_an_open_request_the_caller_can_see_is_requested_and_carries_it() {
    let fx = Fixture::start_on(LibraryKind::LIBRARY_KIND_MOVIES, TestServer::builder()).await;
    let ada = fx.server.user("Ada", &[Permission::RequestMovie]).await;
    let asked = RequestServiceClient::new(HttpClient::plaintext(), ada.client_config())
        .create_request(CreateRequestRequest {
            movie_id: tmdb_id(ALIEN).into(),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .request;

    let parts = search(&ada.client_config(), "Alien", TitleKind::TITLE_KIND_MOVIE)
        .await
        .unwrap();

    let [Part::Hits(library), Part::Hits(to_request), Part::Done(_)] = &parts[..] else {
        panic!("the Libraries' batch, the title source's, then Done: {parts:?}");
    };
    assert!(library.hits.is_empty(), "{library:?}");
    assert_eq!(
        movies_and_statuses(to_request),
        [
            ("Alien", REQUESTED.into()),
            ("Alien: Romulus", NOT_ON_THIS_SERVER.into()),
            ("Aliens", NOT_ON_THIS_SERVER.into())
        ]
    );
    let Some(hit::Title::Movie(alien)) = &to_request.hits[0].title else {
        panic!("Alien first: {to_request:?}");
    };
    assert_eq!(alien.request.id, asked.id);
    assert_eq!(alien.library_id, None);
}

#[tokio::test]
async fn a_movie_a_library_holds_that_its_search_missed_comes_from_the_title_source_as_available() {
    let fx = Fixture::alien().await;
    let user = fx.server.user("Ada", &[Permission::RequestMovie]).await;

    // Alien's French title, which the Libraries' search doesn't index.
    let parts = search(
        &user.client_config(),
        "Le huitième passager",
        TitleKind::TITLE_KIND_MOVIE,
    )
    .await
    .unwrap();

    let [Part::Hits(library), Part::Hits(to_request), Part::Done(_)] = &parts[..] else {
        panic!("the Libraries' batch, the title source's, then Done: {parts:?}");
    };
    assert!(library.hits.is_empty(), "{library:?}");
    assert_eq!(
        movies_and_statuses(to_request),
        [
            ("Alien", AVAILABLE.into()),
            ("Alien: Romulus", NOT_ON_THIS_SERVER.into()),
            ("Aliens", NOT_ON_THIS_SERVER.into())
        ]
    );
    let Some(hit::Title::Movie(alien)) = &to_request.hits[0].title else {
        panic!("Alien first: {to_request:?}");
    };
    assert_eq!(alien.library_id, Some(fx.library));
    assert_eq!(*alien.id, tmdb_id(ALIEN));
}

#[tokio::test]
async fn a_search_of_shows_by_one_who_may_request_only_movies_asks_the_title_source_for_nothing() {
    let fx = Fixture::alien().await;
    let ada = fx.server.user("Ada", &[Permission::RequestMovie]).await;

    let parts = search(
        &ada.client_config(),
        "The Office",
        TitleKind::TITLE_KIND_SHOW,
    )
    .await
    .unwrap();

    assert!(
        matches!(&parts[..], [Part::Hits(_), Part::Done(done)] if **done == Done::default()),
        "{parts:?}"
    );
}

#[tokio::test]
async fn with_the_title_source_down_a_search_streams_the_librarys_titles_then_done_flagging_it() {
    let mut fx = Fixture::alien().await;
    fx.server.stop_tmdb();

    let parts = search(
        &fx.server.client_config(),
        "Alien",
        TitleKind::TITLE_KIND_UNSPECIFIED,
    )
    .await
    .unwrap();

    let [Part::Hits(library), Part::Done(done)] = &parts[..] else {
        panic!("the Libraries' batch, then Done: {parts:?}");
    };
    assert_eq!(movies_and_statuses(library), [("Alien", AVAILABLE.into())]);
    assert!(done.title_source_unavailable);
}

#[tokio::test]
async fn with_no_title_source_enabled_a_search_flags_it_in_done() {
    let server = TestServer::builder().start().await;

    let parts = search(
        &server.client_config(),
        "Alien",
        TitleKind::TITLE_KIND_UNSPECIFIED,
    )
    .await
    .unwrap();

    let [Part::Hits(library), Part::Done(done)] = &parts[..] else {
        panic!("the Libraries' batch, then Done: {parts:?}");
    };
    assert!(library.hits.is_empty(), "{library:?}");
    assert!(done.title_source_unavailable);
}

/// The Office (UK), the other Show TMDB finds for "The Office".
const THE_OFFICE_UK: i64 = 2996;

/// Each Show of `hits` by its TMDB id and status.
fn shows_and_statuses(hits: &Hits) -> Vec<(&str, EnumValue<TitleStatus>)> {
    (hits.hits.iter())
        .filter_map(|hit| match &hit.title {
            Some(hit::Title::Show(show)) => Some((show.id.value.as_str(), hit.status)),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn a_show_with_an_open_season_request_the_caller_can_see_is_requested_and_carries_it() {
    let fx = Fixture::start_on(LibraryKind::LIBRARY_KIND_SHOWS, TestServer::builder()).await;
    let ada = fx.server.user("Ada", &[Permission::RequestTv]).await;
    let ada_requests = RequestServiceClient::new(HttpClient::plaintext(), ada.client_config());
    let asked = ada_requests.ask_seasons(THE_OFFICE_UK, &[1]).await;

    let parts = search(
        &ada.client_config(),
        "The Office",
        TitleKind::TITLE_KIND_SHOW,
    )
    .await
    .unwrap();

    let [
        Part::Hits(library),
        Part::Hits(to_request),
        Part::Done(done),
    ] = &parts[..]
    else {
        panic!("the Libraries' batch, the title source's, then Done: {parts:?}");
    };
    assert!(library.hits.is_empty(), "{library:?}");
    assert_eq!(
        shows_and_statuses(to_request),
        [
            ("2996", REQUESTED.into()),
            ("2316", NOT_ON_THIS_SERVER.into())
        ]
    );
    let Some(hit::Title::Show(uk)) = &to_request.hits[0].title else {
        panic!("the UK Office first: {to_request:?}");
    };
    assert_eq!(uk.request.id, asked[0].id);
    assert_eq!(uk.library_id, None);
    assert_eq!(**done, Done::default());
}

#[tokio::test]
async fn a_show_with_an_episode_in_a_library_is_available_and_found_once() {
    let fx = Fixture::start_on(LibraryKind::LIBRARY_KIND_SHOWS, TestServer::builder()).await;
    fx.write(THE_OFFICE_US_S01E02, b"video");
    fx.scan_and_match().await;
    let ada = fx.server.user("Ada", &[Permission::RequestTv]).await;

    let parts = search(
        &ada.client_config(),
        "The Office",
        TitleKind::TITLE_KIND_SHOW,
    )
    .await
    .unwrap();

    let [Part::Hits(library), Part::Hits(to_request), Part::Done(_)] = &parts[..] else {
        panic!("the Libraries' batch, the title source's, then Done: {parts:?}");
    };
    assert_eq!(shows_and_statuses(library), [("2316", AVAILABLE.into())]);
    assert_eq!(
        shows_and_statuses(to_request),
        [("2996", NOT_ON_THIS_SERVER.into())]
    );
}

#[tokio::test]
async fn a_show_a_library_holds_that_its_search_missed_comes_from_the_title_source_as_available() {
    let fx = Fixture::start_on(LibraryKind::LIBRARY_KIND_SHOWS, TestServer::builder()).await;
    fx.write(THE_OFFICE_US_S01E02, b"video");
    fx.scan_and_match().await;
    let ada = fx.server.user("Ada", &[Permission::RequestTv]).await;

    // A title the Libraries' search doesn't index, which the fake TMDB answers as "The Office".
    let parts = search(
        &ada.client_config(),
        "Bureau américain",
        TitleKind::TITLE_KIND_SHOW,
    )
    .await
    .unwrap();

    let [Part::Hits(library), Part::Hits(to_request), Part::Done(_)] = &parts[..] else {
        panic!("the Libraries' batch, the title source's, then Done: {parts:?}");
    };
    assert!(library.hits.is_empty(), "{library:?}");
    assert_eq!(
        shows_and_statuses(to_request),
        [
            ("2316", AVAILABLE.into()),
            ("2996", NOT_ON_THIS_SERVER.into())
        ]
    );
    let Some(hit::Title::Show(us)) = &to_request.hits[0].title else {
        panic!("the US Office first: {to_request:?}");
    };
    assert_eq!(us.library_id, Some(fx.library));
}

#[tokio::test]
async fn a_followed_show_without_an_open_season_request_is_requested() {
    let fx = Fixture::start_on(LibraryKind::LIBRARY_KIND_SHOWS, TestServer::builder()).await;
    let ada = fx.server.user("Ada", &[Permission::RequestTv]).await;
    let ada_requests = RequestServiceClient::new(HttpClient::plaintext(), ada.client_config());
    let (_, seasons) = ada_requests.follow(THE_OFFICE_UK).await;
    for season in seasons {
        ada_requests.withdraw(season.id).await;
    }

    let parts = search(
        &ada.client_config(),
        "The Office",
        TitleKind::TITLE_KIND_SHOW,
    )
    .await
    .unwrap();

    let [Part::Hits(_), Part::Hits(to_request), Part::Done(_)] = &parts[..] else {
        panic!("the Libraries' batch, the title source's, then Done: {parts:?}");
    };
    assert_eq!(
        shows_and_statuses(to_request),
        [
            ("2996", REQUESTED.into()),
            ("2316", NOT_ON_THIS_SERVER.into())
        ]
    );
    let Some(hit::Title::Show(uk)) = &to_request.hits[0].title else {
        panic!("the UK Office first: {to_request:?}");
    };
    assert!(uk.request.is_unset(), "{uk:?}");
}

#[tokio::test]
async fn a_show_whose_only_open_request_the_caller_cant_see_is_not_on_this_server() {
    let fx = Fixture::start_on(LibraryKind::LIBRARY_KIND_SHOWS, TestServer::builder()).await;
    let ada = fx.server.user("Ada", &[Permission::RequestTv]).await;
    let grace = fx.server.user("Grace", &[Permission::RequestTv]).await;
    RequestServiceClient::new(HttpClient::plaintext(), ada.client_config())
        .ask_seasons(THE_OFFICE_UK, &[1])
        .await;

    let parts = search(
        &grace.client_config(),
        "The Office",
        TitleKind::TITLE_KIND_SHOW,
    )
    .await
    .unwrap();

    let [Part::Hits(_), Part::Hits(to_request), Part::Done(_)] = &parts[..] else {
        panic!("the Libraries' batch, the title source's, then Done: {parts:?}");
    };
    assert_eq!(
        shows_and_statuses(to_request),
        [
            ("2316", NOT_ON_THIS_SERVER.into()),
            ("2996", NOT_ON_THIS_SERVER.into())
        ]
    );
}

#[tokio::test]
async fn when_the_show_search_fails_a_search_of_all_still_streams_the_movies_and_flags_it() {
    let fx = Fixture::alien().await;
    let ada = (fx.server)
        .user("Ada", &[Permission::RequestMovie, Permission::RequestTv])
        .await;

    // The fake TMDB finds Alien by this title, and answers a Show search for it with a page it
    // can't read.
    let parts = search(
        &ada.client_config(),
        "Le huitième passager",
        TitleKind::TITLE_KIND_UNSPECIFIED,
    )
    .await
    .unwrap();

    let [Part::Hits(_), Part::Hits(movies), Part::Done(done)] = &parts[..] else {
        panic!("the Libraries' batch, the Movies to request, then Done: {parts:?}");
    };
    assert_eq!(
        movies_and_statuses(movies),
        [
            ("Alien", AVAILABLE.into()),
            ("Alien: Romulus", NOT_ON_THIS_SERVER.into()),
            ("Aliens", NOT_ON_THIS_SERVER.into())
        ]
    );
    assert!(done.title_source_unavailable);
}

#[tokio::test]
async fn a_show_with_one_season_in_a_library_and_another_requested_is_available() {
    let fx = Fixture::start_on(LibraryKind::LIBRARY_KIND_SHOWS, TestServer::builder()).await;
    fx.write(THE_OFFICE_US_S01E02, b"video");
    fx.scan_and_match().await;
    let ada = fx.server.user("Ada", &[Permission::RequestTv]).await;
    RequestServiceClient::new(HttpClient::plaintext(), ada.client_config())
        .ask_seasons(THE_OFFICE, &[3])
        .await;

    // Found through the title source, where the Season Request could make it Requested.
    let parts = search(
        &ada.client_config(),
        "Bureau américain",
        TitleKind::TITLE_KIND_SHOW,
    )
    .await
    .unwrap();

    let [Part::Hits(_), Part::Hits(to_request), Part::Done(_)] = &parts[..] else {
        panic!("the Libraries' batch, the title source's, then Done: {parts:?}");
    };
    assert_eq!(
        shows_and_statuses(to_request),
        [
            ("2316", AVAILABLE.into()),
            ("2996", NOT_ON_THIS_SERVER.into())
        ]
    );
}

#[tokio::test]
async fn a_show_whose_only_followed_show_the_caller_cant_see_is_not_on_this_server() {
    let fx = Fixture::start_on(LibraryKind::LIBRARY_KIND_SHOWS, TestServer::builder()).await;
    let ada = fx.server.user("Ada", &[Permission::RequestTv]).await;
    let grace = fx.server.user("Grace", &[Permission::RequestTv]).await;
    let ada_requests = RequestServiceClient::new(HttpClient::plaintext(), ada.client_config());
    let (_, seasons) = ada_requests.follow(THE_OFFICE_UK).await;
    for season in seasons {
        ada_requests.withdraw(season.id).await;
    }

    let parts = search(
        &grace.client_config(),
        "The Office",
        TitleKind::TITLE_KIND_SHOW,
    )
    .await
    .unwrap();

    let [Part::Hits(_), Part::Hits(to_request), Part::Done(_)] = &parts[..] else {
        panic!("the Libraries' batch, the title source's, then Done: {parts:?}");
    };
    assert_eq!(
        shows_and_statuses(to_request),
        [
            ("2316", NOT_ON_THIS_SERVER.into()),
            ("2996", NOT_ON_THIS_SERVER.into())
        ]
    );
}
