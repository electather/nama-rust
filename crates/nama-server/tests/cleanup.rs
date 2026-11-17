//! The Cleanup Installed Plugin, bundled in the Server, ticking its real WASM against the real
//! Kernel over a real socket, with TMDB on against a fake: it deletes a Requested Movie or Episode
//! once each of its Requesters is done with it, and only through what the Admin granted it (ADR
//! 0048). A Movie a Library has can't be asked for, so each test asks before the Library's scan.

#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use std::time::Duration;

use buffa::EnumValue;
use connectrpc::client::ClientConfig;
use connectrpc::client::HttpClient;
use nama_proto::nama::v1::{
    BrowseServiceClient, DeleteUserRequest, EnablePluginRequest, ExternalId, LibraryIds,
    LibraryKind, LibraryServiceClient, ListEpisodesRequest, ListMoviesRequest, Movie,
    PluginPermission, PluginServiceClient, ReportProgressRequest, SetPlayedRequest, SettingValue,
    UpdateLibraryRequest, UpdatePluginSettingsRequest, UserServiceClient, WatchStateServiceClient,
    event, set_played_request::Target, setting_value::Value,
};
use nama_server::plugins::Registry;
use server::{
    Library, SignedIn, TestServer, WAIT,
    requests::Requester,
    tmdb::{ALIEN, DARK, DUNE},
};

const CLEANUP: &str = "cleanup";
const ALIEN_FILE: &str = "Alien (1979)/Alien (1979).mkv";
const DUNE_FILE: &str = "Dune (2021)/Dune (2021).mkv";
const DARK_S01E01: &str = "Dark (2017)/Season 01/Dark - S01E01.mkv";
const DARK_S01E02: &str = "Dark (2017)/Season 01/Dark - S01E02.mkv";
/// The TMDB ids of Dark's Episodes S01E01 and S01E02.
const SECRETS: i64 = 1_388_538;
const LIES: i64 = 1_388_539;
/// Two hours, in milliseconds: the length a test's player reports.
const TWO_HOURS: u64 = 7_200_000;
/// The Cleanup's `days` setting in these tests: not its default, so they show a stored one
/// applies.
const DAYS: u64 = 10;
/// The Cleanup's `days_unwatched` setting in these tests, under its default of 90 since a
/// session not used for 90 days expires.
const DAYS_UNWATCHED: u64 = 30;

/// Every Permission the Cleanup requests.
const EVERY_PERMISSION: [PluginPermission; 5] = [
    PluginPermission::PLUGIN_PERMISSION_STORE,
    PluginPermission::PLUGIN_PERMISSION_READ_LIBRARIES,
    PluginPermission::PLUGIN_PERMISSION_READ_WATCH_STATES,
    PluginPermission::PLUGIN_PERMISSION_READ_REQUESTS,
    PluginPermission::PLUGIN_PERMISSION_DELETE_MEDIA,
];

/// A Server with the Built-in Plugins and a fake TMDB.
async fn start() -> TestServer {
    TestServer::builder()
        .plugins(Registry::with_builtins())
        .tmdb()
        .moves_clock()
        .start()
        .await
}

/// A Movies Library holding Alien and Dune, scanned and Matched.
async fn alien_and_dune(server: &TestServer) -> Library {
    server
        .matched_library(LibraryKind::LIBRARY_KIND_MOVIES, &[ALIEN_FILE, DUNE_FILE])
        .await
}

/// A Server where the Admin asked for Alien and Dune, and a Movies Library holding both.
async fn alien_and_dune_the_admin_asked_for() -> (TestServer, Library) {
    let server = start().await;
    for movie in [ALIEN, DUNE] {
        server.admin_requests().ask(movie).await;
    }
    let library = alien_and_dune(&server).await;
    (server, library)
}

/// Asks for each of `movies` as `who`.
async fn ask(server: &TestServer, who: &SignedIn, movies: &[i64]) {
    for &movie in movies {
        server.requests(who).ask(movie).await;
    }
}

/// Sets the Cleanup to clean `library`, by its id, with [`DAYS`] and [`DAYS_UNWATCHED`], and
/// enables it granting `granted`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn enable_cleanup(server: &TestServer, library: &Library, granted: &[PluginPermission]) {
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
                setting(
                    "days_unwatched",
                    Value::IntegerValue(DAYS_UNWATCHED.try_into().unwrap()),
                ),
                setting(
                    "libraries",
                    Value::LibraryIds(Box::new(LibraryIds {
                        ids: vec![library.id],
                        ..Default::default()
                    })),
                ),
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

