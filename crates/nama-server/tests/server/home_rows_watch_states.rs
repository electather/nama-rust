//! A User's Watch State changes make their own Home rows and Featured suggestions from a Plugin
//! granted Read Watch States stale, so their next visit to home asks it again: the test Plugin
//! built as a Home row source, its refreshes counted in the log.

use crate::support::{browsing, home_row_plugin, log, server};

use std::time::{Duration, Instant};

use browsing::{Fixture, tmdb_id};
use connectrpc::client::HttpClient;
use home_row_plugin::{disable, enable_granting, set_handle};
use log::Log;
use nama_proto::nama::v1::{
    BrowseServiceClient, ListFeaturedRequest, ListHomeRowsRequest, PluginPermission,
    ReportProgressRequest, SetPlayedRequest, WatchStateServiceClient, set_played_request::Target,
};
use server::{Builder, SignedIn, TestServer, WAIT, tmdb::ALIEN};

/// The shared rows the test Plugin logs giving when its `home_rows` setting is unset.
const SHARED: &str = "test-plugin gives Home rows \"\"";

/// Long enough for a refresh home started to call the Plugin.
const SETTLE: Duration = Duration::from_millis(500);

/// A Server with a Movies Library of Alien and the test Plugin built as a Home row source,
/// enabled and granted `granted`.
async fn with_plugin_granted(builder: Builder, granted: &[PluginPermission]) -> Fixture {
    // Before the Server logs, since a line logged with no subscriber yet is never logged again.
    Log::global();
    let fx = Fixture::alien_on(builder).await;
    fx.server.install_test_plugin("home-rows").await;
    enable_granting(&fx.server, granted).await;
    fx
}

/// [`with_plugin_granted`] Read Watch States.
async fn with_plugin_reading_watch_states(builder: Builder) -> Fixture {
    let granted = [PluginPermission::PLUGIN_PERMISSION_READ_WATCH_STATES];
    with_plugin_granted(builder, &granted).await
}

/// A User with the default Permissions, named and with the `handle` `name`.
async fn user(server: &TestServer, name: &str) -> SignedIn {
    let user = server.user_with_default_permissions(name).await;
    set_handle(&user, name).await;
    user
}

/// Opens home as `user`: their Home rows and Featured, each refreshing what is stale in the
/// background.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn open_home(user: &SignedIn) {
    let browse = BrowseServiceClient::new(HttpClient::plaintext(), user.client_config());
    browse
        .list_home_rows(ListHomeRowsRequest::default())
        .await
        .unwrap();
    browse
        .list_featured(ListFeaturedRequest::default())
        .await
        .unwrap();
}

/// How often the test Plugin has been asked for the rows, and for the Featured, of the User with
/// the `handle` `handle`.
fn asked(handle: &str) -> (usize, usize) {
    let log = Log::global();
    (
        log.count(&[&format!("test-plugin gives {handle}'s Home rows")]),
        log.count(&[&format!("test-plugin suggests {handle}'s Featured")]),
    )
}

