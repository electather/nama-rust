//! The default Home layout an Admin sets (ADR 0077), over the API: every User's Home follows its
//! order and leaves out what it hides, the test Plugin built as a Home row source giving the rows.

#[path = "support/home_row_plugin.rs"]
mod home_row_plugin;
#[path = "support/installed_test_plugin.rs"]
#[expect(
    dead_code,
    reason = "this binary enables the test Plugin itself, after its settings"
)]
mod installed_test_plugin;
#[path = "support/refused.rs"]
mod refused;
#[path = "support/server.rs"]
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
mod server;
#[path = "support/test_plugin.rs"]
mod test_plugin;

use std::time::{Duration, Instant};

use connectrpc::{
    ErrorCode,
    client::{ClientConfig, HttpClient},
};
use home_row_plugin::{PLUGIN, disable, enable, set_handle};
use nama_proto::nama::v1::{
    BrowseServiceClient, GetDefaultHomeLayoutRequest, HomeLayout, HomeLayoutRow, KernelHomeRow,
    LibraryKind, ListFeaturedRequest, ListHomeRowsRequest, ListPluginsRequest, PluginHomeRow,
    RefusalReason, ServerServiceClient, SetDefaultHomeLayoutRequest, home_layout_row,
    home_row::KernelOrPlugin,
};
use refused::assert_refused;
use server::{TestServer, WAIT};

/// A Server with the fake TMDB, a Movies Library of Alien, and the test Plugin built as a Home
/// row source and enabled, giving its shared rows `picks`, `again` and `missing`.
async fn serving_rows() -> TestServer {
    let server = TestServer::builder().tmdb().start().await;
    let library = (server)
        .create_library(
            LibraryKind::LIBRARY_KIND_MOVIES,
            &["Alien (1979)/Alien (1979).mkv"],
        )
        .await;
    server.scan_and_match(&library).await;
    server.install_test_plugin("home-rows").await;
    enable(&server).await;
    wait_for_rows(
        server.client_config(),
        &["continue-watching", "picks", "again", "missing"],
    )
    .await;
    server
}

fn settings(config: ClientConfig) -> ServerServiceClient<HttpClient> {
    ServerServiceClient::new(HttpClient::plaintext(), config)
}

/// Each Home row the caller `config` signs in as sees now: `continue-watching`, or a Plugin
/// row's key.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn rows(config: ClientConfig) -> Vec<String> {
    let listed = BrowseServiceClient::new(HttpClient::plaintext(), config)
        .list_home_rows(ListHomeRowsRequest::default())
        .await;
    (listed.unwrap().into_owned().rows.into_iter())
        .map(|row| match row.kernel_or_plugin {
            Some(KernelOrPlugin::Plugin(row)) => row.key,
            _ => "continue-watching".to_owned(),
        })
        .collect()
}

