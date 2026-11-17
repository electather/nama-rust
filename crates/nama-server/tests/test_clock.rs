//! Moving the Server's clock ahead, as a test that ages a Watch State does, over a real socket with
//! the TMDB Plugin enabled against a fake TMDB.

#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/browsing.rs"]
mod browsing;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use browsing::Fixture;
use connectrpc::client::HttpClient;
use nama_proto::nama::v1::{
    BrowseServiceClient, ListContinueWatchingRequest, ReportProgressRequest,
    WatchStateServiceClient,
};
use server::{TestServer, tmdb::ALIEN};

const THIRTY_DAYS: Duration = Duration::from_hours(30 * 24);

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn unix_seconds() -> i64 {
    let since = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    i64::try_from(since.as_secs()).unwrap()
}

#[tokio::test]
async fn the_admin_is_still_signed_in_after_the_clock_moves_30_days() {
    let server = TestServer::builder().moves_clock().start().await;

    server.advance_clock(THIRTY_DAYS);

    BrowseServiceClient::new(HttpClient::plaintext(), server.client_config())
        .list_continue_watching(ListContinueWatchingRequest::default())
        .await
        .unwrap();
}

#[tokio::test]
async fn a_watch_state_reported_after_the_clock_moves_is_dated_by_the_moved_clock() {
    let fx = Fixture::alien_on(TestServer::builder().moves_clock()).await;
    let [version] = fx.version_ids(ALIEN).await[..] else {
        panic!("Alien has one Version");
    };
    let before = unix_seconds();

    fx.server.advance_clock(THIRTY_DAYS);
    WatchStateServiceClient::new(HttpClient::plaintext(), fx.server.client_config())
        .report_progress(ReportProgressRequest {
            version_id: version,
            position_ms: 60_000,
            duration_ms: 7_200_000,
            ..Default::default()
        })
        .await
        .unwrap();

    let state = fx
        .get_movie(fx.library, ALIEN)
        .await
        .unwrap()
        .watch_state
        .into_option()
        .unwrap();
    // At least: under `cargo test` this binary's other test may have moved the clock first. Once
    // #868 starts each clock-moving test on the real clock, bound it above again.
    let moved = i64::try_from(THIRTY_DAYS.as_secs()).unwrap();
    assert!(
        state.updated_at >= before + moved,
        "{} >= {}",
        state.updated_at,
        before + moved
    );
}

#[tokio::test]
#[should_panic(expected = "moves_clock()")]
async fn moving_the_clock_of_a_server_built_without_moves_clock_panics() {
    let server = TestServer::builder().start().await;

    server.advance_clock(THIRTY_DAYS);
}
