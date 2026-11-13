use nama_plugin_api::metadata::ExternalId;
use nama_proto::nama::v1::RequestState;

use super::{RequestTitle, approve, cancel, cancel_deleted, decline, retry, withdraw};
use crate::requests::asking::{Wanted, ask, ask_seasons};
use crate::requests::testing::{
    ADA, NOTHING, ada_and_bob, ask_for, dark, fulfilled, handed_off, happened, match_dark_episodes,
    match_in_a_library, movie, scan_dark, settled,
};
use crate::requests::{Error, asked};

#[tokio::test]
async fn approving_a_pending_request_hands_it_off() {
    let (_data, db) = ada_and_bob().await;
    let id = ask_for(&db, ADA, 949, RequestState::Pending).await.value.id;

    let approved = db
        .write(move |tx| Box::pin(approve(tx, id, None)))
        .await
        .unwrap();
    let again = db.write(move |tx| Box::pin(approve(tx, id, None))).await;

    assert_eq!(happened(&approved), [("moved", RequestState::Approved)]);
    assert_eq!(handed_off(&approved), [id]);
    assert!(matches!(again, Err(Error::NotPending(..))), "{again:?}");
}

#[tokio::test]
async fn declining_a_pending_request_hands_nothing_off() {
    let (_data, db) = ada_and_bob().await;
    let id = ask_for(&db, ADA, 949, RequestState::Pending).await.value.id;

    let declined = db
        .write(move |tx| Box::pin(decline(tx, id, Some("No.".to_owned()))))
        .await
        .unwrap();
    let again = db.write(move |tx| Box::pin(decline(tx, id, None))).await;

    assert_eq!(happened(&declined), [("moved", RequestState::Declined)]);
    assert_eq!(handed_off(&declined), NOTHING);
    assert_eq!(declined.value.decline_reason.as_deref(), Some("No."));
    assert!(matches!(again, Err(Error::NotPending(..))), "{again:?}");
}

#[tokio::test]
async fn retrying_a_failed_request_approves_and_hands_it_off() {
    let (_data, db) = ada_and_bob().await;
    let id = ask_for(&db, ADA, 949, RequestState::Approved)
        .await
        .value
        .id;
    let pending = ask_for(&db, ADA, 603, RequestState::Pending).await.value.id;
    settled(&db, id, Err("no space".to_owned())).await;

    let retried = db
        .write(move |tx| Box::pin(retry(tx, id, None)))
        .await
        .unwrap();
    let not_failed = db.write(move |tx| Box::pin(retry(tx, pending, None))).await;

    assert_eq!(happened(&retried), [("moved", RequestState::Approved)]);
    assert_eq!(handed_off(&retried), [id]);
    assert_eq!(retried.value.failure_reason, None);
    assert!(
        matches!(not_failed, Err(Error::NotFailed(_))),
        "{not_failed:?}"
    );
}

#[tokio::test]
async fn cancelling_a_request_says_so_whatever_its_state() {
    let (_data, db) = ada_and_bob().await;
    let pending = ask_for(&db, ADA, 949, RequestState::Pending).await.value.id;
    let processing = ask_for(&db, ADA, 603, RequestState::Approved)
        .await
        .value
        .id;
    settled(&db, processing, Ok(fulfilled())).await;
    let declined = ask_for(&db, ADA, 348, RequestState::Pending).await.value.id;
    db.write(move |tx| Box::pin(decline(tx, declined, None)))
        .await
        .unwrap();
    let approved = ask_for(&db, ADA, 680, RequestState::Approved)
        .await
        .value
        .id;
    let failed = ask_for(&db, ADA, 155, RequestState::Approved)
        .await
        .value
        .id;
    settled(&db, failed, Err("no space".to_owned())).await;
    let available = ask_for(&db, ADA, 157, RequestState::Pending).await.value.id;
    match_in_a_library(&db, &[157]).await;
    let partly = db
        .write(|tx| Box::pin(ask_seasons(tx, ADA, dark(), RequestState::Approved)))
        .await
        .unwrap()
        .value[0]
        .id;
    settled(&db, partly, Ok(fulfilled())).await;
    match_dark_episodes(&db, 1, &[1]).await;
    scan_dark(&db, &[1]).await;

    let mut cancelled = Vec::new();
    for id in [
        pending, processing, declined, approved, failed, available, partly,
    ] {
        let applied = db.write(move |tx| Box::pin(cancel(tx, id))).await.unwrap();
        assert_eq!(handed_off(&applied), NOTHING);
        cancelled.extend(happened(&applied));
    }

    assert_eq!(
        cancelled,
        [
            ("cancelled", RequestState::Pending),
            ("cancelled", RequestState::Processing),
            ("cancelled", RequestState::Declined),
            ("cancelled", RequestState::Approved),
            ("cancelled", RequestState::Failed),
            ("cancelled", RequestState::Available),
            ("cancelled", RequestState::PartiallyAvailable)
        ]
    );
}

#[tokio::test]
async fn withdrawing_from_a_request_that_is_not_pending_is_refused() {
    let (_data, db) = ada_and_bob().await;
    let id = ask_for(&db, ADA, 603, RequestState::Approved)
        .await
        .value
        .id;

    let refused = db
        .write(move |tx| Box::pin(withdraw(tx, ADA, None, id)))
        .await;

    assert!(
        matches!(refused, Err(Error::NotPending(asked::Asked::Request, refused_id)) if refused_id == id),
        "{refused:?}"
    );
}

#[tokio::test]
async fn deleting_media_of_a_title_known_by_a_made_up_id_cancels_its_request() {
    let (_data, db) = ada_and_bob().await;
    let anidb = ExternalId::new("anidb", "1").unwrap();
    let wanted = Wanted {
        id: anidb.clone(),
        ..movie(1)
    };
    db.write(move |tx| Box::pin(ask(tx, ADA, wanted, RequestState::Pending)))
        .await
        .unwrap();
    let titles = vec![RequestTitle {
        id: anidb,
        season: None,
    }];

    let deleted = db
        .write(move |tx| Box::pin(cancel_deleted(tx, titles, "bob".to_owned())))
        .await
        .unwrap();

    assert_eq!(happened(&deleted), [("cancelled", RequestState::Pending)]);
}
