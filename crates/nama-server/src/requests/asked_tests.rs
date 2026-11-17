use std::sync::Arc;

use nama_proto::nama::v1::{ListRequestsResponse, PageRequest, Permission, RequestState};

use super::Asked;
use crate::requests::apply::Applied;
use crate::requests::testing::{
    ADA, BOB, NOTHING, ada, ada_and_bob, ask_for, dark, follow_as, handed_off, requests_on, said,
    user,
};
use crate::requests::{Error, Requests};
use crate::{
    db::Database,
    plugins::ExtensionPoints,
    users::{Permissions, User},
};

const KINDS: [Asked; 2] = [Asked::Request, Asked::FollowedShow];

/// `user` asks for the Movie 949 or follows Dark, as `kind` says, joining it if it is open.
/// Returns what that applied, with the id of the one asked for as its value.
async fn ask_kind(db: &Database, kind: Asked, user: i64) -> Applied<i64> {
    match kind {
        Asked::Request => {
            let Applied {
                value,
                events,
                hand_off,
                unfollow,
            } = ask_for(db, user, 949, RequestState::Pending).await;
            Applied {
                value: value.id,
                events,
                hand_off,
                unfollow,
            }
        }
        Asked::FollowedShow => {
            let Applied {
                value: (followed, _),
                events,
                hand_off,
                unfollow,
            } = follow_as(db, user, dark(), RequestState::Pending).await;
            Applied {
                value: followed.id,
                events,
                hand_off,
                unfollow,
            }
        }
    }
}

/// The ids of the Requesters `page` lists for the `kind` with the ID `id`, if it lists it.
fn listed_requesters(page: &ListRequestsResponse, kind: Asked, id: i64) -> Option<Vec<i64>> {
    let requesters = match kind {
        Asked::Request => page
            .requests
            .iter()
            .find(|request| request.id == id)
            .map(|request| &request.requesters),
        Asked::FollowedShow => page
            .followed_shows
            .iter()
            .find(|followed| followed.id == id)
            .map(|followed| &followed.requesters),
    }?;
    Some(
        requesters
            .iter()
            .map(|requester| requester.user_id)
            .collect(),
    )
}

#[tokio::test]
async fn requesters_are_listed_in_the_order_they_asked() {
    let (_data, db) = ada_and_bob().await;
    let requests = requests_on(Arc::clone(&db), ExtensionPoints::default());

    for kind in KINDS {
        let id = ask_kind(&db, kind, BOB).await.value;
        ask_kind(&db, kind, ADA).await;
        let page = requests
            .list(&ada(Permissions::ADMIN), None, &PageRequest::default())
            .await
            .unwrap();

        assert_eq!(
            listed_requesters(&page, kind, id),
            Some(vec![BOB, ADA]),
            "{kind:?}"
        );
    }
}

#[tokio::test]
async fn asking_for_an_open_one_joins_it_and_says_so() {
    let (_data, db) = ada_and_bob().await;

    for kind in KINDS {
        let opened = ask_kind(&db, kind, ADA).await.value;
        let joined = ask_kind(&db, kind, BOB).await;
        let says = match kind {
            Asked::Request => "joined",
            Asked::FollowedShow => "joined the Show",
        };

        assert_eq!(joined.value, opened, "{kind:?}");
        assert_eq!(
            said(&joined.events).first(),
            Some(&(says, RequestState::Pending)),
            "{kind:?}"
        );
    }
}

/// `requester` withdraws from the `kind` with the ID `id`.
async fn withdraw_kind(
    requests: &Requests,
    kind: Asked,
    requester: &User,
    id: i64,
) -> Result<(), Error> {
    match kind {
        Asked::Request => requests.withdraw(requester, id).await,
        Asked::FollowedShow => requests.withdraw_followed(requester, id).await,
    }
}

#[tokio::test]
async fn joining_one_twice_changes_nothing() {
    let (_data, db) = ada_and_bob().await;
    let requests = requests_on(Arc::clone(&db), ExtensionPoints::default());

    for kind in KINDS {
        let opened = ask_kind(&db, kind, ADA).await.value;
        let again = ask_kind(&db, kind, ADA).await;
        let page = requests
            .list(&ada(Permissions::ADMIN), None, &PageRequest::default())
            .await
            .unwrap();

        assert_eq!(again.value, opened, "{kind:?}");
        assert_eq!(said(&again.events), [], "{kind:?}");
        assert_eq!(handed_off(&again), NOTHING, "{kind:?}");
        assert_eq!(
            listed_requesters(&page, kind, again.value),
            Some(vec![ADA]),
            "{kind:?}"
        );
    }
}

#[tokio::test]
async fn a_requester_leaving_takes_only_them_off() {
    let (_data, db) = ada_and_bob().await;
    let requests = requests_on(Arc::clone(&db), ExtensionPoints::default());
    let bob = user(BOB, Permissions::from_iter([Permission::ViewRequests]));

    for kind in KINDS {
        let id = ask_kind(&db, kind, ADA).await.value;
        ask_kind(&db, kind, BOB).await;
        withdraw_kind(&requests, kind, &bob, id).await.unwrap();
        let again = withdraw_kind(&requests, kind, &bob, id).await;
        let page = requests
            .list(&ada(Permissions::ADMIN), None, &PageRequest::default())
            .await
            .unwrap();

        assert_eq!(
            listed_requesters(&page, kind, id),
            Some(vec![ADA]),
            "{kind:?}"
        );
        assert!(
            matches!(again, Err(Error::NotARequester(k, i)) if k == kind && i == id),
            "{again:?}"
        );
    }
}

