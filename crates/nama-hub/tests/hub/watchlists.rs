//! Linked Servers syncing their Users' Watchlists and reading the most watchlisted, against a real
//! PostgreSQL.

use crate::support::hub::TestHub;
use crate::support::linking::*;
use crate::support::refused::assert_refused;
use connectrpc::ErrorCode;
use nama_hub_proto::nama::hub::v1::{
    ListMostWatchlistedTitlesRequest, RefusalReason, SyncWatchlistRequest, TitleKind,
    WatchlistEntry,
};
use sqlx::PgPool;

/// The Watchlist entry of the Movie `tmdb-603`, on the Watchlist since `changed_at_ms`.
fn the_matrix(changed_at_ms: i64) -> WatchlistEntry {
    WatchlistEntry {
        kind: TitleKind::TITLE_KIND_MOVIE.into(),
        id: "tmdb-603".to_owned(),
        changed_at_ms,
        ..Default::default()
    }
}

/// Syncs `entries` as the Watchlist of the User `user_id` of the Server presenting `credential`,
/// and answers when each entry of the Account's changed.
async fn sync_watchlist(
    hub: &TestHub,
    credential: &str,
    user_id: i64,
    entries: Vec<WatchlistEntry>,
) -> Result<Vec<i64>, connectrpc::ConnectError> {
    let synced = linked_server(hub, credential)
        .sync_watchlist(SyncWatchlistRequest {
            user_id,
            entries,
            ..Default::default()
        })
        .await?
        .into_owned();
    Ok((synced.entries.iter())
        .map(|entry| entry.changed_at_ms)
        .collect())
}

// Which change wins is checked where it merges, in `src/sync/sync_tests.rs`.
#[sqlx::test(migrations = false)]
async fn a_linked_server_syncs_a_watchlist_only_with_the_account_its_user_is_linked_to(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let bea = create_proven(&hub, &mut inbox, "bea@example.com").await;
    let server = linked(&hub, &ada, "Ada's").await;
    let ada_code = start_user(&hub, &server, 7).await.unwrap();
    approve_user(&hub, &ada, &ada_code).await.unwrap();
    let bea_code = start_user(&hub, &server, 8).await.unwrap();
    approve_user(&hub, &bea, &bea_code).await.unwrap();

    let synced = sync_watchlist(&hub, &server, 8, vec![the_matrix(1_000)]).await;
    assert_eq!(synced.unwrap(), [1_000]);
    let not_linked = sync_watchlist(&hub, &server, 9, vec![the_matrix(2_000)]).await;
    assert_eq!(not_linked.unwrap(), Vec::<i64>::new());
    assert_eq!(
        sync_watchlist(&hub, &server, 7, Vec::new()).await.unwrap(),
        Vec::<i64>::new(),
        "Ada gets none of Bea's Watchlist, nor User 9's"
    );
    let unspecified = WatchlistEntry {
        kind: TitleKind::TITLE_KIND_UNSPECIFIED.into(),
        ..the_matrix(1_000)
    };
    let refused = assert_refused(
        sync_watchlist(&hub, &server, 7, vec![the_matrix(1_000), unspecified]).await,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_WATCHLIST_INVALID,
    );
    assert_eq!(refused.field, "entries[1].kind");
}

// Each limit's edge is checked where it is kept, in `src/sync/sync_tests.rs`.
#[sqlx::test(migrations = false)]
async fn a_watchlist_breaking_a_limit_is_refused_even_of_a_user_who_isnt_linked(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let server = linked(&hub, &ada, "Ada's").await;
    let long_id = WatchlistEntry {
        id: format!("tmdb-{}", "6".repeat(96)),
        ..the_matrix(1_000)
    };

    let refused = assert_refused(
        sync_watchlist(&hub, &server, 7, vec![the_matrix(1_000), long_id]).await,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_WATCHLIST_INVALID,
    );
    assert_eq!(refused.field, "entries[1].id");
    assert_eq!(refused.params["maxIdLength"], "100");
    let without_a_namespace = WatchlistEntry {
        id: "603".to_owned(),
        ..the_matrix(1_000)
    };
    let malformed = assert_refused(
        sync_watchlist(
            &hub,
            &server,
            7,
            vec![the_matrix(1_000), without_a_namespace],
        )
        .await,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_WATCHLIST_INVALID,
    );
    assert_eq!(malformed.field, "entries[1].id");
}

// How many Accounts it takes is checked where they're counted, in `src/sync/sync_tests.rs`.
#[sqlx::test(migrations = false)]
async fn a_title_on_one_accounts_watchlist_isnt_most_watchlisted(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let server = linked(&hub, &ada, "Ada's").await;
    let code = start_user(&hub, &server, 7).await.unwrap();
    approve_user(&hub, &ada, &code).await.unwrap();
    sync_watchlist(&hub, &server, 7, vec![the_matrix(1_000)])
        .await
        .unwrap();

    let watchlisted = linked_server(&hub, &server)
        .list_most_watchlisted_titles(ListMostWatchlistedTitlesRequest::default())
        .await
        .unwrap();

    assert_eq!(watchlisted.into_owned().titles, []);
}
