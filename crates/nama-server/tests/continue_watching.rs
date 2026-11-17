//! Continue Watching a page at a time (ADR 0055), over a real socket with the TMDB Plugin enabled
//! against a fake TMDB.

#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/browsing.rs"]
mod browsing;
#[path = "support/refused.rs"]
mod refused;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use std::time::Duration;

use browsing::{ALIEN_1080P, Fixture, tmdb_id};
use connectrpc::{ConnectError, ErrorCode, client::HttpClient};
use nama_proto::nama::v1::{
    ExternalId, LibraryKind, ListContinueWatchingRequest, ListContinueWatchingResponse,
    PageRequest, RefusalReason, ReportProgressRequest, WatchStateServiceClient,
    continue_watching_entry::MovieOrEpisode,
};
use refused::assert_refused;
use server::{
    TestServer,
    tmdb::{ALIEN, DUNE, HEAT},
};

impl Fixture {
    /// The page of at most `size` entries of Continue Watching after the one that returned
    /// `token`, or the first for an empty one.
    async fn page(
        &self,
        size: i32,
        token: &str,
    ) -> Result<ListContinueWatchingResponse, ConnectError> {
        let page = PageRequest {
            size,
            token: token.to_owned(),
            ..Default::default()
        };
        let asked = ListContinueWatchingRequest {
            page: page.into(),
            ..Default::default()
        };
        Ok(self
            .browse
            .list_continue_watching(asked)
            .await?
            .into_owned())
    }
}

/// The Movie of each entry.
fn movies(page: &ListContinueWatchingResponse) -> Vec<Option<&ExternalId>> {
    (page.entries.iter())
        .map(|entry| match &entry.movie_or_episode {
            Some(MovieOrEpisode::Movie(movie)) => movie.id.as_option(),
            _ => None,
        })
        .collect()
}

/// A Library of Alien, Heat and Dune, started in that order, then Heat again two seconds later:
/// Heat is the most recent by time, Dune by row, and Alien by neither.
async fn three_started_movies() -> Fixture {
    let builder = TestServer::builder().moves_clock();
    let fx = Fixture::start_on(LibraryKind::LIBRARY_KIND_MOVIES, builder).await;
    fx.write(ALIEN_1080P, b"video");
    fx.write("Heat (1995) {tmdb-949}/Heat (1995).mkv", b"video");
    fx.write("Dune (2021)/Dune (2021).mkv", b"video");
    fx.scan_and_match().await;
    for movie in [ALIEN, HEAT, DUNE] {
        start(&fx, movie).await;
    }
    fx.server.advance_clock(Duration::from_secs(2));
    start(&fx, HEAT).await;
    fx
}

/// Reports the Movie with the TMDB id `tmdb` a minute in.
#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "a test helper fails its test by panicking"
)]
async fn start(fx: &Fixture, tmdb: i64) {
    let report = ReportProgressRequest {
        version_id: fx.version_ids(tmdb).await[0],
        position_ms: 60_000,
        duration_ms: 7_200_000,
        ..Default::default()
    };
    WatchStateServiceClient::new(HttpClient::plaintext(), fx.server.client_config())
        .report_progress(report)
        .await
        .unwrap();
}

#[tokio::test]
async fn more_in_progress_than_one_page_comes_in_pages_most_recent_first() {
    let fx = three_started_movies().await;

    let first = fx.page(2, "").await.unwrap();
    let second = fx.page(2, &first.page.next_token).await.unwrap();

    let (alien, heat, dune) = (tmdb_id(ALIEN), tmdb_id(HEAT), tmdb_id(DUNE));
    assert_eq!(movies(&first), [Some(&heat), Some(&dune)]);
    assert_eq!(first.page.total_size, Some(3));
    assert_eq!(movies(&second), [Some(&alien)]);
    assert_eq!(
        second.page.total_size, None,
        "counted on the first page only"
    );
    assert_eq!(second.page.next_token, "", "the last page");
}

#[tokio::test]
async fn a_made_up_page_token_is_refused() {
    let fx = Fixture::alien().await;

    let refusal = assert_refused(
        fx.page(2, "made up").await,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_PAGE_TOKEN_INVALID,
    );

    assert_eq!(refusal.field, "page.token");
}
