//! The Shows Fulfiller asked to follow a Show as its Followed Show becomes Approved, whatever was
//! handed off before (ADR 0108).

use crate::support::server;

use nama_proto::nama::v1::{
    ApproveFollowedShowRequest, DeclineFollowedShowRequest, FollowedShow, Permission,
};
use server::{
    SignedIn, TestServer,
    requests::Requester,
    tmdb::{DARK, DARK_TVDB},
};

/// May Request and see Requests, but not approve their own.
const NEW_USER: [Permission; 2] = [Permission::Request, Permission::ViewRequests];
/// May Request TV and have it Approved at once.
const AUTO_APPROVED: [Permission; 2] = [Permission::RequestTv, Permission::AutoApproveTv];

/// A Server with TMDB and a fake Shows Fulfiller on, and Ada, who has `permissions`.
async fn with_ada(permissions: &[Permission]) -> (TestServer, SignedIn) {
    let server = TestServer::builder()
        .shows_fulfiller(&[])
        .tmdb()
        .start()
        .await;
    let ada = server.user("Ada", permissions).await;
    (server, ada)
}

/// Ada follows Dark by putting it on her Watchlist.
async fn ada_follows_dark(server: &TestServer, ada: &SignedIn) -> FollowedShow {
    server.requests(ada).follow(DARK).await
}

#[tokio::test]
async fn a_show_watchlisted_with_no_season_asked_for_is_followed_once_approved() {
    let (server, ada) = with_ada(&AUTO_APPROVED).await;

    ada_follows_dark(&server, &ada).await;

    let fulfiller = server.fulfiller();
    assert_eq!(fulfiller.handed_seasons(), []);
    assert_eq!(fulfiller.followed(), [DARK]);
}

#[tokio::test]
async fn a_show_whose_seasons_were_handed_off_before_it_was_watchlisted_is_followed_once_approved()
{
    let (server, ada) = with_ada(&AUTO_APPROVED).await;
    server.requests(&ada).ask_seasons(DARK, &[1, 2]).await;

    ada_follows_dark(&server, &ada).await;

    let fulfiller = server.fulfiller();
    let without_following = [
        (DARK, Some(DARK_TVDB), 1, false),
        (DARK, Some(DARK_TVDB), 2, false),
    ];
    assert_eq!(fulfiller.handed_seasons(), without_following);
    assert_eq!(fulfiller.followed(), [DARK]);
}

#[tokio::test]
async fn a_pending_followed_show_approved_after_its_season_requests_were_is_followed_then() {
    let (server, ada) = with_ada(&NEW_USER).await;
    let followed = ada_follows_dark(&server, &ada).await;
    let asked = server.requests(&ada).ask_seasons(DARK, &[1]).await;
    server.approve(asked[0].id).await;
    let fulfiller = server.fulfiller();
    let while_pending = fulfiller.followed();

    server
        .admin_requests()
        .approve_followed_show(ApproveFollowedShowRequest {
            id: followed.id,
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(while_pending, [0; 0]);
    assert_eq!(
        fulfiller.handed_seasons(),
        [(DARK, Some(DARK_TVDB), 1, false)]
    );
    assert_eq!(fulfiller.followed(), [DARK]);
}

#[tokio::test]
async fn a_declined_followed_show_is_never_followed() {
    let (server, ada) = with_ada(&NEW_USER).await;
    let followed = ada_follows_dark(&server, &ada).await;

    server
        .admin_requests()
        .decline_followed_show(DeclineFollowedShowRequest {
            id: followed.id,
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(server.fulfiller().followed(), [0; 0]);
}