/// The title with the TMDB id `tmdb`, as browsing identifies it.
fn tmdb_id(tmdb: i64) -> ExternalId {
    ExternalId {
        namespace: "tmdb".to_owned(),
        value: tmdb.to_string(),
        ..Default::default()
    }
}

fn movie(tmdb: i64) -> Target {
    Target::MovieId(Box::new(tmdb_id(tmdb)))
}

fn episode(tmdb: i64) -> Target {
    Target::EpisodeId(Box::new(tmdb_id(tmdb)))
}

/// Marks `target` in `library` played or unplayed, as the User whose session `who` carries.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn mark(who: ClientConfig, library: &Library, target: Target, played: bool) {
    WatchStateServiceClient::new(HttpClient::plaintext(), who)
        .set_played(SetPlayedRequest {
            library_id: library.id,
            target: Some(target),
            played,
            ..Default::default()
        })
        .await
        .unwrap();
}

/// Marks the Movie with the TMDB id `tmdb` in `library` played, as the Admin.
async fn mark_played(server: &TestServer, library: &Library, tmdb: i64) {
    mark(server.client_config(), library, movie(tmdb), true).await;
}

/// Reports `position_ms` into the Version `version`, [`TWO_HOURS`] long, as the User whose
/// session `who` carries.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn report_progress(who: ClientConfig, version: i64, position_ms: u64) {
    WatchStateServiceClient::new(HttpClient::plaintext(), who)
        .report_progress(ReportProgressRequest {
            version_id: version,
            position_ms,
            duration_ms: TWO_HOURS,
            ..Default::default()
        })
        .await
        .unwrap();
}

/// The Movies `library` lists.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn listed_movies(server: &TestServer, library: &Library) -> Vec<Movie> {
    BrowseServiceClient::new(HttpClient::plaintext(), server.client_config())
        .list_movies(ListMoviesRequest {
            library_id: library.id,
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .movies
}

/// The id of the one Version of the Movie with the TMDB id `tmdb` in `library`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn version_of(server: &TestServer, library: &Library, tmdb: i64) -> i64 {
    let listed = listed_movies(server, library).await;
    let movie = (listed.iter())
        .find(|movie| movie.id.as_option() == Some(&tmdb_id(tmdb)))
        .unwrap();
    assert_eq!(movie.versions.len(), 1, "the Movie has one Version");
    movie.versions.first().unwrap().id
}

/// The TMDB ids of the Movies `library` lists, sorted.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn movies(server: &TestServer, library: &Library) -> Vec<i64> {
    let mut ids: Vec<i64> = (listed_movies(server, library).await.into_iter())
        .map(|movie| movie.id.into_option().unwrap().value.parse().unwrap())
        .collect();
    ids.sort_unstable();
    ids
}

