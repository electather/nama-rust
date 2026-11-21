//! The default Home layout an Admin sets, and each User's own over it (ADR 0077), over the API:
//! every User's Home follows the default's order and leaves out what it, or the User, hides, the
//! test Plugin built as a Home row source giving the rows.

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
    BrowseServiceClient, DeleteUserRequest, GetDefaultHomeLayoutRequest, GetHomeLayoutRequest,
    HomeLayout, HomeLayoutRow, KernelHomeRow, LibraryKind, ListFeaturedRequest,
    ListHomeRowsRequest, ListPluginsRequest, PluginHomeRow, RefusalReason, ReorderHomeRowsRequest,
    ResetHomeLayoutRequest, ServerServiceClient, SetDefaultHomeLayoutRequest, SetHomeLayoutRequest,
    UserServiceClient, home_layout_row, home_row::KernelOrPlugin,
};
use refused::assert_refused;
use server::{SignedIn, TestServer, WAIT};

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

fn users(config: ClientConfig) -> UserServiceClient<HttpClient> {
    UserServiceClient::new(HttpClient::plaintext(), config)
}

/// Saves `layout` as `user`'s own Home layout.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn set_own(user: &SignedIn, layout: HomeLayout) -> HomeLayout {
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
async fn reorder(user: &SignedIn, rows: Vec<HomeLayoutRow>) -> HomeLayout {
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
async fn get_own(user: &SignedIn) -> HomeLayout {
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
async fn featured(config: ClientConfig) -> usize {
    let browse = BrowseServiceClient::new(HttpClient::plaintext(), config);
    let listed = browse.list_featured(ListFeaturedRequest::default()).await;
    listed.unwrap().into_owned().titles.len()
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
    assert_eq!(
        featured(server.client_config()).await,
        1,
        "Alien was just added"
    );

    let saved = set_default(&server, layout(true, Vec::new())).await;

    assert!(saved.featured_hidden);
    assert_eq!(featured(server.client_config()).await, 0);
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

#[tokio::test]
async fn a_row_a_user_hides_leaves_their_home_and_no_one_elses() {
    let server = serving_rows().await;
    let ada = server.user_with_default_permissions("ada").await;

    let saved = set_own(
        &ada,
        layout(
            false,
            vec![shared("picks", "", true), shared("again", "", false)],
        ),
    )
    .await;

    let expected = layout(
        false,
        vec![
            continue_watching(false),
            shared("picks", "Picks", true),
            shared("again", "Alien again", false),
            shared("missing", "Not on the Server", false),
        ],
    );
    assert_eq!(saved, expected);
    assert_eq!(get_own(&ada).await, expected);
    assert_eq!(
        rows(ada.client_config()).await,
        ["continue-watching", "again", "missing"]
    );
    assert_eq!(
        rows(server.client_config()).await,
        ["continue-watching", "picks", "again", "missing"]
    );
}

#[tokio::test]
async fn a_user_shows_a_row_the_default_hides_for_them_alone() {
    let server = serving_rows().await;
    let ada = server.user_with_default_permissions("ada").await;
    set_default(&server, layout(false, vec![shared("picks", "", true)])).await;
    assert_eq!(get_own(&ada).await.rows[0], shared("picks", "Picks", true));

    set_own(&ada, layout(false, vec![shared("picks", "", false)])).await;

    assert_eq!(
        rows(ada.client_config()).await,
        ["picks", "continue-watching", "again", "missing"]
    );
    assert_eq!(
        rows(server.client_config()).await,
        ["continue-watching", "again", "missing"]
    );
}

#[tokio::test]
async fn a_user_shows_one_of_their_own_rows_the_default_hides() {
    let server = serving_rows().await;
    let ada = server.user_with_default_permissions("ada").await;
    set_handle(&ada, "ada").await;
    wait_for_rows(
        ada.client_config(),
        &["continue-watching", "picks", "again", "missing", "for-ada"],
    )
    .await;
    set_default(&server, layout(false, vec![users_rows(true)])).await;
    assert_eq!(
        get_own(&ada).await.rows[0],
        shared("for-ada", "For ada", true)
    );

    set_own(&ada, layout(false, vec![shared("for-ada", "", false)])).await;

    assert_eq!(
        rows(ada.client_config()).await,
        ["for-ada", "continue-watching", "picks", "again", "missing"]
    );
}

#[tokio::test]
async fn a_user_hiding_featured_lists_no_titles_to_them_alone() {
    let server = serving_rows().await;
    let ada = server.user_with_default_permissions("ada").await;
    assert_eq!(
        featured(ada.client_config()).await,
        1,
        "Alien was just added"
    );

    let saved = set_own(&ada, layout(true, Vec::new())).await;

    assert!(saved.featured_hidden);
    assert_eq!(featured(ada.client_config()).await, 0);
    assert_eq!(featured(server.client_config()).await, 1);
}

#[tokio::test]
async fn a_user_who_only_hid_rows_follows_a_later_default_order() {
    let server = serving_rows().await;
    let ada = server.user_with_default_permissions("ada").await;
    set_own(&ada, layout(false, vec![shared("picks", "", true)])).await;

    set_default(
        &server,
        layout(
            false,
            vec![
                shared("missing", "", false),
                shared("again", "", false),
                continue_watching(false),
            ],
        ),
    )
    .await;

    assert_eq!(
        rows(ada.client_config()).await,
        ["missing", "again", "continue-watching"]
    );
}

#[tokio::test]
async fn a_user_who_shows_what_the_default_shows_follows_it_when_it_changes() {
    let server = serving_rows().await;
    let ada = server.user_with_default_permissions("ada").await;
    set_own(&ada, layout(false, vec![shared("picks", "", false)])).await;

    set_default(&server, layout(true, vec![shared("picks", "", true)])).await;

    assert_eq!(
        rows(ada.client_config()).await,
        ["continue-watching", "again", "missing"]
    );
    assert_eq!(featured(ada.client_config()).await, 0);
}

#[tokio::test]
async fn a_users_flag_stays_when_they_save_while_the_default_agrees_with_it() {
    let server = serving_rows().await;
    let ada = server.user_with_default_permissions("ada").await;
    set_own(&ada, layout(false, vec![shared("picks", "", true)])).await;
    set_default(&server, layout(false, vec![shared("picks", "", true)])).await;

    set_own(&ada, layout(true, vec![shared("picks", "", true)])).await;
    set_default(&server, layout(false, vec![shared("picks", "", false)])).await;

    assert_eq!(
        rows(ada.client_config()).await,
        ["continue-watching", "again", "missing"]
    );
}

#[tokio::test]
async fn resetting_a_users_home_layout_drops_their_flags() {
    let server = serving_rows().await;
    let ada = server.user_with_default_permissions("ada").await;
    set_own(&ada, layout(true, vec![shared("picks", "", true)])).await;

    users(ada.client_config())
        .reset_home_layout(ResetHomeLayoutRequest::default())
        .await
        .unwrap();

    let own = get_own(&ada).await;
    assert!(!own.featured_hidden);
    assert!(own.rows.iter().all(|row| !row.hidden), "{own:?}");
    assert_eq!(featured(ada.client_config()).await, 1);
}

#[tokio::test]
async fn a_user_reorders_their_home_and_no_one_elses() {
    let server = serving_rows().await;
    let ada = server.user_with_default_permissions("ada").await;

    let saved = reorder(
        &ada,
        vec![
            shared("missing", "", false),
            continue_watching(false),
            shared("again", "", false),
            shared("picks", "", false),
        ],
    )
    .await;

    let expected = layout(
        false,
        vec![
            shared("missing", "Not on the Server", false),
            continue_watching(false),
            shared("again", "Alien again", false),
            shared("picks", "Picks", false),
        ],
    );
    assert_eq!(saved, expected);
    assert_eq!(get_own(&ada).await, expected);
    assert_eq!(
        rows(ada.client_config()).await,
        ["missing", "continue-watching", "again", "picks"]
    );
    assert_eq!(
        rows(server.client_config()).await,
        ["continue-watching", "picks", "again", "missing"]
    );
}

#[tokio::test]
async fn after_a_users_first_reorder_the_defaults_order_no_longer_moves_their_rows() {
    let server = serving_rows().await;
    let ada = server.user_with_default_permissions("ada").await;
    set_handle(&ada, "ada").await;
    wait_for_rows(
        ada.client_config(),
        &["continue-watching", "picks", "again", "missing", "for-ada"],
    )
    .await;
    let order = ["for-ada", "picks", "continue-watching", "again", "missing"];
    reorder(
        &ada,
        vec![
            shared("for-ada", "", false),
            shared("picks", "", false),
            continue_watching(false),
            shared("again", "", false),
            shared("missing", "", false),
        ],
    )
    .await;

    set_default(
        &server,
        layout(
            false,
            vec![
                shared("missing", "", false),
                shared("again", "", false),
                continue_watching(false),
                users_rows(false),
                shared("picks", "", false),
            ],
        ),
    )
    .await;

    assert_eq!(rows(ada.client_config()).await, order);
}

#[tokio::test]
async fn a_row_a_user_has_not_placed_goes_at_the_end_of_their_order() {
    let server = serving_rows().await;
    let ada = server.user_with_default_permissions("ada").await;
    set_default(&server, layout(false, vec![users_rows(false)])).await;
    reorder(
        &ada,
        vec![
            shared("missing", "", false),
            continue_watching(false),
            shared("again", "", false),
            shared("picks", "", false),
        ],
    )
    .await;

    set_handle(&ada, "ada").await;

    wait_for_rows(
        ada.client_config(),
        &["missing", "continue-watching", "again", "picks", "for-ada"],
    )
    .await;
}

#[tokio::test]
async fn reordering_while_a_plugin_is_disabled_keeps_where_its_rows_go_and_their_flags() {
    let server = serving_rows().await;
    let ada = server.user_with_default_permissions("ada").await;
    let missing_first = vec![shared("missing", "", false), continue_watching(false)];
    set_default(&server, layout(false, missing_first)).await;
    set_own(&ada, layout(false, vec![shared("again", "", true)])).await;
    let kept = ["missing", "continue-watching", "picks"];

    disable(&server).await;
    reorder(&ada, vec![continue_watching(false)]).await;
    enable(&server).await;
    wait_for_rows(ada.client_config(), &kept).await;

    let missing_last = vec![continue_watching(false), shared("missing", "", false)];
    set_default(&server, layout(false, missing_last)).await;
    disable(&server).await;
    reorder(&ada, vec![continue_watching(false)]).await;
    enable(&server).await;
    wait_for_rows(ada.client_config(), &kept).await;
    let again = shared("again", "Alien again", true);
    assert!(get_own(&ada).await.rows.contains(&again));
}

#[tokio::test]
async fn a_first_reorder_while_a_plugin_is_disabled_keeps_where_a_users_own_rows_go() {
    let server = serving_rows().await;
    let ada = server.user_with_default_permissions("ada").await;
    set_handle(&ada, "ada").await;
    set_default(&server, layout(false, vec![users_rows(false)])).await;
    let own_first = ["for-ada", "continue-watching", "picks", "again", "missing"];
    wait_for_rows(ada.client_config(), &own_first).await;

    disable(&server).await;
    reorder(&ada, vec![continue_watching(false)]).await;
    enable(&server).await;

    wait_for_rows(ada.client_config(), &own_first).await;
}

#[tokio::test]
async fn resetting_a_users_home_layout_returns_them_to_the_defaults_order() {
    let server = serving_rows().await;
    let ada = server.user_with_default_permissions("ada").await;
    reorder(&ada, vec![shared("missing", "", false)]).await;

    users(ada.client_config())
        .reset_home_layout(ResetHomeLayoutRequest::default())
        .await
        .unwrap();
    set_default(&server, layout(false, vec![shared("again", "", false)])).await;

    assert_eq!(
        rows(ada.client_config()).await,
        ["again", "continue-watching", "picks", "missing"]
    );
}

#[tokio::test]
async fn deleting_a_user_deletes_their_home_layout() {
    let server = serving_rows().await;
    let ada = server.user_with_default_permissions("ada").await;
    set_own(&ada, layout(true, Vec::new())).await;

    users(server.client_config())
        .delete_user(DeleteUserRequest {
            user_id: ada.id,
            ..Default::default()
        })
        .await
        .unwrap();

    let stored: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_home_layouts")
        .fetch_one(server.db().reader())
        .await
        .unwrap();
    assert_eq!(stored, 0);
}

#[tokio::test]
async fn uninstalling_a_plugin_drops_its_rows_from_users_layouts() {
    let server = serving_rows().await;
    let ada = server.user_with_default_permissions("ada").await;
    set_own(&ada, layout(false, vec![shared("missing", "", true)])).await;

    disable(&server).await;
    std::fs::remove_file(server.data_dir().join("plugins/test.wasm")).unwrap();
    (server.plugin_service())
        .list_plugins(ListPluginsRequest::default())
        .await
        .unwrap();
    server.install_test_plugin("home-rows").await;
    enable(&server).await;

    wait_for_rows(
        ada.client_config(),
        &["continue-watching", "picks", "again", "missing"],
    )
    .await;
}

#[tokio::test]
async fn saving_a_users_home_layout_without_one_is_refused() {
    let server = TestServer::builder().start().await;
    let ada = server.user_with_default_permissions("ada").await;

    let saved = users(ada.client_config())
        .set_home_layout(SetHomeLayoutRequest::default())
        .await;

    let refusal = assert_refused(
        saved,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
    );
    assert_eq!(refusal.field, "layout");
}
