//! The Cleanup Installed Plugin, bundled in the Server, ticking its real WASM against the real
//! Kernel over a real socket, with TMDB on against a fake: it deletes a Movie every User played
//! long enough ago, and only through what the Admin granted it (ADR 0048).

#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use std::time::Duration;

use buffa::EnumValue;
use connectrpc::client::HttpClient;
use nama_proto::nama::v1::{
    BrowseServiceClient, EnablePluginRequest, ExternalId, LibraryKind, LibraryServiceClient,
    ListMoviesRequest, PluginPermission, PluginServiceClient, SetPlayedRequest, SettingValue,
    UpdateLibraryRequest, UpdatePluginSettingsRequest, WatchStateServiceClient, set_played_request,
    setting_value::Value,
};
use nama_server::plugins::Registry;
use server::{
    Library, TestServer, WAIT,
    tmdb::{ALIEN, DUNE},
};

const CLEANUP: &str = "cleanup";
const ALIEN_FILE: &str = "Alien (1979)/Alien (1979).mkv";
const DUNE_FILE: &str = "Dune (2021)/Dune (2021).mkv";
/// The Cleanup's `days` setting in these tests.
const DAYS: u64 = 30;

/// Every Permission the Cleanup requests.
const EVERY_PERMISSION: [PluginPermission; 5] = [
    PluginPermission::PLUGIN_PERMISSION_STORE,
    PluginPermission::PLUGIN_PERMISSION_READ_LIBRARIES,
    PluginPermission::PLUGIN_PERMISSION_READ_WATCH_STATES,
    PluginPermission::PLUGIN_PERMISSION_READ_USERS,
    PluginPermission::PLUGIN_PERMISSION_DELETE_MEDIA,
];

/// A Server with the Built-in Plugins and a fake TMDB, and a Movies Library holding Alien and
/// Dune, scanned and Matched.
async fn alien_and_dune() -> (TestServer, Library) {
    let server = TestServer::builder()
        .plugins(Registry::with_builtins())
        .tmdb()
        .start()
        .await;
    let library = server
        .matched_library(LibraryKind::LIBRARY_KIND_MOVIES, &[ALIEN_FILE, DUNE_FILE])
        .await;
    (server, library)
}

/// Sets the Cleanup to clean `library` of what every User played [`DAYS`] days ago, and enables
/// it granting `granted`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn enable_cleanup(server: &TestServer, granted: &[PluginPermission]) {
    let plugins = PluginServiceClient::new(HttpClient::plaintext(), server.client_config());
    let setting = |key: &str, value: Value| {
        let value = SettingValue {
            value: Some(value),
            ..Default::default()
        };
        (key.to_owned(), value)
    };
    plugins
        .update_plugin_settings(UpdatePluginSettingsRequest {
            id: CLEANUP.to_owned(),
            settings: [
                setting("days", Value::IntegerValue(DAYS.try_into().unwrap())),
                // `TestServer` names every Library it creates "Media".
                setting("libraries", Value::TextValue("Media".to_owned())),
            ]
            .into_iter()
            .collect(),
            ..Default::default()
        })
        .await
        .unwrap();
    plugins
        .enable_plugin(EnablePluginRequest {
            id: CLEANUP.to_owned(),
            granted_permissions: granted.iter().map(|&it| EnumValue::from(it)).collect(),
            ..Default::default()
        })
        .await
        .unwrap();
}

/// Marks the Movie with the TMDB id `tmdb` in `library` played, as the Admin.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn mark_played(server: &TestServer, library: &Library, tmdb: i64) {
    let movie = ExternalId {
        namespace: "tmdb".to_owned(),
        value: tmdb.to_string(),
        ..Default::default()
    };
    WatchStateServiceClient::new(HttpClient::plaintext(), server.client_config())
        .set_played(SetPlayedRequest {
            library_id: library.id,
            target: Some(set_played_request::Target::MovieId(Box::new(movie))),
            played: true,
            ..Default::default()
        })
        .await
        .unwrap();
}

