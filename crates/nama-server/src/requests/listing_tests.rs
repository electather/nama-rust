use std::sync::Arc;

use nama_proto::nama::v1::{Permission, RequestState};

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
            .list(viewer, Some(RequestState::Pending), 100, "")
            .await
            .unwrap();
        let listed = page.requests.len() + page.followed_shows.len();
        counted_and_listed.push((counted, u32::try_from(listed).unwrap()));
    }

    // Ada's Pending Movie, Followed Show and its two Season Requests.
    assert_eq!(counted_and_listed, [(4, 4), (4, 4), (0, 0)]);
}
