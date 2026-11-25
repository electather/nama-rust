//! Watch sync through an Installed Plugin: the test Plugin built as a Watch sync, logging each
//! scrobble it is told with the User's `handle`, and linking an account once its `approved`
//! setting names the device code polled.

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
use home_row_plugin::{PLUGIN, set_handle, text};
use log::Log;
use nama_proto::nama::v1::{
    DeletePersonalSettingsRequest, LibraryKind, ListPersonalSettingsRequest, PersonalSettings,
    PollWatchSyncLinkRequest, SetPlayedRequest, StartWatchSyncLinkRequest,
    UpdatePluginSettingsRequest, UserServiceClient, WatchStateServiceClient, WatchSyncLinkState,
    set_played_request::Target,
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

/// The test Plugin's Personal settings as `users`' caller lists them.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn listed(users: &UserServiceClient<HttpClient>) -> PersonalSettings {
    let listed = users.list_personal_settings(ListPersonalSettingsRequest::default());
    let listed = listed.await.unwrap().into_owned().plugins;
    listed
        .into_iter()
        .find(|it| it.plugin_id == PLUGIN)
        .unwrap()
}

/// Whether the test Plugin's secret `token` is set in `listed`.
fn token_set(listed: &PersonalSettings) -> bool {
    (listed.settings.iter()).any(|setting| setting.key == "token" && setting.secret_set)
}

/// Has the test Plugin's tracker approve `device_code`, as if its User entered the code there.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn approve(server: &TestServer, device_code: &str) {
    (server.plugin_service())
        .update_plugin_settings(UpdatePluginSettingsRequest {
            id: PLUGIN.to_owned(),
            settings: std::iter::once(("approved".to_owned(), text(device_code))).collect(),
            ..Default::default()
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn a_user_links_by_device_code_once_approved_and_never_gets_the_token_back_until_they_unlink()
{
    Log::global();
    let server = TestServer::builder().start().await;
    server.enable_test_plugin("watch-sync").await;
    let ada = server.user_with_default_permissions("ada").await;
    let users = UserServiceClient::new(HttpClient::plaintext(), ada.client_config());

    let start = StartWatchSyncLinkRequest {
        plugin_id: PLUGIN.to_owned(),
        ..Default::default()
    };
    let started = users
        .start_watch_sync_link(start)
        .await
        .unwrap()
        .into_owned();
    let poll = PollWatchSyncLinkRequest {
        plugin_id: PLUGIN.to_owned(),
        device_code: started.device_code.clone(),
        ..Default::default()
    };
    let pending = (users.poll_watch_sync_link(poll.clone()).await.unwrap()).into_owned();
    let before = listed(&users).await;
    approve(&server, &started.device_code).await;
    let polled = users.poll_watch_sync_link(poll).await.unwrap().into_owned();
    let linked = listed(&users).await;
    let unlink = DeletePersonalSettingsRequest {
        plugin_id: PLUGIN.to_owned(),
        ..Default::default()
    };
    users.delete_personal_settings(unlink).await.unwrap();
    let unlinked = listed(&users).await;

    assert_eq!(started.user_code, "TEST-CODE");
    assert_eq!(started.verification_url, "https://tracker.example/activate");
    assert_eq!(
        pending.state,
        WatchSyncLinkState::WATCH_SYNC_LINK_STATE_PENDING
    );
    assert!(!token_set(&before), "{before:?}");
    assert_eq!(
        polled.state,
        WatchSyncLinkState::WATCH_SYNC_LINK_STATE_LINKED
    );
    assert!(linked.watch_sync);
    assert!(token_set(&linked), "{linked:?}");
    assert!(!format!("{linked:?}").contains("token-of-"), "{linked:?}");
    assert!(!token_set(&unlinked), "{unlinked:?}");
}
