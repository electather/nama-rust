//! Home rows a Plugin gives, every User's and each User's own, over the API (ADR 0058): the test
//! Plugin built as a Home row source, refreshed in the background, its rows showing the titles on
//! the Server and those missing from it to Request.

use crate::support::{home_row_plugin, http, log, refused, server, test_plugin};

use std::{
    path::Path,
    time::{Duration, Instant},
};

use connectrpc::{
    ErrorCode,
    client::{ClientConfig, HttpClient},
};
use home_row_plugin::{PLUGIN, disable, enable, set_handle, text};
use http::fetch;
use log::Log;
use nama_proto::nama::v1::{
    BrowseServiceClient, ExtensionPoint, ExtensionPointFailure, GetPluginRequest, HomeRow,
    HomeRowTitle, KernelHomeRow, LibraryKind, ListHomeRowTitlesRequest, ListHomeRowTitlesResponse,
    ListHomeRowsRequest, PageRequest, Permission, PluginHomeRow, PluginServiceClient,
    RecentlyAddedHomeRow, RefusalReason, RequestState, UpdatePluginSettingsRequest,
    home_row::KernelOrPlugin, home_row_title::MovieOrShow,
};
use refused::assert_refused;
use server::{
    Builder, SignedIn, TestServer, WAIT,
    requests::Requester,
    tmdb::{ALIEN, DARK, DUNE, HEAT},
};

/// A Server with the fake TMDB, a Movies Library of Alien and Heat, and the test Plugin built as a
/// Home row source, its `home_rows` setting `rows`, not yet enabled.
async fn installed(builder: Builder, rows: &str) -> TestServer {
    let server = builder.tmdb().start().await;
    let library = (server)
        .create_library(
            LibraryKind::LIBRARY_KIND_MOVIES,
            &[
                "Alien (1979)/Alien (1979).mkv",
                "Heat (1995) {tmdb-949}/Heat (1995).mkv",
            ],
        )
        .await;
    server.scan_and_match(&library).await;
    server.install_test_plugin("home-rows").await;
    set_rows(&server, rows).await;
    server
}

/// [`installed`], with the test Plugin enabled.
async fn enabled(rows: &str) -> TestServer {
    let server = installed(TestServer::builder(), rows).await;
    enable(&server).await;
    server
}

fn plugins(server: &TestServer) -> PluginServiceClient<HttpClient> {
    PluginServiceClient::new(HttpClient::plaintext(), server.client_config())
}

fn browse(server: &TestServer) -> BrowseServiceClient<HttpClient> {
    BrowseServiceClient::new(HttpClient::plaintext(), server.client_config())
}

fn browse_as(server: &TestServer, who: &SignedIn) -> BrowseServiceClient<HttpClient> {
    BrowseServiceClient::new(HttpClient::plaintext(), server.client_config_as(&who.token))
}

/// Saves `rows` as the test Plugin's `home_rows` setting, the keys of the rows it gives.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn set_rows(server: &TestServer, rows: &str) {
    (plugins(server))
        .update_plugin_settings(UpdatePluginSettingsRequest {
            id: PLUGIN.to_owned(),
            settings: std::iter::once(("home_rows".to_owned(), text(rows))).collect(),
            ..Default::default()
        })
        .await
        .unwrap();
}

/// The Home rows home lists now, each as its Plugin, key and heading.
async fn rows(server: &TestServer) -> Vec<(String, String, String)> {
    rows_from(&browse(server)).await
}

/// The Home rows home lists the caller `config` signs in as now, as [`rows`] lists them.
async fn rows_as(config: ClientConfig) -> Vec<(String, String, String)> {
    rows_from(&BrowseServiceClient::new(HttpClient::plaintext(), config)).await
}

