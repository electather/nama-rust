use nama_proto::nama::v1::RequestState;

use super::{cancel_followed, decide_followed, withdraw_followed};
use crate::requests::Error;
use crate::requests::apply::Applied;
use crate::requests::testing::{
    ADA, BOB, NOTHING, ada_and_bob, breaking_bad, dark, follow_as, handed_off, happened, season_ids,
};

/// The id of the Season Request whose Show `applied` stops following, if any.
fn unfollowed<T>(applied: &Applied<T>) -> Option<i64> {
    applied.unfollow.as_ref().map(|season| season.id)
}

#[tokio::test]
async fn following_a_show_asks_for_each_season_and_hands_them_off_only_if_approved() {
    let (_data, db) = ada_and_bob().await;

    let pending = follow_as(&db, ADA, dark(), RequestState::Pending).await;
    let joined = follow_as(&db, BOB, dark(), RequestState::Pending).await;
    let approved = follow_as(&db, ADA, breaking_bad(), RequestState::Approved).await;

    assert_eq!(
        happened(&pending),
        [
            ("followed", RequestState::Pending),
            ("created", RequestState::Pending),
            ("created", RequestState::Pending)
        ]
    );
    assert_eq!(handed_off(&pending), NOTHING);
    assert_eq!(
        happened(&joined),
        [
            ("joined the Show", RequestState::Pending),
            ("joined", RequestState::Pending),
            ("joined", RequestState::Pending)
        ]
    );
    assert_eq!(handed_off(&joined), NOTHING);
    assert_eq!(
        happened(&approved),
        [
            ("followed", RequestState::Approved),
            ("created", RequestState::Approved),
            ("created", RequestState::Approved)
        ]
    );
    assert_eq!(handed_off(&approved), season_ids(&approved));
    for followed in [pending, joined, approved] {
        assert_eq!(unfollowed(&followed), None);
    }
}

#[tokio::test]
async fn approving_a_pending_followed_show_approves_and_hands_off_its_pending_seasons() {
    let (_data, db) = ada_and_bob().await;
    let followed = follow_as(&db, ADA, dark(), RequestState::Pending).await;
    let id = followed.value.0.id;

    let approved = db
        .write(move |tx| Box::pin(decide_followed(tx, id, RequestState::Approved, None, None)))
        .await
        .unwrap();
    let again = db
        .write(move |tx| Box::pin(decide_followed(tx, id, RequestState::Approved, None, None)))
        .await;

    assert_eq!(
        happened(&approved),
        [
            ("moved the Show", RequestState::Approved),
            ("moved", RequestState::Approved),
            ("moved", RequestState::Approved)
        ]
    );
    assert_eq!(handed_off(&approved), season_ids(&followed));
    assert_eq!(unfollowed(&approved), None);
    assert!(matches!(again, Err(Error::NotPending(..))), "{again:?}");
}

#[tokio::test]
async fn declining_a_pending_followed_show_declines_its_pending_seasons_and_hands_nothing_off() {
    let (_data, db) = ada_and_bob().await;
    let id = follow_as(&db, ADA, dark(), RequestState::Pending)
        .await
        .value
        .0
        .id;

    let declined = db
        .write(move |tx| {
            let reason = Some("No.".to_owned());
            Box::pin(decide_followed(
                tx,
                id,
                RequestState::Declined,
                reason,
                None,
            ))
        })
        .await
        .unwrap();

    assert_eq!(
        happened(&declined),
        [
            ("moved the Show", RequestState::Declined),
            ("moved", RequestState::Declined),
            ("moved", RequestState::Declined)
        ]
    );
    assert_eq!(handed_off(&declined), NOTHING);
    assert_eq!(unfollowed(&declined), None);
    assert_eq!(declined.value.decline_reason.as_deref(), Some("No."));
}

#[tokio::test]
async fn cancelling_a_followed_show_unfollows_it_only_if_it_was_approved() {
    let (_data, db) = ada_and_bob().await;
    let approved = follow_as(&db, ADA, dark(), RequestState::Approved).await;
    let pending = follow_as(&db, ADA, breaking_bad(), RequestState::Pending).await;
    let (approved_id, pending_id) = (approved.value.0.id, pending.value.0.id);

    let cancelled_approved = db
        .write(move |tx| Box::pin(cancel_followed(tx, approved_id)))
        .await
        .unwrap();
    let cancelled_pending = db
        .write(move |tx| Box::pin(cancel_followed(tx, pending_id)))
        .await
        .unwrap();
    let again = db
        .write(move |tx| Box::pin(cancel_followed(tx, approved_id)))
        .await;

    assert_eq!(
        happened(&cancelled_approved),
        [("cancelled the Show", RequestState::Approved)]
    );
    assert_eq!(
        unfollowed(&cancelled_approved),
        season_ids(&approved).first().copied()
    );
    assert_eq!(
        happened(&cancelled_pending),
        [("cancelled the Show", RequestState::Pending)]
    );
    assert_eq!(unfollowed(&cancelled_pending), None);
    assert_eq!(handed_off(&cancelled_approved), NOTHING);
    assert_eq!(handed_off(&cancelled_pending), NOTHING);
    assert!(matches!(again, Err(Error::NoSuch(..))), "{again:?}");
}

#[tokio::test]
async fn withdrawing_from_a_followed_show_that_is_not_pending_is_allowed() {
    let (_data, db) = ada_and_bob().await;
    let id = follow_as(&db, ADA, dark(), RequestState::Approved)
        .await
        .value
        .0
        .id;
    follow_as(&db, BOB, dark(), RequestState::Pending).await;

    let bob_left = db
        .write(move |tx| Box::pin(withdraw_followed(tx, BOB, None, id)))
        .await
        .unwrap();

    assert_eq!(
        happened(&bob_left),
        [("left the Show", RequestState::Approved)]
    );
    assert_eq!(unfollowed(&bob_left), None);
}

#[tokio::test]
async fn the_last_requester_withdrawing_from_an_approved_followed_show_unfollows_it() {
    let (_data, db) = ada_and_bob().await;
    let followed = follow_as(&db, ADA, dark(), RequestState::Approved).await;
    let id = followed.value.0.id;

    let ada_left = db
        .write(move |tx| Box::pin(withdraw_followed(tx, ADA, None, id)))
        .await
        .unwrap();

    assert_eq!(
        happened(&ada_left),
        [("withdrew the Show", RequestState::Approved)]
    );
    assert_eq!(
        unfollowed(&ada_left),
        season_ids(&followed).first().copied()
    );
}
