//! The default Home layout an Admin sets, and each User's own over it (ADR 0077), over the API:
//! every User's Home follows the default's order and leaves out what it, or the User, hides, the
//! test Plugin built as a Home row source giving the rows.

#[path = "support/home_layouts.rs"]
mod home_layouts;
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

use connectrpc::ErrorCode;
use home_layouts::{
    continue_watching, featured, get_default, get_own, layout, reorder, rows, serving_rows,
    set_default, set_own, settings, shared, users, users_rows, wait_for_rows,
};
use home_row_plugin::{disable, enable, set_handle};
use nama_proto::nama::v1::{
    DeleteUserRequest, GetDefaultHomeLayoutRequest, ListPluginsRequest, RefusalReason,
    ResetHomeLayoutRequest, SetDefaultHomeLayoutRequest, SetHomeLayoutRequest,
};
use refused::assert_refused;
use server::TestServer;

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