/// Lists the Home rows of the caller `config` signs in as until they are `keys`, as [`rows`]
/// names them, since a Plugin's rows are refreshed in the background.
async fn wait_for_rows(config: ClientConfig, keys: &[&str]) {
    let started = Instant::now();
    loop {
        let listed = rows(config.clone()).await;
        if listed.iter().map(String::as_str).eq(keys.iter().copied()) {
            return;
        }
        assert!(
            started.elapsed() < WAIT,
            "home lists {listed:?}, not {keys:?}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

fn continue_watching(hidden: bool) -> HomeLayoutRow {
    HomeLayoutRow {
        row: Some(home_layout_row::Row::Kernel(
            KernelHomeRow::KERNEL_HOME_ROW_CONTINUE_WATCHING.into(),
        )),
        hidden,
        ..Default::default()
    }
}

/// The test Plugin's shared row `key`, headed `heading` as `GetDefaultHomeLayout` returns it.
fn shared(key: &str, heading: &str, hidden: bool) -> HomeLayoutRow {
    HomeLayoutRow {
        row: Some(home_layout_row::Row::Plugin(Box::new(PluginHomeRow {
            plugin_id: PLUGIN.to_owned(),
            key: key.to_owned(),
            heading: heading.to_owned(),
            ..Default::default()
        }))),
        hidden,
        ..Default::default()
    }
}

/// The test Plugin's entry for each User's own rows.
fn users_rows(hidden: bool) -> HomeLayoutRow {
    HomeLayoutRow {
        row: Some(home_layout_row::Row::PluginUsersRows(PLUGIN.to_owned())),
        hidden,
        ..Default::default()
    }
}

fn layout(featured_hidden: bool, rows: Vec<HomeLayoutRow>) -> HomeLayout {
    HomeLayout {
        featured_hidden,
        rows,
        ..Default::default()
    }
}

/// Saves `layout` as the default Home layout, as the Admin.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn set_default(server: &TestServer, layout: HomeLayout) -> HomeLayout {
    let saved = settings(server.client_config())
        .set_default_home_layout(SetDefaultHomeLayoutRequest {
            layout: layout.into(),
            ..Default::default()
        })
        .await;
    saved.unwrap().into_owned().layout.into_option().unwrap()
}

/// The default Home layout as the Admin gets it.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn get_default(server: &TestServer) -> HomeLayout {
    let got = settings(server.client_config())
        .get_default_home_layout(GetDefaultHomeLayoutRequest::default())
        .await;
    got.unwrap().into_owned().layout.into_option().unwrap()
}

#[tokio::test]
async fn the_default_home_layout_needs_manage_settings() {
    let server = TestServer::builder().start().await;
    let ada = server.user_with_default_permissions("ada").await;

    let got = settings(ada.client_config())
        .get_default_home_layout(GetDefaultHomeLayoutRequest::default())
        .await;
    let set = settings(ada.client_config())
        .set_default_home_layout(SetDefaultHomeLayoutRequest {
            layout: layout(true, Vec::new()).into(),
            ..Default::default()
        })
        .await;

    for refused in [got.map(drop), set.map(drop)] {
        assert_refused(
            refused,
            ErrorCode::PermissionDenied,
            RefusalReason::REFUSAL_REASON_PERMISSION_REQUIRED,
        );
    }
}

#[tokio::test]
async fn every_users_home_follows_the_default_order_and_leaves_out_what_it_hides() {
    let server = serving_rows().await;
    let ada = server.user_with_default_permissions("ada").await;

    set_default(
        &server,
        layout(
            false,
            vec![
                shared("missing", "Not on the Server", false),
                shared("picks", "Picks", true),
                continue_watching(false),
                shared("again", "Alien again", false),
            ],
        ),
    )
    .await;

    assert_eq!(
        rows(ada.client_config()).await,
        ["missing", "continue-watching", "again"]
    );
    assert_eq!(
        rows(server.client_config()).await,
        ["missing", "continue-watching", "again"]
    );
}

#[tokio::test]
async fn a_hidden_featured_lists_no_titles() {
    let server = serving_rows().await;
    let featured = || async {
        let browse = BrowseServiceClient::new(HttpClient::plaintext(), server.client_config());
        let listed = browse.list_featured(ListFeaturedRequest::default()).await;
        listed.unwrap().into_owned().titles.len()
    };
    assert_eq!(featured().await, 1, "Alien was just added");

    let saved = set_default(&server, layout(true, Vec::new())).await;

    assert!(saved.featured_hidden);
    assert_eq!(featured().await, 0);
}

#[tokio::test]
async fn a_row_the_default_does_not_place_is_listed_at_the_end_shown() {
    let server = serving_rows().await;

    let saved = set_default(&server, layout(false, vec![shared("again", "", true)])).await;

    let expected = layout(
        false,
        vec![
            shared("again", "Alien again", true),
            continue_watching(false),
            shared("picks", "Picks", false),
            shared("missing", "Not on the Server", false),
            users_rows(false),
        ],
    );
    assert_eq!(saved, expected);
    assert_eq!(get_default(&server).await, expected);
    assert_eq!(
        rows(server.client_config()).await,
        ["continue-watching", "picks", "missing"]
    );
}

#[tokio::test]
async fn each_users_own_rows_go_where_their_plugins_entry_is() {
    let server = serving_rows().await;
    let ada = server.user_with_default_permissions("ada").await;
    set_handle(&ada, "ada").await;
    wait_for_rows(
        ada.client_config(),
        &["continue-watching", "picks", "again", "missing", "for-ada"],
    )
    .await;

    set_default(&server, layout(false, vec![users_rows(false)])).await;
    assert_eq!(
        rows(ada.client_config()).await,
        ["for-ada", "continue-watching", "picks", "again", "missing"]
    );

    set_default(&server, layout(false, vec![users_rows(true)])).await;
    assert_eq!(
        rows(ada.client_config()).await,
        ["continue-watching", "picks", "again", "missing"]
    );
}

#[tokio::test]
async fn uninstalling_a_plugin_drops_its_rows_from_the_default() {
    let server = serving_rows().await;
    set_default(
        &server,
        layout(
            false,
            vec![shared("missing", "", true), continue_watching(false)],
        ),
    )
    .await;

    disable(&server).await;
    std::fs::remove_file(server.data_dir().join("plugins/test.wasm")).unwrap();
    (server.plugin_service())
        .list_plugins(ListPluginsRequest::default())
        .await
        .unwrap();
    server.install_test_plugin("home-rows").await;
    enable(&server).await;

    wait_for_rows(
        server.client_config(),
        &["continue-watching", "picks", "again", "missing"],
    )
    .await;
    assert_eq!(
        get_default(&server).await.rows.first(),
        Some(&continue_watching(false))
    );
}

#[tokio::test]
async fn saving_while_a_plugin_is_disabled_keeps_its_rows_where_they_were() {
    let server = serving_rows().await;
    set_default(
        &server,
        layout(
            false,
            vec![
                shared("missing", "", true),
                continue_watching(false),
                shared("again", "", true),
            ],
        ),
    )
    .await;
    disable(&server).await;

    let seen = get_default(&server).await;
    assert_eq!(seen.rows, [continue_watching(false)]);
    set_default(&server, layout(false, vec![continue_watching(true)])).await;
    enable(&server).await;

    wait_for_rows(server.client_config(), &["picks"]).await;
    assert_eq!(
        get_default(&server).await.rows,
        [
            shared("missing", "Not on the Server", true),
            continue_watching(true),
            shared("again", "Alien again", true),
            shared("picks", "Picks", false),
            users_rows(false),
        ]
    );
}
