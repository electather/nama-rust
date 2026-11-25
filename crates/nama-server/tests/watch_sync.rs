//! Watch sync through an Installed Plugin: the test Plugin built as a Watch sync, logging each
//! scrobble it is told with the User's `handle`.

#[path = "support/browsing.rs"]
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
mod browsing;
#[path = "support/home_row_plugin.rs"]
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
mod home_row_plugin;
#[path = "support/installed_test_plugin.rs"]
mod installed_test_plugin;
#[path = "support/log.rs"]
mod log;
#[path = "support/server.rs"]
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
mod server;
#[path = "support/test_plugin.rs"]
mod test_plugin;

use browsing::tmdb_id;
use connectrpc::client::HttpClient;
use home_row_plugin::set_handle;
use log::Log;
use nama_proto::nama::v1::{
    LibraryKind, SetPlayedRequest, WatchStateServiceClient, set_played_request::Target,
};
use server::{SignedIn, TestServer, tmdb::ALIEN};

/// Dark's S01E01, "Secrets", by its TMDB id.
const SECRETS: i64 = 1_388_538;

/// A Server with a Movies Library of Alien and a Shows Library of Dark's S01E01, and the test
/// Plugin built as a Watch sync, enabled; with their Library ids.
async fn with_watch_sync() -> (TestServer, i64, i64) {
    // Before the Server logs, since a line logged with no subscriber yet is never logged again.
    Log::global();
    let server = TestServer::builder().tmdb().start().await;
    let movies = (server)
        .matched_library(
            LibraryKind::LIBRARY_KIND_MOVIES,
            &["Alien (1979)/Alien (1979).mkv"],
        )
        .await;
    let shows = (server)
        .matched_library(
            LibraryKind::LIBRARY_KIND_SHOWS,
            &["Dark {tmdb-70523}/Season 01/Dark - S01E01.mkv"],
        )
        .await;
    server.enable_test_plugin("watch-sync").await;
    (server, movies.id, shows.id)
}

/// Marks `target` of the Library `library` played, as `user`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn mark_played(user: &SignedIn, library: i64, target: Target) {
    WatchStateServiceClient::new(HttpClient::plaintext(), user.client_config())
        .set_played(SetPlayedRequest {
            library_id: library,
            target: Some(target),
            played: true,
            ..Default::default()
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn marking_a_movie_and_an_episode_played_scrobbles_each_with_the_users_personal_settings() {
    let (server, movies, shows) = with_watch_sync().await;
    let ada = server.user_with_default_permissions("ada").await;
    set_handle(&ada, "ada").await;

    mark_played(&ada, movies, Target::MovieId(Box::new(tmdb_id(ALIEN)))).await;
    mark_played(&ada, shows, Target::EpisodeId(Box::new(tmdb_id(SECRETS)))).await;

    let log = Log::global();
    let (alien, secrets) = (format!("tmdb-{ALIEN}"), format!("tmdb-{SECRETS}"));
    let movie = ["test-plugin scrobbled ada's Movie", &alien, " played"];
    log.wait_for(&movie, 1).await;
    let episode = ["test-plugin scrobbled ada's Episode", &secrets, " played"];
    log.wait_for(&episode, 1).await;
}

#[tokio::test]
async fn a_user_who_set_no_personal_settings_is_not_scrobbled() {
    let (server, movies, _) = with_watch_sync().await;
    let bob = server.user_with_default_permissions("bob").await;
    let cy = server.user_with_default_permissions("cy").await;
    set_handle(&cy, "cy").await;

    mark_played(&bob, movies, Target::MovieId(Box::new(tmdb_id(ALIEN)))).await;
    // Cy's scrobble, sent after Bob's, has had the time Bob's would have taken.
    mark_played(&cy, movies, Target::MovieId(Box::new(tmdb_id(ALIEN)))).await;
    let log = Log::global();
    log.wait_for(&["test-plugin scrobbled cy's Movie"], 1).await;

    assert_eq!(log.count(&["test-plugin scrobbled nobody's"]), 0);
}