/// The Plugin rows home lists `browse`'s caller now, as [`rows`] lists them.
async fn rows_from(browse: &BrowseServiceClient<HttpClient>) -> Vec<(String, String, String)> {
    (all_rows_from(browse).await.into_iter())
        .filter_map(|row| match row.kernel_or_plugin {
            Some(KernelOrPlugin::Plugin(row)) => Some((row.plugin_id, row.key, row.heading)),
            _ => None,
        })
        .collect()
}

/// Every Home row home lists `browse`'s caller now, the Kernel's and the Plugins'.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn all_rows_from(browse: &BrowseServiceClient<HttpClient>) -> Vec<HomeRow> {
    let listed = browse.list_home_rows(ListHomeRowsRequest::default()).await;
    listed.unwrap().into_owned().rows
}

/// Lists the Admin's Home rows until they are the rows `keys` name, since a refresh runs in the
/// background.
async fn wait_for_rows(server: &TestServer, keys: &[&str]) {
    wait_for_rows_as(server.client_config(), keys).await;
}

/// [`wait_for_rows`], for the caller `config` signs in as.
async fn wait_for_rows_as(config: ClientConfig, keys: &[&str]) {
    let started = Instant::now();
    loop {
        let listed = rows_as(config.clone()).await;
        if listed
            .iter()
            .map(|(_, key, _)| key.as_str())
            .eq(keys.iter().copied())
        {
            return;
        }
        assert!(
            started.elapsed() < WAIT,
            "home lists {listed:?}, not {keys:?}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// The test Plugin's failures the Plugins page shows, once there is one, since a refresh runs in
/// the background.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn wait_for_failures(server: &TestServer) -> Vec<ExtensionPointFailure> {
    let asked = GetPluginRequest {
        id: PLUGIN.to_owned(),
        ..Default::default()
    };
    let started = Instant::now();
    loop {
        let got = plugins(server).get_plugin(asked.clone()).await.unwrap();
        let failures = got.into_owned().plugin.into_option().unwrap().failures;
        if !failures.is_empty() {
            return failures;
        }
        assert!(started.elapsed() < WAIT, "no failure shown on {PLUGIN}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

async fn titles_page(
    server: &TestServer,
    key: &str,
    size: i32,
    token: &str,
) -> Result<ListHomeRowTitlesResponse, connectrpc::ConnectError> {
    titles_page_from(&browse(server), key, size, token).await
}

/// [`titles_page`], for the caller `config` signs in as.
async fn titles_page_as(
    config: ClientConfig,
    key: &str,
    size: i32,
    token: &str,
) -> Result<ListHomeRowTitlesResponse, connectrpc::ConnectError> {
    let browse = BrowseServiceClient::new(HttpClient::plaintext(), config);
    titles_page_from(&browse, key, size, token).await
}

/// The whole of the row `key` as `browse`'s caller sees it.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn titles_from(browse: &BrowseServiceClient<HttpClient>, key: &str) -> Vec<HomeRowTitle> {
    titles_page_from(browse, key, 0, "").await.unwrap().titles
}

async fn titles_page_from(
    browse: &BrowseServiceClient<HttpClient>,
    key: &str,
    size: i32,
    token: &str,
) -> Result<ListHomeRowTitlesResponse, connectrpc::ConnectError> {
    let asked = ListHomeRowTitlesRequest {
        plugin_id: PLUGIN.to_owned(),
        key: key.to_owned(),
        page: PageRequest {
            size,
            token: token.to_owned(),
            ..Default::default()
        }
        .into(),
        ..Default::default()
    };
    Ok(browse.list_home_row_titles(asked).await?.into_owned())
}

/// A title in a row as its kind, whether it is on the Server, and its TMDB id.
fn shown(title: &HomeRowTitle) -> String {
    match &title.movie_or_show {
        Some(MovieOrShow::Movie(movie)) => format!("Movie {}", movie.id.value),
        Some(MovieOrShow::Show(show)) => format!("Show {}", show.id.value),
        Some(MovieOrShow::MissingMovie(movie)) => format!("missing Movie {}", movie.id.value),
        Some(MovieOrShow::MissingShow(show)) => format!("missing Show {}", show.id.value),
        None => "nothing".to_owned(),
    }
}

/// Each of `titles`, as [`shown`] gives it.
fn all_shown(titles: &[HomeRowTitle]) -> Vec<String> {
    titles.iter().map(shown).collect()
}

#[tokio::test]
async fn enabling_a_home_row_source_shows_its_rows_with_the_titles_on_the_server_or_a_title_source()
{
    let server = enabled("").await;

    wait_for_rows(&server, &["picks", "again", "missing"]).await;
    let picks = titles_page(&server, "picks", 0, "").await.unwrap();
    let missing = titles_page(&server, "missing", 0, "").await.unwrap();

    assert_eq!(
        rows(&server).await,
        [
            (PLUGIN.to_owned(), "picks".to_owned(), "Picks".to_owned()),
            (
                PLUGIN.to_owned(),
                "again".to_owned(),
                "Alien again".to_owned()
            ),
            (
                PLUGIN.to_owned(),
                "missing".to_owned(),
                "Not on the Server".to_owned()
            ),
        ]
    );
    assert_eq!(
        all_shown(&picks.titles),
        [
            format!("Movie {HEAT}"),
            format!("missing Movie {DUNE}"),
            format!("Movie {ALIEN}"),
            format!("missing Show {DARK}"),
        ],
        "the Plugin's order"
    );
    assert_eq!(picks.page.total_size, Some(4));
    assert_eq!(
        all_shown(&missing.titles),
        [format!("missing Movie {DUNE}")],
        "the id no title source has is dropped"
    );
}

fn continue_watching() -> HomeRow {
    HomeRow {
        kernel_or_plugin: Some(KernelOrPlugin::Kernel(
            KernelHomeRow::KERNEL_HOME_ROW_CONTINUE_WATCHING.into(),
        )),
        ..Default::default()
    }
}

/// The Recently Added row of the Library `id` of `kind`, named after its folder `folder`, as
/// [`TestServer::create_library`] names it.
fn recently_added(id: i64, kind: LibraryKind, folder: &Path) -> HomeRow {
    HomeRow {
        kernel_or_plugin: Some(
            RecentlyAddedHomeRow {
                library_id: id,
                library_name: folder.display().to_string(),
                library_kind: kind.into(),
                ..Default::default()
            }
            .into(),
        ),
        ..Default::default()
    }
}

#[tokio::test]
async fn home_lists_a_recently_added_row_for_each_library_after_continue_watching() {
    let server = TestServer::builder().start().await;
    let movies = (server)
        .create_library(LibraryKind::LIBRARY_KIND_MOVIES, &[])
        .await;
    let shows = (server)
        .create_library(LibraryKind::LIBRARY_KIND_SHOWS, &[])
        .await;
    let ada = server.user_with_default_permissions("ada").await;

    assert_eq!(
        all_rows_from(&browse_as(&server, &ada)).await,
        [
            continue_watching(),
            recently_added(movies.id, LibraryKind::LIBRARY_KIND_MOVIES, movies.folder()),
            recently_added(shows.id, LibraryKind::LIBRARY_KIND_SHOWS, shows.folder()),
        ]
    );
}

#[tokio::test]
async fn home_lists_the_kernel_rows_before_the_plugin_rows() {
    let server = enabled("").await;
    wait_for_rows(&server, &["picks", "again", "missing"]).await;

    let plugin = |key: &str, heading: &str| HomeRow {
        kernel_or_plugin: Some(
            PluginHomeRow {
                plugin_id: PLUGIN.to_owned(),
                key: key.to_owned(),
                heading: heading.to_owned(),
                ..Default::default()
            }
            .into(),
        ),
        ..Default::default()
    };
    let rows = all_rows_from(&browse(&server)).await;
    assert_eq!(rows[0], continue_watching());
    assert!(
        matches!(
            rows[1].kernel_or_plugin,
            Some(KernelOrPlugin::RecentlyAdded(_))
        ),
        "{rows:?}"
    );
    assert_eq!(
        rows[2..],
        [
            plugin("picks", "Picks"),
            plugin("again", "Alien again"),
            plugin("missing", "Not on the Server"),
        ]
    );
}

#[tokio::test]
async fn each_user_sees_their_own_rows_built_from_their_personal_settings() {
    let server = enabled("").await;
    let ada = server.user_with_default_permissions("ada").await;
    let bea = server.user_with_default_permissions("bea").await;
    set_handle(&ada, "ada").await;
    set_handle(&bea, "bea").await;

    wait_for_rows_as(
        ada.client_config(),
        &["picks", "again", "missing", "for-ada"],
    )
    .await;
    wait_for_rows_as(
        bea.client_config(),
        &["picks", "again", "missing", "for-bea"],
    )
    .await;

    let headings = |rows: Vec<(String, String, String)>| -> Vec<String> {
        rows.into_iter().map(|(_, _, heading)| heading).collect()
    };
    assert_eq!(
        headings(rows_as(ada.client_config()).await),
        ["Picks", "Alien again", "Not on the Server", "For ada"]
    );
    assert_eq!(
        headings(rows_as(bea.client_config()).await),
        ["Picks", "Alien again", "Not on the Server", "For bea"]
    );
    let beas = titles_page_as(bea.client_config(), "for-bea", 10, "").await;
    let adas = titles_page_as(ada.client_config(), "for-bea", 10, "").await;
    assert_eq!(beas.unwrap().titles.len(), 1);
    assert!(
        adas.unwrap().titles.is_empty(),
        "Ada can't page Bea's own row"
    );
}

#[tokio::test]
async fn a_row_pages_by_keyset_and_refuses_another_rows_token() {
    let server = enabled("").await;
    wait_for_rows(&server, &["picks", "again", "missing"]).await;

    let first = titles_page(&server, "picks", 1, "").await.unwrap();
    let second = (titles_page(&server, "picks", 1, &first.page.next_token).await).unwrap();
    let other_row = titles_page(&server, "again", 1, &first.page.next_token).await;

    let last = (titles_page(&server, "picks", 2, &second.page.next_token).await).unwrap();

    assert_eq!(all_shown(&first.titles), [format!("Movie {HEAT}")]);
    assert_eq!(all_shown(&second.titles), [format!("missing Movie {DUNE}")]);
    assert_eq!(
        all_shown(&last.titles),
        [format!("Movie {ALIEN}"), format!("missing Show {DARK}")]
    );
    assert_eq!(last.page.next_token, "", "the last page");
    let refusal = assert_refused(
        other_row,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_PAGE_TOKEN_INVALID,
    );
    assert_eq!(refusal.field, "page.token");
}

#[tokio::test]
async fn disabling_the_plugin_removes_its_rows() {
    let server = enabled("").await;
    wait_for_rows(&server, &["picks", "again", "missing"]).await;

    disable(&server).await;

    assert_eq!(rows(&server).await, []);
    let picks = titles_page(&server, "picks", 0, "").await.unwrap();
    assert_eq!(picks.titles.len(), 0);
}

#[tokio::test]
async fn a_row_the_plugin_stops_giving_is_gone_once_its_settings_change_refreshes_it() {
    let server = enabled("").await;
    wait_for_rows(&server, &["picks", "again", "missing"]).await;

    set_rows(&server, "picks").await;

    wait_for_rows(&server, &["picks"]).await;
}

#[tokio::test]
async fn home_lists_the_stored_rows_without_waiting_for_a_refresh() {
    let log = Log::global();
    let server = enabled("").await;
    wait_for_rows(&server, &["picks", "again", "missing"]).await;

    // Its new settings refresh it, and the refresh loops until the call's time limit.
    set_rows(&server, "loop").await;
    log.wait_for(&["test-plugin gives Home rows \"loop\""], 1)
        .await;
    let started = Instant::now();
    let listed = rows(&server).await;

    assert!(
        started.elapsed() < Duration::from_secs(2),
        "home waited on the Plugin"
    );
    assert_eq!(listed.len(), 3, "the rows stored before: {listed:?}");
    // Interrupts the looping refresh, which the runtime's drop would wait out to its time limit.
    disable(&server).await;
}

#[tokio::test]
async fn opening_home_once_the_rows_are_stale_refreshes_them() {
    let log = Log::global();
    let asked = ["test-plugin gives Home rows \"again,picks\""];
    let server = installed(TestServer::builder(), "again,picks").await;
    enable(&server).await;
    wait_for_rows(&server, &["picks", "again"]).await;
    assert_eq!(log.count(&asked), 1, "enabling refreshes it once");

    rows(&server).await;
    assert_eq!(log.count(&asked), 1, "fresh rows aren't refreshed");
    // The test Plugin's rows live an hour.
    server.advance_clock(Duration::from_mins(61));
    rows(&server).await;

    log.wait_for(&asked, 2).await;
}

#[tokio::test]
async fn a_failed_refresh_is_not_tried_again_for_15_minutes() {
    let log = Log::global();
    let asked = ["test-plugin gives Home rows \"fail\""];
    // Logged once the next try is put off.
    let failed = [
        "refreshing the Plugin's Home rows failed",
        "test-plugin was set to fail",
    ];
    let server = installed(TestServer::builder(), "fail").await;
    enable(&server).await;
    log.wait_for(&failed, 1).await;

    rows(&server).await;
    // Time for a refresh home started to call the Plugin, which a failing one does at once.
    tokio::time::sleep(Duration::from_millis(500)).await;
    let before = log.count(&asked);
    server.advance_clock(Duration::from_mins(16));
    rows(&server).await;

    assert_eq!(
        before, 1,
        "home within 15 minutes of the failure refreshed it"
    );
    log.wait_for(&asked, 2).await;
}

#[tokio::test]
async fn a_failed_refresh_keeps_the_rows_and_shows_under_the_home_row_point() {
    let server = enabled("").await;
    wait_for_rows(&server, &["picks", "again", "missing"]).await;

    set_rows(&server, "then-fail").await;
    let failures = wait_for_failures(&server).await;

    let [failure] = failures.as_slice() else {
        panic!("{failures:?}");
    };
    assert_eq!(failure.point, ExtensionPoint::EXTENSION_POINT_HOME_ROW);
    assert!(
        failure
            .last_error
            .contains("test-plugin was set to then-fail"),
        "{}",
        failure.last_error
    );
    let listed: Vec<_> = rows(&server)
        .await
        .into_iter()
        .map(|(_, key, _)| key)
        .collect();
    assert_eq!(
        listed,
        ["picks", "again", "missing"],
        "the rows stored before"
    );
}

#[tokio::test]
async fn a_missing_title_shows_what_the_title_source_gave_and_its_poster_loads_after_a_restart() {
    let server = enabled("picks").await;
    wait_for_rows(&server, &["picks"]).await;

    let picks = titles_from(&browse(&server), "picks").await;
    let Some(MovieOrShow::MissingMovie(dune)) = &picks[1].movie_or_show else {
        panic!("Dune isn't a missing Movie: {:?}", picks[1]);
    };
    let Some(MovieOrShow::MissingShow(dark)) = &picks[3].movie_or_show else {
        panic!("Dark isn't a missing Show: {:?}", picks[3]);
    };
    assert_eq!((dune.title.as_str(), dune.year), ("Dune", Some(2021)));
    assert_eq!((dark.title.as_str(), dark.year), ("Dark", Some(2017)));
    let poster = dark.poster_url.clone().unwrap_or_default();
    let before = fetch(&server, &poster, Some(&server.admin_token)).await;
    assert_eq!(before.status, 200);

    let server = TestServer::builder().tmdb().restart(server).await;
    titles_from(&browse(&server), "picks").await;

    let after = fetch(&server, &poster, Some(&server.admin_token)).await;
    assert_eq!(after.status, 200);
}

#[tokio::test]
async fn a_missing_movie_with_an_open_request_shows_its_state() {
    let server = enabled("picks").await;
    wait_for_rows(&server, &["picks"]).await;
    let ada = server.user_with_default_permissions("Ada").await;
    server.requests(&ada).ask(DUNE).await;

    let picks = titles_from(&browse_as(&server, &ada), "picks").await;

    let Some(MovieOrShow::MissingMovie(dune)) = &picks[1].movie_or_show else {
        panic!("Dune isn't a missing Movie: {:?}", picks[1]);
    };
    let state = dune.request.as_option().map(|request| request.state);
    assert_eq!(state, Some(RequestState::REQUEST_STATE_PENDING.into()));
}

#[tokio::test]
async fn a_user_sees_only_the_missing_titles_of_kinds_they_may_request() {
    let server = enabled("").await;
    wait_for_rows(&server, &["picks", "again", "missing"]).await;
    let grace = server.user("Grace", &[]).await;
    let mia = server.user("Mia", &[Permission::RequestMovie]).await;

    let for_grace = titles_from(&browse_as(&server, &grace), "picks").await;
    let for_mia = titles_from(&browse_as(&server, &mia), "picks").await;

    assert_eq!(
        all_shown(&for_grace),
        [format!("Movie {HEAT}"), format!("Movie {ALIEN}")]
    );
    assert_eq!(
        all_shown(&for_mia),
        [
            format!("Movie {HEAT}"),
            format!("missing Movie {DUNE}"),
            format!("Movie {ALIEN}")
        ]
    );
}

#[tokio::test]
async fn a_row_of_only_missing_titles_is_left_out_for_a_user_who_may_not_request_them() {
    let server = enabled("").await;
    wait_for_rows(&server, &["picks", "again", "missing"]).await;
    let grace = server.user("Grace", &[]).await;

    let for_grace = rows_from(&browse_as(&server, &grace)).await;

    let keys: Vec<&str> = for_grace.iter().map(|(_, key, _)| key.as_str()).collect();
    assert_eq!(keys, ["picks", "again"]);
}

#[tokio::test]
async fn a_title_a_scan_adds_turns_playable_in_its_row_without_a_refresh() {
    let log = Log::global();
    // A setting no other test sets, so only this test's refreshes log it.
    let asked = ["test-plugin gives Home rows \"picks,again\""];
    let server = enabled("picks,again").await;
    wait_for_rows(&server, &["picks", "again"]).await;

    let library = (server)
        .create_library(
            LibraryKind::LIBRARY_KIND_MOVIES,
            &["Dune (2021) {tmdb-438631}/Dune (2021).mkv"],
        )
        .await;
    server.scan_and_match(&library).await;
    let picks = titles_from(&browse(&server), "picks").await;

    assert_eq!(shown(&picks[1]), format!("Movie {DUNE}"));
    assert_eq!(log.count(&asked), 1, "the row was refreshed");
}

#[tokio::test]
async fn opening_home_asks_no_title_source() {
    let server = enabled("").await;
    wait_for_rows(&server, &["picks", "again", "missing"]).await;
    let asked_before = server.tmdb().requests().len();

    rows(&server).await;
    titles_from(&browse(&server), "picks").await;

    assert_eq!(server.tmdb().requests().len(), asked_before);
}

#[tokio::test]
async fn a_refresh_no_title_source_answers_keeps_the_missing_titles_and_tries_again_in_15_minutes()
{
    let log = Log::global();
    // A setting no other test sets, so only this test's refreshes log it.
    let asked = ["test-plugin gives Home rows \"missing,picks\""];
    let unanswered = ["the title source couldn't look up the Plugin's Home row titles"];
    let server = installed(TestServer::builder(), "missing,picks").await;
    enable(&server).await;
    wait_for_rows(&server, &["picks", "missing"]).await;

    // Without TMDB no title source answers the refresh the Plugin coming up starts.
    let server = TestServer::builder().restart(server).await;
    log.wait_for(&unanswered, 1).await;
    let picks = titles_from(&browse(&server), "picks").await;
    rows(&server).await;
    // Time for a refresh home started to call the Plugin.
    tokio::time::sleep(Duration::from_millis(500)).await;
    let before = log.count(&asked);
    server.advance_clock(Duration::from_mins(16));
    rows(&server).await;

    assert_eq!(
        all_shown(&picks),
        [
            format!("Movie {HEAT}"),
            format!("missing Movie {DUNE}"),
            format!("Movie {ALIEN}"),
            format!("missing Show {DARK}"),
        ],
        "the titles the last refresh looked up"
    );
    assert_eq!(before, 2, "home within 15 minutes refreshed it");
    log.wait_for(&asked, 3).await;
}

/// `server` restarted with the test Plugin built with `features` in place of the one it had, as
/// a new version of it is installed.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn restarted_as(server: TestServer, features: &str) -> TestServer {
    let plugin = server.data_dir().join("plugins/test.wasm");
    std::fs::write(plugin, test_plugin::build(features)).unwrap();
    TestServer::builder().tmdb().restart(server).await
}

/// The keys of the Home rows home lists the caller `config` signs in as now.
async fn keys_as(config: ClientConfig) -> Vec<String> {
    let rows = rows_as(config).await;
    rows.into_iter().map(|(_, key, _)| key).collect()
}

#[tokio::test]
async fn a_version_giving_only_shared_rows_shows_them_and_no_users_rows_of_the_old_one() {
    let server = enabled("picks").await;
    let ada = server.user_with_default_permissions("ada").await;
    let bea = server.user_with_default_permissions("bea").await;
    set_handle(&ada, "ada").await;
    set_handle(&bea, "bea").await;
    wait_for_rows_as(ada.client_config(), &["picks", "for-ada"]).await;
    wait_for_rows_as(bea.client_config(), &["picks", "for-bea"]).await;

    let server = restarted_as(server, "home-rows-shared-only").await;

    // Before any listing, which deletes the old version's rows.
    let beas = titles_page_as(server.client_config_as(&bea.token), "for-bea", 10, "").await;
    assert!(beas.unwrap().titles.is_empty(), "the old version's row");
    assert_eq!(
        keys_as(server.client_config_as(&ada.token)).await,
        ["picks"]
    );
    assert_eq!(
        keys_as(server.client_config_as(&bea.token)).await,
        ["picks"]
    );
}

#[tokio::test]
async fn a_version_giving_only_users_rows_shows_theirs_and_no_shared_rows_of_the_old_one() {
    let server = enabled("picks").await;
    let ada = server.user_with_default_permissions("ada").await;
    set_handle(&ada, "ada").await;
    wait_for_rows_as(ada.client_config(), &["picks", "for-ada"]).await;

    let server = restarted_as(server, "home-rows-user-only").await;
    let cy = server.user_with_default_permissions("cy").await;
    set_handle(&cy, "cy").await;

    // Before any listing, which deletes the old version's rows.
    let picks = titles_page_as(server.client_config_as(&ada.token), "picks", 10, "").await;
    assert!(picks.unwrap().titles.is_empty(), "the old version's row");
    assert_eq!(
        keys_as(server.client_config_as(&ada.token)).await,
        ["for-ada"]
    );
    wait_for_rows_as(cy.client_config(), &["for-cy"]).await;
}