#[tokio::test]
async fn the_last_requester_leaving_removes_it() {
    let (_data, db) = ada_and_bob().await;
    let requests = requests_on(Arc::clone(&db), ExtensionPoints::default());

    for kind in KINDS {
        let id = ask_kind(&db, kind, ADA).await.value;
        withdraw_kind(&requests, kind, &ada(Permissions::NEW_USER), id)
            .await
            .unwrap();
        let page = requests
            .list(&ada(Permissions::ADMIN), None, &PageRequest::default())
            .await
            .unwrap();

        assert_eq!(listed_requesters(&page, kind, id), None, "{kind:?}");
    }
}

#[tokio::test]
async fn a_non_requester_withdrawing_is_not_found_unless_they_may_see_it_even_once_approved() {
    let (_data, db) = ada_and_bob().await;
    let requests = requests_on(Arc::clone(&db), ExtensionPoints::default());
    let viewer = user(BOB, Permissions::from_iter([Permission::ViewRequests]));
    let non_viewer = user(BOB, Permissions::from_iter([]));

    for kind in KINDS {
        let id = ask_kind(&db, kind, ADA).await.value;
        approve_kind(&requests, kind, id).await.unwrap();
        let unseen = withdraw_kind(&requests, kind, &non_viewer, id).await;
        let seen = withdraw_kind(&requests, kind, &viewer, id).await;

        assert!(
            matches!(unseen, Err(Error::NoSuch(k, i)) if k == kind && i == id),
            "{unseen:?}"
        );
        assert!(
            matches!(seen, Err(Error::NotARequester(k, i)) if k == kind && i == id),
            "{seen:?}"
        );
    }
}

/// A Request manager approves the `kind` with the ID `id`.
async fn approve_kind(requests: &Requests, kind: Asked, id: i64) -> Result<(), Error> {
    let manager = ada(Permissions::ADMIN);
    match kind {
        Asked::Request => requests.approve(&manager, id, None).await.map(drop),
        Asked::FollowedShow => requests
            .approve_followed(&manager, id, None)
            .await
            .map(drop),
    }
}

/// Declines the `kind` with the ID `id`, giving `reason`. Returns the reason it keeps.
async fn decline_kind(
    requests: &Requests,
    kind: Asked,
    id: i64,
    reason: &str,
) -> Result<Option<String>, Error> {
    match kind {
        Asked::Request => requests
            .decline(id, reason)
            .await
            .map(|request| request.decline_reason),
        Asked::FollowedShow => requests
            .decline_followed(id, reason)
            .await
            .map(|followed| followed.decline_reason),
    }
}

#[tokio::test]
async fn only_a_pending_one_is_decided_and_declining_keeps_its_reason() {
    let (_data, db) = ada_and_bob().await;
    let requests = requests_on(Arc::clone(&db), ExtensionPoints::default());

    for kind in KINDS {
        let declined = ask_kind(&db, kind, ADA).await.value;
        let reason = decline_kind(&requests, kind, declined, " No. ")
            .await
            .unwrap();
        let approved = ask_kind(&db, kind, ADA).await.value;
        approve_kind(&requests, kind, approved).await.unwrap();
        let approved_again = approve_kind(&requests, kind, declined).await;
        let declined_again = decline_kind(&requests, kind, approved, "").await;

        assert_eq!(reason.as_deref(), Some("No."), "{kind:?}");
        assert!(
            matches!(approved_again, Err(Error::NotPending(k, id)) if k == kind && id == declined),
            "{approved_again:?}"
        );
        assert!(
            matches!(declined_again, Err(Error::NotPending(k, id)) if k == kind && id == approved),
            "{declined_again:?}"
        );
    }
}

/// Whether `viewer` sees the `kind` with the ID `id` among the Requests listed.
async fn sees(requests: &Requests, viewer: &User, kind: Asked, id: i64) -> bool {
    let page = requests
        .list(viewer, None, &PageRequest::default())
        .await
        .unwrap();
    listed_requesters(&page, kind, id).is_some()
}

#[tokio::test]
async fn only_its_requesters_and_those_who_may_view_requests_see_one() {
    let (_data, db) = ada_and_bob().await;
    let requests = requests_on(Arc::clone(&db), ExtensionPoints::default());
    let nothing = Permissions::from_iter([]);
    let view_requests = Permissions::from_iter([Permission::ViewRequests]);

    for kind in KINDS {
        let id = ask_kind(&db, kind, ADA).await.value;
        let seen = [
            sees(&requests, &ada(nothing), kind, id).await,
            sees(&requests, &user(BOB, nothing), kind, id).await,
            sees(&requests, &user(BOB, view_requests), kind, id).await,
        ];

        assert_eq!(seen, [true, false, true], "{kind:?}");
    }
}
