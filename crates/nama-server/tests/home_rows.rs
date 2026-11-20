//! Home rows a Plugin gives every User, over the API (ADR 0058): the test Plugin built as a Home
//! row source, refreshed in the background, its rows showing the titles on the Server and those
//! missing from it to Request.

#[path = "support/http.rs"]
#[expect(dead_code, reason = "this binary reads only a response's status")]
mod http;
#[path = "support/log.rs"]
mod log;
#[path = "support/refused.rs"]
mod refused;
#[path = "support/server.rs"]
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
mod server;
#[path = "support/test_plugin.rs"]
mod test_plugin;

use std::time::{Duration, Instant};

use connectrpc::{ErrorCode, client::HttpClient};
use http::fetch;
use log::Log;
use nama_proto::nama::v1::{
    BrowseServiceClient, DisablePluginRequest, EnablePluginRequest, ExtensionPoint,
    ExtensionPointFailure, GetPluginRequest, HomeRowTitle, LibraryKind, ListHomeRowTitlesRequest,
    ListHomeRowTitlesResponse, ListHomeRowsRequest, ListPluginsRequest, PageRequest, Permission,
    PluginServiceClient, RefusalReason, RequestState, SettingValue, UpdatePluginSettingsRequest,
    home_row_title::MovieOrShow, setting_value::Value,
};
use refused::assert_refused;
use server::{
    Builder, SignedIn, TestServer, WAIT,
    requests::Requester,
    tmdb::{ALIEN, DARK, DUNE, HEAT},
};

const PLUGIN: &str = "test-plugin";

/// A Server with the fake TMDB, a Movies Library of Alien and Heat, and the test Plugin built as a
/// Home row source, its `home_rows` setting `rows`, not yet enabled.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
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
    let dir = server.data_dir().join("plugins");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("test.wasm"), test_plugin::build("home-rows")).unwrap();
    // Listing is what reads `plugins/`.
    (plugins(&server))
        .list_plugins(ListPluginsRequest::default())
        .await
        .unwrap();
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

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn enable(server: &TestServer) {
    (plugins(server))
        .enable_plugin(EnablePluginRequest {
            id: PLUGIN.to_owned(),
            ..Default::default()
        })
        .await
        .unwrap();
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn disable(server: &TestServer) {
    (plugins(server))
        .disable_plugin(DisablePluginRequest {
            id: PLUGIN.to_owned(),
            ..Default::default()
        })
        .await
        .unwrap();
}

/// Saves `rows` as the test Plugin's `home_rows` setting, the keys of the rows it gives.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn set_rows(server: &TestServer, rows: &str) {
    let value = SettingValue {
        value: Some(Value::TextValue(rows.to_owned())),
        ..Default::default()
    };
    (plugins(server))
        .update_plugin_settings(UpdatePluginSettingsRequest {
            id: PLUGIN.to_owned(),
            settings: std::iter::once(("home_rows".to_owned(), value)).collect(),
            ..Default::default()
        })
        .await
        .unwrap();
}

/// The Home rows home lists now, each as its Plugin, key and heading.
async fn rows(server: &TestServer) -> Vec<(String, String, String)> {
    rows_from(&browse(server)).await
}

/// The Home rows home lists `browse`'s caller now, as [`rows`] lists them.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn rows_from(browse: &BrowseServiceClient<HttpClient>) -> Vec<(String, String, String)> {
    let listed = browse
        .list_home_rows(ListHomeRowsRequest::default())
        .await
        .unwrap()
        .into_owned();
    (listed.rows.into_iter())
        .map(|row| (row.plugin_id, row.key, row.heading))
        .collect()
}

/// Lists the Home rows until they are the rows `keys` name, since a refresh runs in the
/// background.
async fn wait_for_rows(server: &TestServer, keys: &[&str]) {
    let started = Instant::now();
    loop {
        let listed = rows(server).await;
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
    let server = installed(TestServer::builder().moves_clock(), "again,picks").await;
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
    let server = installed(TestServer::builder().moves_clock(), "fail").await;
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
    let server = installed(TestServer::builder().moves_clock(), "missing,picks").await;
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
