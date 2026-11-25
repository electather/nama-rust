//! Each User's own order of their Home rows over the default Home layout (ADR 0077): their
//! first reorder copies the default's order, which from then on no longer moves their rows.

#[path = "support/home_layouts.rs"]
#[expect(
    dead_code,
    reason = "this binary uses only some of the Home layout helpers"
)]
mod home_layouts;
#[path = "support/home_row_plugin.rs"]
mod home_row_plugin;
#[path = "support/installed_test_plugin.rs"]
#[expect(
    dead_code,
    reason = "this binary enables the test Plugin itself, after its settings"
)]
mod installed_test_plugin;
#[path = "support/server.rs"]
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
mod server;
#[path = "support/test_plugin.rs"]
mod test_plugin;

use home_layouts::{
    continue_watching, get_own, layout, recently_added, reorder, rows, serving_rows, set_default,
    set_own, shared, users, users_rows, wait_for_rows,
};
use home_row_plugin::{disable, enable, set_handle};
use nama_proto::nama::v1::ResetHomeLayoutRequest;

#[tokio::test]
async fn a_user_reorders_their_home_and_no_one_elses() {
    let server = serving_rows().await;
    let ada = server.user_with_default_permissions("ada").await;

    let saved = reorder(
        &ada,
        vec![
            recently_added(&server, false).await,
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
            recently_added(&server, false).await,
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
        [
            "recently-added",
            "missing",
            "continue-watching",
            "again",
            "picks"
        ]
    );
    assert_eq!(
        rows(server.client_config()).await,
        [
            "continue-watching",
            "recently-added",
            "picks",
            "again",
            "missing"
        ]
    );
}

#[tokio::test]
async fn after_a_users_first_reorder_the_defaults_order_no_longer_moves_their_rows() {
    let server = serving_rows().await;
    let ada = server.user_with_default_permissions("ada").await;
    set_handle(&ada, "ada").await;
    wait_for_rows(
        ada.client_config(),
        &[
            "continue-watching",
            "recently-added",
            "picks",
            "again",
            "missing",
            "for-ada",
        ],
    )
    .await;
    let order = [
        "for-ada",
        "picks",
        "continue-watching",
        "recently-added",
        "again",
        "missing",
    ];
    reorder(
        &ada,
        vec![
            shared("for-ada", "", false),
            shared("picks", "", false),
            continue_watching(false),
            recently_added(&server, false).await,
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
            recently_added(&server, false).await,
            shared("again", "", false),
            shared("picks", "", false),
        ],
    )
    .await;

    set_handle(&ada, "ada").await;

    wait_for_rows(
        ada.client_config(),
        &[
            "missing",
            "continue-watching",
            "recently-added",
            "again",
            "picks",
            "for-ada",
        ],
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
    let kept = ["missing", "continue-watching", "recently-added", "picks"];
    let kernel_rows = vec![
        continue_watching(false),
        recently_added(&server, false).await,
    ];

    disable(&server).await;
    reorder(&ada, kernel_rows.clone()).await;
    enable(&server).await;
    wait_for_rows(ada.client_config(), &kept).await;

    let missing_last = vec![continue_watching(false), shared("missing", "", false)];
    set_default(&server, layout(false, missing_last)).await;
    disable(&server).await;
    reorder(&ada, kernel_rows.clone()).await;
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
    let own_first = [
        "for-ada",
        "continue-watching",
        "recently-added",
        "picks",
        "again",
        "missing",
    ];
    wait_for_rows(ada.client_config(), &own_first).await;

    disable(&server).await;
    reorder(
        &ada,
        vec![
            continue_watching(false),
            recently_added(&server, false).await,
        ],
    )
    .await;
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
        [
            "again",
            "continue-watching",
            "recently-added",
            "picks",
            "missing"
        ]
    );
}
