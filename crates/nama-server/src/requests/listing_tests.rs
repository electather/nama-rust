use std::sync::Arc;

use nama_proto::nama::v1::{PageRequest, Permission, RequestState};

use crate::{
    plugins::ExtensionPoints,
    requests::testing::{
        ADA, BOB, ada, ada_and_bob, ask_for, breaking_bad, dark, follow_as, requests_on, user,
    },
    users::Permissions,
};

#[tokio::test]
async fn the_pending_count_is_as_many_as_the_pending_list_shows() {
    let (_data, db) = ada_and_bob().await;
    let requests = requests_on(Arc::clone(&db), ExtensionPoints::default());
    ask_for(&db, ADA, 949, RequestState::Pending).await;
    ask_for(&db, ADA, 603, RequestState::Approved).await;
    follow_as(&db, ADA, dark(), RequestState::Pending).await;
    follow_as(&db, ADA, breaking_bad(), RequestState::Approved).await;
    let nothing = Permissions::from_iter([]);
    let viewers = [
        user(BOB, Permissions::from_iter([Permission::ViewRequests])),
        ada(nothing),
        user(BOB, nothing),
    ];

    let mut counted_and_listed = Vec::new();
    for viewer in &viewers {
        let counted = requests.count_pending(viewer).await.unwrap();
        let page = requests
            .list(viewer, Some(RequestState::Pending), &sized(100))
            .await
            .unwrap();
        let listed = page.requests.len() + page.followed_shows.len();
        counted_and_listed.push((counted, u32::try_from(listed).unwrap()));
    }

    // Ada's Pending Movie, Followed Show and its two Season Requests.
    assert_eq!(counted_and_listed, [(4, 4), (4, 4), (0, 0)]);
}

fn sized(size: i32) -> PageRequest {
    PageRequest {
        size,
        ..Default::default()
    }
}

#[tokio::test]
async fn no_size_lists_50_and_500_lists_100_and_only_the_first_page_counts_them_all() {
    let (_data, db) = ada_and_bob().await;
    let requests = requests_on(Arc::clone(&db), ExtensionPoints::default());
    for tmdb_id in 1..=101 {
        ask_for(&db, ADA, tmdb_id, RequestState::Pending).await;
    }
    let viewer = ada(Permissions::from_iter([]));

    let unsized_page = requests.list(&viewer, None, &sized(0)).await.unwrap();
    let first = requests.list(&viewer, None, &sized(500)).await.unwrap();
    let after_first = PageRequest {
        token: first.page.next_token.clone(),
        ..sized(500)
    };
    let second = requests.list(&viewer, None, &after_first).await.unwrap();

    assert_eq!(unsized_page.requests.len(), 50);
    assert_eq!(first.requests.len(), 100);
    assert_eq!(first.page.total_size, Some(101));
    assert_eq!(second.requests.len(), 1);
    assert_eq!(
        second.page.total_size, None,
        "counted on the first page only"
    );
    assert_eq!(second.page.next_token, "", "the last page");
}