/// Opens home as `user`, whose `handle` is `handle`, until the test Plugin has been asked for
/// their rows and their Featured `times` times each, since home starts no refresh while one runs.
async fn open_home_until_asked(user: &SignedIn, handle: &str, times: usize) {
    let started = Instant::now();
    loop {
        open_home(user).await;
        if asked(handle) == (times, times) {
            return;
        }
        assert!(
            started.elapsed() < WAIT,
            "asked {:?}, not {times} times each",
            asked(handle)
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// Reports, as `user`, `position_ms` into Alien's Version.
#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "a test helper fails its test by panicking"
)]
async fn watch_alien(fx: &Fixture, user: &SignedIn, position_ms: u64) {
    let report = ReportProgressRequest {
        version_id: fx.version_ids(ALIEN).await[0],
        position_ms,
        duration_ms: 7_000_000,
        ..Default::default()
    };
    WatchStateServiceClient::new(HttpClient::plaintext(), user.client_config())
        .report_progress(report)
        .await
        .unwrap();
}

/// Marks Alien, as `user`, played or unplayed.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn mark_alien(fx: &Fixture, user: &SignedIn, played: bool) {
    WatchStateServiceClient::new(HttpClient::plaintext(), user.client_config())
        .set_played(SetPlayedRequest {
            library_id: fx.library,
            target: Some(Target::MovieId(Box::new(tmdb_id(ALIEN)))),
            played,
            ..Default::default()
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn starting_playing_through_and_marking_a_title_played_or_unplayed_asks_the_plugin_again() {
    let fx = with_plugin_reading_watch_states(TestServer::builder()).await;
    let ada = user(&fx.server, "ada").await;
    open_home_until_asked(&ada, "ada", 1).await;

    watch_alien(&fx, &ada, 60_000).await;
    open_home_until_asked(&ada, "ada", 2).await;
    mark_alien(&fx, &ada, true).await;
    open_home_until_asked(&ada, "ada", 3).await;
    mark_alien(&fx, &ada, false).await;
    open_home_until_asked(&ada, "ada", 4).await;
    watch_alien(&fx, &ada, 60_000).await;
    open_home_until_asked(&ada, "ada", 5).await;
    // Past 90% of its duration, it becomes played.
    watch_alien(&fx, &ada, 6_900_000).await;

    open_home_until_asked(&ada, "ada", 6).await;
}

#[tokio::test]
async fn moving_the_position_alone_does_not_ask_the_plugin_again() {
    let fx = with_plugin_reading_watch_states(TestServer::builder()).await;
    let cy = user(&fx.server, "cy").await;
    watch_alien(&fx, &cy, 60_000).await;
    open_home_until_asked(&cy, "cy", 1).await;

    watch_alien(&fx, &cy, 120_000).await;
    open_home(&cy).await;
    tokio::time::sleep(SETTLE).await;

    assert_eq!(asked("cy"), (1, 1));
}

#[tokio::test]
async fn another_users_rows_and_the_shared_rows_are_not_asked_again() {
    let fx = with_plugin_reading_watch_states(TestServer::builder()).await;
    let dee = user(&fx.server, "dee").await;
    let eve = user(&fx.server, "eve").await;
    open_home_until_asked(&dee, "dee", 1).await;
    open_home_until_asked(&eve, "eve", 1).await;
    let shared = Log::global().count(&[SHARED]);

    watch_alien(&fx, &dee, 60_000).await;
    open_home(&eve).await;
    tokio::time::sleep(SETTLE).await;

    assert_eq!(asked("eve"), (1, 1));
    assert_eq!(Log::global().count(&[SHARED]), shared);
}

#[tokio::test]
async fn a_plugin_not_granted_read_watch_states_is_not_asked_again() {
    let fx = with_plugin_granted(TestServer::builder(), &[]).await;
    let fay = user(&fx.server, "fay").await;
    open_home_until_asked(&fay, "fay", 1).await;

    watch_alien(&fx, &fay, 60_000).await;
    open_home(&fay).await;
    tokio::time::sleep(SETTLE).await;

    assert_eq!(asked("fay"), (1, 1));
}

#[tokio::test]
async fn an_answer_to_a_refresh_running_as_the_title_was_started_is_not_kept_as_fresh() {
    let fx = with_plugin_reading_watch_states(TestServer::builder()).await;
    // The test Plugin answers the handle `waits` only once the clock moves a minute.
    let waits = user(&fx.server, "waits").await;
    open_home(&waits).await;
    Log::global()
        .wait_for(&["test-plugin gives waits's Home rows"], 1)
        .await;
    Log::global()
        .wait_for(&["test-plugin suggests waits's Featured"], 1)
        .await;

    watch_alien(&fx, &waits, 60_000).await;
    fx.server.advance_clock(Duration::from_mins(1));

    open_home_until_asked(&waits, "waits", 2).await;
    // Interrupts the waiting refreshes, which the runtime's drop would wait out to their time
    // limit.
    disable(&fx.server).await;
}