/// The TMDB ids of the Episodes of Dark's Season 1 that `library` lists, sorted.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn dark_episodes(server: &TestServer, library: &Library) -> Vec<i64> {
    let listed = BrowseServiceClient::new(HttpClient::plaintext(), server.client_config())
        .list_episodes(ListEpisodesRequest {
            library_id: library.id,
            show_id: tmdb_id(DARK).into(),
            season_number: 1,
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned();
    let mut ids: Vec<i64> = (listed.episodes.into_iter())
        .map(|episode| episode.id.into_option().unwrap().value.parse().unwrap())
        .collect();
    ids.sort_unstable();
    ids
}

/// Waits until `listed` gives exactly the TMDB ids `tmdb`, as a delete running in the background
/// leaves the Library, failing the test after [`WAIT`].
async fn wait_until_listed(listed: impl AsyncFn() -> Vec<i64>, tmdb: &[i64]) {
    let waited = tokio::time::timeout(WAIT, async {
        while listed().await != tmdb {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
    assert!(
        waited.is_ok(),
        "the Library lists {:?}, not {tmdb:?}",
        listed().await
    );
}

/// Waits until `library` lists exactly the Movies `tmdb` names.
async fn wait_for_movies(server: &TestServer, library: &Library, tmdb: &[i64]) {
    wait_until_listed(async || movies(server, library).await, tmdb).await;
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

/// Moves the clock past the `days` setting, so a Movie its Requesters played before is due.
fn past_the_limit(server: &TestServer) {
    server.advance_clock(Duration::from_hours((DAYS + 1) * 24));
}

/// Moves the clock past `days_unwatched`, so a Movie added before is due for a Requester who
/// didn't play it.
fn past_the_days_unwatched(server: &TestServer) {
    server.advance_clock(Duration::from_hours((DAYS_UNWATCHED + 1) * 24));
}

/// Deletes the User `who` as the Admin.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn delete_user(server: &TestServer, who: &SignedIn) {
    UserServiceClient::new(HttpClient::plaintext(), server.client_config())
        .delete_user(DeleteUserRequest {
            user_id: who.id,
            ..Default::default()
        })
        .await
        .unwrap();
}

/// Makes Dune due, played by each of its Requesters, whose sessions `requesters` carries, ticks,
/// and waits until the Library lists Alien alone, so the test can tell Alien was kept without
/// waiting a set time. Deletes of one Library wait for its scan lock, a `tokio::sync::Mutex`,
/// which is handed out in the order asked; so a delete of Alien an earlier tick started would have
/// run before Dune's.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn dune_alone_is_deleted_next(
    server: &TestServer,
    library: &Library,
    requesters: &[ClientConfig],
) {
    for who in requesters {
        mark(who.clone(), library, movie(DUNE), true).await;
    }
    past_the_limit(server);

    server.tick(CLEANUP).await.unwrap();

    wait_for_movies(server, library, &[ALIEN]).await;
}

#[tokio::test]
async fn a_movie_its_requester_watched_past_the_limit_is_deleted_and_a_recent_one_kept() {
    let (server, library) = alien_and_dune_the_admin_asked_for().await;
    enable_cleanup(&server, &library, &EVERY_PERMISSION).await;
    mark_played(&server, &library, ALIEN).await;
    past_the_limit(&server);
    mark_played(&server, &library, DUNE).await;

    server.tick(CLEANUP).await.unwrap();

    wait_for_movies(&server, &library, &[DUNE]).await;
}

#[tokio::test]
async fn a_library_renamed_after_a_tick_found_it_is_still_cleaned() {
    let (server, library) = alien_and_dune_the_admin_asked_for().await;
    enable_cleanup(&server, &library, &EVERY_PERMISSION).await;
    mark_played(&server, &library, ALIEN).await;
    // Nothing is due yet.
    server.tick(CLEANUP).await.unwrap();
    rename(&server, &library, "Films").await;
    past_the_limit(&server);
    mark_played(&server, &library, DUNE).await;

    server.tick(CLEANUP).await.unwrap();

    wait_for_movies(&server, &library, &[DUNE]).await;
}

/// The bug the Cleanup's old setting of Library names had: a Library renamed before any tick found
/// it under its name was dropped silently.
#[tokio::test]
async fn a_library_renamed_before_any_tick_is_still_cleaned() {
    let (server, library) = alien_and_dune_the_admin_asked_for().await;
    enable_cleanup(&server, &library, &EVERY_PERMISSION).await;
    rename(&server, &library, "Films").await;
    mark_played(&server, &library, ALIEN).await;
    past_the_limit(&server);
    mark_played(&server, &library, DUNE).await;

    server.tick(CLEANUP).await.unwrap();

    wait_for_movies(&server, &library, &[DUNE]).await;
}

#[tokio::test]
async fn without_the_delete_permission_it_deletes_nothing_and_says_why() {
    let (server, library) = alien_and_dune_the_admin_asked_for().await;
    let all_but_delete = EVERY_PERMISSION
        .into_iter()
        .filter(|&it| it != PluginPermission::PLUGIN_PERMISSION_DELETE_MEDIA)
        .collect::<Vec<_>>();
    enable_cleanup(&server, &library, &all_but_delete).await;
    mark_played(&server, &library, ALIEN).await;
    past_the_limit(&server);
    mark_played(&server, &library, DUNE).await;

    let refused = server
        .tick(CLEANUP)
        .await
        .expect_err("the delete was never granted");

    assert!(refused.contains("Delete media"), "{refused}");
    // The Kernel refuses before it starts a delete, so none is running in the background.
    assert_eq!(movies(&server, &library).await, [ALIEN, DUNE]);
}

#[tokio::test]
async fn a_movie_another_user_watched_is_kept_until_its_requester_had_the_days_unwatched() {
    let server = start().await;
    let ada = server.user_with_default_permissions("Ada").await;
    ask(&server, &ada, &[ALIEN, DUNE]).await;
    let library = alien_and_dune(&server).await;
    enable_cleanup(&server, &library, &EVERY_PERMISSION).await;
    mark_played(&server, &library, ALIEN).await;
    past_the_limit(&server);
    server.tick(CLEANUP).await.unwrap();
    dune_alone_is_deleted_next(&server, &library, &[ada.client_config()]).await;
    past_the_days_unwatched(&server);

    server.tick(CLEANUP).await.unwrap();

    wait_for_movies(&server, &library, &[]).await;
}

#[tokio::test]
async fn a_movie_two_users_requested_is_kept_until_both_are_done() {
    let server = start().await;
    let ada = server.user_with_default_permissions("Ada").await;
    let ben = server.user_with_default_permissions("Ben").await;
    for who in [&ada, &ben] {
        ask(&server, who, &[ALIEN, DUNE]).await;
    }
    let library = alien_and_dune(&server).await;
    enable_cleanup(&server, &library, &EVERY_PERMISSION).await;
    mark(ada.client_config(), &library, movie(ALIEN), true).await;
    past_the_limit(&server);
    server.tick(CLEANUP).await.unwrap();
    let both = [ada.client_config(), ben.client_config()];
    dune_alone_is_deleted_next(&server, &library, &both).await;
    mark(ben.client_config(), &library, movie(ALIEN), true).await;
    past_the_limit(&server);

    server.tick(CLEANUP).await.unwrap();

    wait_for_movies(&server, &library, &[]).await;
}

#[tokio::test]
async fn a_movie_its_requester_never_played_is_deleted_once_the_days_unwatched_passed() {
    let server = start().await;
    let ada = server.user_with_default_permissions("Ada").await;
    ask(&server, &ada, &[ALIEN]).await;
    let library = alien_and_dune(&server).await;
    enable_cleanup(&server, &library, &EVERY_PERMISSION).await;
    past_the_days_unwatched(&server);

    server.tick(CLEANUP).await.unwrap();

    wait_for_movies(&server, &library, &[DUNE]).await;
}

#[tokio::test]
async fn a_movie_nobody_requested_is_never_deleted() {
    let server = start().await;
    let ada = server.user_with_default_permissions("Ada").await;
    ask(&server, &ada, &[DUNE]).await;
    let library = alien_and_dune(&server).await;
    enable_cleanup(&server, &library, &EVERY_PERMISSION).await;
    mark_played(&server, &library, ALIEN).await;
    mark(ada.client_config(), &library, movie(ALIEN), true).await;
    past_the_days_unwatched(&server);
    // Played just now, so Dune isn't due yet, though it was added long ago.
    mark(ada.client_config(), &library, movie(DUNE), true).await;

    server.tick(CLEANUP).await.unwrap();

    dune_alone_is_deleted_next(&server, &library, &[ada.client_config()]).await;
}

#[tokio::test]
async fn a_movie_whose_requester_was_deleted_is_deleted_once_the_days_unwatched_passed() {
    let server = start().await;
    let ada = server.user_with_default_permissions("Ada").await;
    ask(&server, &ada, &[ALIEN]).await;
    server.admin_requests().ask(DUNE).await;
    let library = alien_and_dune(&server).await;
    enable_cleanup(&server, &library, &EVERY_PERMISSION).await;
    mark(ada.client_config(), &library, movie(ALIEN), true).await;
    delete_user(&server, &ada).await;
    past_the_limit(&server);
    server.tick(CLEANUP).await.unwrap();
    dune_alone_is_deleted_next(&server, &library, &[server.client_config()]).await;
    past_the_days_unwatched(&server);

    server.tick(CLEANUP).await.unwrap();

    wait_for_movies(&server, &library, &[]).await;
}

/// A Requester starting a played Movie again keeps it, since starting again unmarks it, so they
/// no longer have it played.
#[tokio::test]
async fn a_movie_its_requester_has_started_again_is_kept() {
    let server = start().await;
    let ada = server.user_with_default_permissions("Ada").await;
    ask(&server, &ada, &[ALIEN, DUNE]).await;
    let library = alien_and_dune(&server).await;
    enable_cleanup(&server, &library, &EVERY_PERMISSION).await;
    mark(ada.client_config(), &library, movie(ALIEN), true).await;
    past_the_limit(&server);
    // A played title stays played, so starting it again from the beginning unmarks it first.
    mark(ada.client_config(), &library, movie(ALIEN), false).await;
    let alien = version_of(&server, &library, ALIEN).await;
    report_progress(ada.client_config(), alien, 600_000).await;

    server.tick(CLEANUP).await.unwrap();

    dune_alone_is_deleted_next(&server, &library, &[ada.client_config()]).await;
}

#[tokio::test]
async fn an_episode_of_a_followed_show_waits_for_its_requesters() {
    let server = start().await;
    let ada = server.user_with_default_permissions("Ada").await;
    // A User who never watches anything holds nothing back.
    server.user_with_default_permissions("Ben").await;
    server.requests(&ada).follow(DARK).await;
    let library = server
        .matched_library(LibraryKind::LIBRARY_KIND_SHOWS, &[DARK_S01E01, DARK_S01E02])
        .await;
    enable_cleanup(&server, &library, &EVERY_PERMISSION).await;
    mark(server.client_config(), &library, episode(LIES), true).await;
    past_the_limit(&server);
    // Played just now, so not due yet.
    mark(ada.client_config(), &library, episode(SECRETS), true).await;
    // The Admin played Lies long ago, but didn't ask for Dark.
    server.tick(CLEANUP).await.unwrap();
    past_the_limit(&server);

    server.tick(CLEANUP).await.unwrap();

    // Deletes of one Library run in the order asked, so a delete of Lies the first tick started
    // would have left Secrets alone, never Lies.
    wait_until_listed(async || dark_episodes(&server, &library).await, &[LIES]).await;
}

/// A folder made read-only, made writable again however the test ends, so a failing assertion
/// can't leave behind a scratch directory nobody can remove.
#[cfg(unix)]
struct ReadOnly(std::path::PathBuf);

#[cfg(unix)]
impl ReadOnly {
    /// Makes `folder` read-only until the guard is dropped.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    fn new(folder: &std::path::Path) -> Self {
        set_mode(folder, 0o555).unwrap();
        Self(folder.to_owned())
    }
}

#[cfg(unix)]
impl Drop for ReadOnly {
    fn drop(&mut self) {
        // No unwrap: a panic while the test is already unwinding aborts the whole run.
        let _: std::io::Result<()> = set_mode(&self.0, 0o755);
    }
}

/// Sets the mode of the folder `folder` to `mode`.
#[cfg(unix)]
fn set_mode(folder: &std::path::Path, mode: u32) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(folder, std::fs::Permissions::from_mode(mode))
}

/// A delete that failed in the background leaves the Movie listed, and a day later the Cleanup
/// asks again. The Kernel refuses that second delete up front, since the Library's last scan found
/// its folder read-only, so the tick says so. Unix only: Windows has no simple equivalent of
/// taking away these permissions.
#[cfg(unix)]
#[tokio::test]
async fn a_delete_that_failed_is_asked_again_a_day_later() {
    let (server, library) = alien_and_dune_the_admin_asked_for().await;
    enable_cleanup(&server, &library, &EVERY_PERMISSION).await;
    mark_played(&server, &library, ALIEN).await;
    past_the_limit(&server);
    let alien_folder = ReadOnly::new(&library.folder().join("Alien (1979)"));
    if std::fs::File::create(alien_folder.0.join("probe")).is_ok() {
        // Running as root, which writes anywhere; there's nothing to test.
        return;
    }

    server.tick(CLEANUP).await.unwrap();
    let _library_folder = ReadOnly::new(library.folder());
    (server.scan_until(library.id, |event| {
        matches!(event, event::Event::ScanFinished(_))
    }))
    .await;
    let same_day = server.tick(CLEANUP).await;
    server.advance_clock(Duration::from_hours(24));
    let a_day_later = server.tick(CLEANUP).await;

    assert_eq!(same_day, Ok(()), "asked again before a day passed");
    let refused = a_day_later.expect_err("a day later, the delete is asked again");
    assert!(refused.contains("can't write to it"), "{refused}");
    assert_eq!(movies(&server, &library).await, [ALIEN, DUNE]);
}
