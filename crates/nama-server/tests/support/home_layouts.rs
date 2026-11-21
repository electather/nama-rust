//! A Server whose Home rows come from the test Plugin built as a Home row source, and the
//! default and Users' own Home layouts saved and read over the API.

use std::time::{Duration, Instant};

use connectrpc::client::{ClientConfig, HttpClient};
use nama_proto::nama::v1::{
    BrowseServiceClient, GetDefaultHomeLayoutRequest, GetHomeLayoutRequest, HomeLayout,
    HomeLayoutRow, KernelHomeRow, LibraryKind, ListFeaturedRequest, ListHomeRowsRequest,
    PluginHomeRow, ReorderHomeRowsRequest, ServerServiceClient, SetDefaultHomeLayoutRequest,
    SetHomeLayoutRequest, UserServiceClient, home_layout_row, home_row::KernelOrPlugin,
};

use crate::{
    home_row_plugin::{PLUGIN, enable},
    server::{SignedIn, TestServer, WAIT},
};

/// A Server with the fake TMDB, a Movies Library of Alien, and the test Plugin built as a Home
/// row source and enabled, giving its shared rows `picks`, `again` and `missing`.
pub(crate) async fn serving_rows() -> TestServer {
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

pub(crate) fn settings(config: ClientConfig) -> ServerServiceClient<HttpClient> {
    ServerServiceClient::new(HttpClient::plaintext(), config)
}

/// Each Home row the caller `config` signs in as sees now: `continue-watching`, or a Plugin
/// row's key.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
pub(crate) async fn rows(config: ClientConfig) -> Vec<String> {
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
pub(crate) async fn wait_for_rows(config: ClientConfig, keys: &[&str]) {
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

pub(crate) fn continue_watching(hidden: bool) -> HomeLayoutRow {
    HomeLayoutRow {
        row: Some(home_layout_row::Row::Kernel(
            KernelHomeRow::KERNEL_HOME_ROW_CONTINUE_WATCHING.into(),
        )),
        hidden,
        ..Default::default()
    }
}

/// The test Plugin's shared row `key`, headed `heading` as `GetDefaultHomeLayout` returns it.
pub(crate) fn shared(key: &str, heading: &str, hidden: bool) -> HomeLayoutRow {
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
pub(crate) fn users_rows(hidden: bool) -> HomeLayoutRow {
    HomeLayoutRow {
        row: Some(home_layout_row::Row::PluginUsersRows(PLUGIN.to_owned())),
        hidden,
        ..Default::default()
    }
}

pub(crate) fn layout(featured_hidden: bool, rows: Vec<HomeLayoutRow>) -> HomeLayout {
    HomeLayout {
        featured_hidden: Some(featured_hidden),
        rows,
        ..Default::default()
    }
}

/// A layout of `rows` that leaves Featured unset.
pub(crate) fn rows_only(rows: Vec<HomeLayoutRow>) -> HomeLayout {
    HomeLayout {
        rows,
        ..Default::default()
    }
}

/// Saves `layout` as the default Home layout, as the Admin.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
pub(crate) async fn set_default(server: &TestServer, layout: HomeLayout) -> HomeLayout {
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
pub(crate) async fn get_default(server: &TestServer) -> HomeLayout {
    let got = settings(server.client_config())
        .get_default_home_layout(GetDefaultHomeLayoutRequest::default())
        .await;
    got.unwrap().into_owned().layout.into_option().unwrap()
}

pub(crate) fn users(config: ClientConfig) -> UserServiceClient<HttpClient> {
    UserServiceClient::new(HttpClient::plaintext(), config)
}

/// Saves `layout` as `user`'s own Home layout.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
pub(crate) async fn set_own(user: &SignedIn, layout: HomeLayout) -> HomeLayout {
    let saved = users(user.client_config())
        .set_home_layout(SetHomeLayoutRequest {
            layout: layout.into(),
            ..Default::default()
        })
        .await;
    saved.unwrap().into_owned().layout.into_option().unwrap()
}

/// Saves `rows` as `user`'s order of their Home rows.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
pub(crate) async fn reorder(user: &SignedIn, rows: Vec<HomeLayoutRow>) -> HomeLayout {
    let saved = users(user.client_config())
        .reorder_home_rows(ReorderHomeRowsRequest {
            rows,
            ..Default::default()
        })
        .await;
    saved.unwrap().into_owned().layout.into_option().unwrap()
}

/// `user`'s own Home layout.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
pub(crate) async fn get_own(user: &SignedIn) -> HomeLayout {
    let got = users(user.client_config())
        .get_home_layout(GetHomeLayoutRequest::default())
        .await;
    got.unwrap().into_owned().layout.into_option().unwrap()
}

/// How many titles Featured lists to the caller `config` signs in as.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
pub(crate) async fn featured(config: ClientConfig) -> usize {
    let browse = BrowseServiceClient::new(HttpClient::plaintext(), config);
    let listed = browse.list_featured(ListFeaturedRequest::default()).await;
    listed.unwrap().into_owned().titles.len()
}
