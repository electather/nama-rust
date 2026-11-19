//! Home rows a Plugin gives, every User's and each User's own, over the API (ADR 0058): the test
//! Plugin built as a Home row source, refreshed in the background, its rows showing the titles on
//! the Server.

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

use connectrpc::{ErrorCode, client::ClientConfig, client::HttpClient};
use log::Log;
use nama_proto::nama::v1::{
    BrowseServiceClient, DisablePluginRequest, EnablePluginRequest, HomeRowTitle, LibraryKind,
    ListHomeRowTitlesRequest, ListHomeRowTitlesResponse, ListHomeRowsRequest, ListPluginsRequest,
    PageRequest, PluginServiceClient, RefusalReason, SettingValue, UpdatePersonalSettingsRequest,
    UpdatePluginSettingsRequest, UserServiceClient, home_row_title::MovieOrShow,
    setting_value::Value,
};
use refused::assert_refused;
use server::{
    Builder, SignedIn, TestServer, WAIT,
    tmdb::{ALIEN, HEAT},
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

fn text(text: &str) -> SettingValue {
    SettingValue {
        value: Some(Value::TextValue(text.to_owned())),
        ..Default::default()
    }
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

/// Saves `handle` as `user`'s `handle` Personal setting for the test Plugin.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn set_handle(user: &SignedIn, handle: &str) {
    UserServiceClient::new(HttpClient::plaintext(), user.client_config())
        .update_personal_settings(UpdatePersonalSettingsRequest {
            plugin_id: PLUGIN.to_owned(),
            settings: std::iter::once(("handle".to_owned(), text(handle))).collect(),
            ..Default::default()
        })
        .await
        .unwrap();
}

/// The Home rows home lists the Admin now, each as its Plugin, key and heading.
async fn rows(server: &TestServer) -> Vec<(String, String, String)> {
    rows_as(server.client_config()).await
}

/// The Home rows home lists the caller `config` signs in as now, as [`rows`] gives them.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn rows_as(config: ClientConfig) -> Vec<(String, String, String)> {
    let listed = BrowseServiceClient::new(HttpClient::plaintext(), config)
        .list_home_rows(ListHomeRowsRequest::default())
        .await
        .unwrap()
        .into_owned();
    (listed.rows.into_iter())
        .map(|row| (row.plugin_id, row.key, row.heading))
        .collect()
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

async fn titles_page(
    server: &TestServer,
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
    Ok(browse(server)
        .list_home_row_titles(asked)
        .await?
        .into_owned())
}

/// The TMDB id of a Movie in a row, or what else it is.
fn tmdb(title: &HomeRowTitle) -> String {
    match &title.movie_or_show {
        Some(MovieOrShow::Movie(movie)) => movie.id.value.clone(),
        other => format!("not a Movie: {other:?}"),
    }
}

#[tokio::test]
async fn enabling_a_home_row_source_shows_its_rows_with_only_the_titles_on_the_server() {
    let server = enabled("").await;

    wait_for_rows(&server, &["picks", "again"]).await;
    let picks = titles_page(&server, "picks", 0, "").await.unwrap();

    assert_eq!(
        rows(&server).await,
        [
            (PLUGIN.to_owned(), "picks".to_owned(), "Picks".to_owned()),
            (
                PLUGIN.to_owned(),
                "again".to_owned(),
                "Alien again".to_owned()
            ),
        ],
        "the row with no title on the Server is left out"
    );
    let ids: Vec<String> = picks.titles.iter().map(tmdb).collect();
    assert_eq!(
        ids,
        [HEAT.to_string(), ALIEN.to_string()],
        "the Plugin's order, without Dune"
    );
    assert_eq!(picks.page.total_size, Some(2));
}

#[tokio::test]
async fn each_user_sees_their_own_rows_built_from_their_personal_settings() {
    let server = enabled("").await;
    let ada = server.user_with_default_permissions("ada").await;
    let bea = server.user_with_default_permissions("bea").await;
    set_handle(&ada, "ada").await;
    set_handle(&bea, "bea").await;

    wait_for_rows_as(ada.client_config(), &["picks", "again", "for-ada"]).await;
    wait_for_rows_as(bea.client_config(), &["picks", "again", "for-bea"]).await;

    let headings = |rows: Vec<(String, String, String)>| -> Vec<String> {
        rows.into_iter().map(|(_, _, heading)| heading).collect()
    };
    assert_eq!(
        headings(rows_as(ada.client_config()).await),
        ["Picks", "Alien again", "For ada"]
    );
    assert_eq!(
        headings(rows_as(bea.client_config()).await),
        ["Picks", "Alien again", "For bea"]
    );
}

#[tokio::test]
async fn a_row_pages_by_keyset_and_refuses_another_rows_token() {
    let server = enabled("").await;
    wait_for_rows(&server, &["picks", "again"]).await;

    let first = titles_page(&server, "picks", 1, "").await.unwrap();
    let second = (titles_page(&server, "picks", 1, &first.page.next_token).await).unwrap();
    let other_row = titles_page(&server, "again", 1, &first.page.next_token).await;

    assert_eq!(
        first.titles.iter().map(tmdb).collect::<Vec<_>>(),
        [HEAT.to_string()]
    );
    assert_eq!(
        second.titles.iter().map(tmdb).collect::<Vec<_>>(),
        [ALIEN.to_string()]
    );
    assert_eq!(second.page.next_token, "", "the last page");
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
    wait_for_rows(&server, &["picks", "again"]).await;

    (plugins(&server))
        .disable_plugin(DisablePluginRequest {
            id: PLUGIN.to_owned(),
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(rows(&server).await, []);
    let picks = titles_page(&server, "picks", 0, "").await.unwrap();
    assert_eq!(picks.titles.len(), 0);
}

#[tokio::test]
async fn a_row_the_plugin_stops_giving_is_gone_once_its_settings_change_refreshes_it() {
    let server = enabled("").await;
    wait_for_rows(&server, &["picks", "again"]).await;

    set_rows(&server, "picks").await;

    wait_for_rows(&server, &["picks"]).await;
}

#[tokio::test]
async fn home_lists_the_stored_rows_without_waiting_for_a_refresh() {
    let log = Log::global();
    let server = enabled("").await;
    wait_for_rows(&server, &["picks", "again"]).await;

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
    assert_eq!(listed.len(), 2, "the rows stored before: {listed:?}");
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