/// The TMDB ids of the Movies `library` lists, sorted.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn movies(server: &TestServer, library: &Library) -> Vec<i64> {
    let listed = BrowseServiceClient::new(HttpClient::plaintext(), server.client_config())
        .list_movies(ListMoviesRequest {
            library_id: library.id,
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned();
    let mut ids: Vec<i64> = (listed.movies.into_iter())
        .map(|movie| movie.id.into_option().unwrap().value.parse().unwrap())
        .collect();
    ids.sort_unstable();
    ids
}

/// Waits until `library` lists exactly the Movies `tmdb` names, as a delete running in the
/// background leaves it, failing the test after [`WAIT`].
async fn wait_for_movies(server: &TestServer, library: &Library, tmdb: &[i64]) {
    let waited = tokio::time::timeout(WAIT, async {
        while movies(server, library).await != tmdb {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
    assert!(
        waited.is_ok(),
        "the Library lists {:?}, not {tmdb:?}",
        movies(server, library).await
    );
}

/// Renames `library` to `name`, as the Admin, keeping its folder.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn rename(server: &TestServer, library: &Library, name: &str) {
    LibraryServiceClient::new(HttpClient::plaintext(), server.client_config())
        .update_library(UpdateLibraryRequest {
            id: library.id,
            name: name.to_owned(),
            folders: vec![library.folder().to_str().unwrap().to_owned()],
            ..Default::default()
        })
        .await
        .unwrap();
}

/// Moves the clock past the `days` setting, so a Movie played before is due.
fn past_the_limit(server: &TestServer) {
    server.advance_clock(Duration::from_hours((DAYS + 1) * 24));
}

#[tokio::test]
async fn a_movie_every_user_watched_past_the_limit_is_deleted_and_a_recent_one_kept() {
    let (server, library) = alien_and_dune().await;
    enable_cleanup(&server, &EVERY_PERMISSION).await;
    mark_played(&server, &library, ALIEN).await;
    past_the_limit(&server);
    mark_played(&server, &library, DUNE).await;

    server.tick(CLEANUP).await.unwrap();

    wait_for_movies(&server, &library, &[DUNE]).await;
}

#[tokio::test]
async fn a_library_renamed_after_a_tick_found_it_is_still_cleaned() {
    let (server, library) = alien_and_dune().await;
    enable_cleanup(&server, &EVERY_PERMISSION).await;
    mark_played(&server, &library, ALIEN).await;
    // Nothing is due yet; this tick finds the Library by the name the setting gives it.
    server.tick(CLEANUP).await.unwrap();
    rename(&server, &library, "Films").await;
    past_the_limit(&server);
    mark_played(&server, &library, DUNE).await;

    server.tick(CLEANUP).await.unwrap();

    wait_for_movies(&server, &library, &[DUNE]).await;
}

#[tokio::test]
async fn without_the_delete_permission_it_deletes_nothing_and_says_why() {
    let (server, library) = alien_and_dune().await;
    let all_but_delete = EVERY_PERMISSION
        .into_iter()
        .filter(|&it| it != PluginPermission::PLUGIN_PERMISSION_DELETE_MEDIA)
        .collect::<Vec<_>>();
    enable_cleanup(&server, &all_but_delete).await;
    mark_played(&server, &library, ALIEN).await;
    past_the_limit(&server);
    mark_played(&server, &library, DUNE).await;

    let refused = server
        .tick(CLEANUP)
        .await
        .expect_err("the delete was never granted");

    assert!(refused.contains("Delete media"), "{refused}");
    // A delete that started would show soon; give it the time one takes.
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(movies(&server, &library).await, [ALIEN, DUNE]);
}

#[tokio::test]
async fn a_movie_one_user_has_not_watched_is_kept() {
    let (server, library) = alien_and_dune().await;
    server.user_with_default_permissions("Ada").await;
    enable_cleanup(&server, &EVERY_PERMISSION).await;
    mark_played(&server, &library, ALIEN).await;
    past_the_limit(&server);

    server.tick(CLEANUP).await.unwrap();

    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(movies(&server, &library).await, [ALIEN, DUNE]);
}
