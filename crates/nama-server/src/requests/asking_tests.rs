use std::collections::HashSet;

use nama_plugin_api::metadata::ExternalIds;
use nama_proto::nama::v1::RequestState;

use super::{Wanted, ask_movie, ask_seasons};
use crate::db::Database;
use crate::requests::deciding::{cancel, decline};
use crate::requests::testing::{
    ADA, BOB, NOTHING, ada_and_bob, ask_each, ask_for, dark, fulfilled, handed_off, happened,
    match_in_a_library, movie, settled,
};
use crate::requests::{Error, qualities};

#[tokio::test]
async fn a_movie_and_a_show_with_the_same_tmdb_id_have_requests_of_their_own() {
    let data = tempfile::tempdir().unwrap();
    let db = Database::open(data.path()).await.unwrap();
    let wanted = [None, Some(0), Some(1), None].map(|season| Wanted {
        tmdb_id: 1,
        season,
        title: "Shared".to_owned(),
        release_date: None,
        show_ids: ExternalIds::default(),
        followed_show: None,
        quality: qualities::Ask::default(),
    });

    let ids = db
        .write(move |tx| Box::pin(ask_each(tx, wanted.into(), RequestState::Pending)))
        .await
        .unwrap();

    assert_eq!(ids.iter().collect::<HashSet<_>>().len(), 3, "{ids:?}");
    assert_eq!(
        ids.first(),
        ids.last(),
        "asking again joins the Movie's Request"
    );
}

#[tokio::test]
async fn asking_for_a_movie_opens_its_request_and_hands_it_off_only_if_approved() {
    let (_data, db) = ada_and_bob().await;

    let pending = ask_for(&db, ADA, 949, RequestState::Pending).await;
    let approved = ask_for(&db, ADA, 603, RequestState::Approved).await;

    assert_eq!(happened(&pending), [("created", RequestState::Pending)]);
    assert_eq!(handed_off(&pending), NOTHING);
    assert_eq!(happened(&approved), [("created", RequestState::Approved)]);
    assert_eq!(handed_off(&approved), [approved.value.id]);
}

#[tokio::test]
async fn joining_a_pending_request_approves_and_hands_it_off_only_for_an_auto_approver() {
    let (_data, db) = ada_and_bob().await;
    ask_for(&db, ADA, 949, RequestState::Pending).await;

    let joined = ask_for(&db, BOB, 949, RequestState::Pending).await;
    let asked_again = ask_for(&db, BOB, 949, RequestState::Approved).await;

    assert_eq!(happened(&joined), [("joined", RequestState::Pending)]);
    assert_eq!(handed_off(&joined), NOTHING);
    assert_eq!(happened(&asked_again), [("moved", RequestState::Approved)]);
    assert_eq!(handed_off(&asked_again), [joined.value.id]);
}

#[tokio::test]
async fn joining_a_request_already_approved_hands_nothing_off() {
    let (_data, db) = ada_and_bob().await;
    ask_for(&db, ADA, 949, RequestState::Approved).await;
    let processing = ask_for(&db, ADA, 603, RequestState::Approved)
        .await
        .value
        .id;
    settled(&db, processing, Ok(fulfilled())).await;
    let failed = ask_for(&db, ADA, 348, RequestState::Approved)
        .await
        .value
        .id;
    settled(&db, failed, Err("no space".to_owned())).await;

    let mut joined = Vec::new();
    for tmdb_id in [949, 603, 348] {
        let applied = ask_for(&db, BOB, tmdb_id, RequestState::Approved).await;
        assert_eq!(handed_off(&applied), NOTHING);
        joined.extend(happened(&applied));
    }

    assert_eq!(
        joined,
        [
            ("joined", RequestState::Approved),
            ("joined", RequestState::Processing),
            ("joined", RequestState::Failed)
        ]
    );
}

#[tokio::test]
async fn asking_for_a_declined_movie_opens_a_new_request_but_one_in_a_library_is_refused() {
    let (_data, db) = ada_and_bob().await;
    let declined = ask_for(&db, ADA, 949, RequestState::Pending).await.value.id;
    db.write(move |tx| Box::pin(decline(tx, declined, None)))
        .await
        .unwrap();
    ask_for(&db, ADA, 603, RequestState::Pending).await;
    match_in_a_library(&db, &[603]).await;

    let asked_again = ask_for(&db, BOB, 949, RequestState::Pending).await;
    let in_a_library = db
        .write(|tx| Box::pin(ask_movie(tx, BOB, movie(603), RequestState::Pending)))
        .await;

    assert_eq!(happened(&asked_again), [("created", RequestState::Pending)]);
    assert_ne!(asked_again.value.id, declined);
    assert!(
        matches!(in_a_library, Err(Error::InLibrary(603))),
        "{in_a_library:?}"
    );
}

#[tokio::test]
async fn asking_for_seasons_opens_a_request_for_each_and_hands_off_the_approved_ones() {
    let (_data, db) = ada_and_bob().await;
    let pending = db
        .write(|tx| Box::pin(ask_seasons(tx, ADA, dark(), RequestState::Pending)))
        .await
        .unwrap();
    let first = pending.value[0].id;
    db.write(move |tx| Box::pin(cancel(tx, first)))
        .await
        .unwrap();

    let approved = db
        .write(|tx| Box::pin(ask_seasons(tx, BOB, dark(), RequestState::Approved)))
        .await
        .unwrap();

    assert_eq!(
        happened(&pending),
        [
            ("created", RequestState::Pending),
            ("created", RequestState::Pending)
        ]
    );
    assert_eq!(handed_off(&pending), NOTHING);
    assert_eq!(
        happened(&approved),
        [
            ("created", RequestState::Approved),
            ("joined", RequestState::Pending),
            ("moved", RequestState::Approved)
        ]
    );
    let ids: Vec<i64> = approved.value.iter().map(|request| request.id).collect();
    assert_eq!(handed_off(&approved), ids);
}
