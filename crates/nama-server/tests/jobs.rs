//! Jobs: the Kernel runs each Job of an enabled Plugin on its schedule, one Run at a time per Job
//! (ADR 0079).

#[path = "support/installed_test_plugin.rs"]
mod installed_test_plugin;
#[path = "support/log.rs"]
mod log;
#[path = "support/server.rs"]
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
mod server;
#[path = "support/test_plugin.rs"]
mod test_plugin;

use std::time::Duration;

use connectrpc::client::HttpClient;
use jiff::{ToSpan as _, Zoned};
use log::Log;
use nama_proto::nama::v1::{DisablePluginRequest, PluginServiceClient};
use server::TestServer;

const STARTED: &[&str] = &["test-plugin's Job wait started"];
const ENDED: &[&str] = &["test-plugin's Job wait ended"];
const DAY: Duration = Duration::from_hours(24);
/// Long enough for a firing loop that would fire to have run the Job.
const SETTLE: Duration = Duration::from_millis(500);

/// How long from now on the real clock, which a Server moving the clock starts on, until 04:00
/// next comes in the Server's time zone, when the test Plugin's `wait` is due.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn until_four() -> Duration {
    let now = Zoned::now();
    let today = now.with().hour(4).minute(0).second(0).subsec_nanosecond(0);
    let mut four = today.build().unwrap();
    if four <= now {
        four = four.checked_add(1.day()).unwrap();
    }
    now.duration_until(&four).unsigned_abs()
}

/// A Server moving the clock, with the test Plugin built with Jobs enabled.
async fn with_jobs() -> TestServer {
    let server = TestServer::builder().moves_clock().start().await;
    server.enable_test_plugin("jobs").await;
    server
}

#[tokio::test]
async fn a_job_fires_on_its_schedule_and_a_firing_due_while_it_runs_is_dropped() {
    let log = Log::global();
    let (started, ended) = (log.count(STARTED), log.count(ENDED));
    let server = with_jobs().await;

    server.advance_clock(until_four().saturating_sub(Duration::from_secs(5)));
    tokio::time::sleep(SETTLE).await;
    assert_eq!(log.count(STARTED), started, "it fired before 04:00");

    server.advance_clock(Duration::from_secs(5));
    log.wait_for(STARTED, started + 1).await;
    // The next 04:00 comes due while the Run waits for the clock to move.
    server.advance_clock(DAY);
    log.wait_for(ENDED, ended + 1).await;
    tokio::time::sleep(SETTLE).await;
    assert_eq!(
        log.count(STARTED),
        started + 1,
        "the overlapping firing ran"
    );

    server.advance_clock(DAY);
    log.wait_for(STARTED, started + 2).await;
    // Ends the Run, which the runtime's drop would otherwise wait out to its time limit.
    server.advance_clock(Duration::from_mins(1));
    log.wait_for(ENDED, ended + 2).await;
    assert_eq!(log.count(&["test-plugin's Job on-ask"]), 0);
}

#[tokio::test]
async fn disabling_the_plugin_stops_its_jobs() {
    let log = Log::global();
    let started = log.count(STARTED);
    let server = with_jobs().await;

    PluginServiceClient::new(HttpClient::plaintext(), server.client_config())
        .disable_plugin(DisablePluginRequest {
            id: "test-plugin".to_owned(),
            ..Default::default()
        })
        .await
        .unwrap();
    server.advance_clock(until_four() + Duration::from_secs(5));
    tokio::time::sleep(SETTLE).await;

    assert_eq!(log.count(STARTED), started);
}
