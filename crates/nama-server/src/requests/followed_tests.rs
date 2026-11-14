use nama_plugin_api::metadata::{ExternalId, ExternalIds};
use nama_proto::nama::v1::RequestState;

use super::{
    approved_followed_show, cancel_followed, decide_followed, open_followed_show, withdraw_followed,
};
use crate::requests::Error;
use crate::requests::apply::Applied;
use crate::requests::asking::WantedSeasons;
use crate::requests::listing::load;
use crate::requests::testing::{
    ADA, BOB, DARK, NOTHING, ada_and_bob, asked_season, breaking_bad, dark, follow_as, handed_off,
    happened, match_episodes, said, scan, season_ids,
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

#[tokio::test]
async fn a_show_known_only_by_a_made_up_id_is_followed_under_it_and_only_once() {
    let (_data, db) = ada_and_bob().await;
    let mal: ExternalId = "mal-1".parse().unwrap();
    let show = || WantedSeasons {
        show_id: mal.clone(),
        ..dark()
    };

    let followed = follow_as(&db, ADA, show(), RequestState::Approved).await;
    let joined = follow_as(&db, BOB, show(), RequestState::Approved).await;
    let second = db
        .write(|tx| {
            Box::pin(
                sqlx::query(
                    "INSERT INTO followed_shows (show_id, title, state, created_at)
                     VALUES ('mal-1', 'Dark', 'pending', 0)",
                )
                .execute(&mut **tx),
            )
        })
        .await;
    let open = db
        .write(move |tx| Box::pin(async move { open_followed_show(tx, &mal).await }))
        .await
        .unwrap();

    let followed = followed.value.0;
    let show_id = followed.show_id.as_option().unwrap();
    assert_eq!(
        (show_id.namespace.as_str(), show_id.value.as_str()),
        ("mal", "1")
    );
    assert_eq!(open, Some(followed.id));
    assert_eq!(joined.value.0.id, followed.id);
    let second = second.unwrap_err();
    assert!(
        second.to_string().contains("UNIQUE constraint failed"),
        "{second}"
    );
}

#[tokio::test]
async fn a_scan_opens_a_new_season_of_a_show_followed_under_an_id_other_than_tmdb() {
    let (_data, db) = ada_and_bob().await;
    let mal = ExternalId::new("mal", "1").unwrap();
    let show = || WantedSeasons {
        show_id: mal.clone(),
        ..dark()
    };
    follow_as(&db, BOB, show(), RequestState::Approved).await;
    follow_as(&db, ADA, show(), RequestState::Approved).await;
    match_episodes(&db, mal.clone(), 3, &[1, 2]).await;

    let scanned = scan(&db, mal.clone(), &[3]).await;

    assert_eq!(said(&scanned), [("created", RequestState::Available)]);
    let newest_first = db.write(|tx| Box::pin(load(tx, None, None))).await.unwrap();
    let season = &newest_first[0];
    let asked = asked_season(season);
    let show_id = asked.show_id.as_option().unwrap();
    assert_eq!(
        (
            show_id.namespace.as_str(),
            show_id.value.as_str(),
            asked.season_number
        ),
        ("mal", "1", 3)
    );
    let requesters: Vec<_> = season.requesters.iter().map(|one| one.user_id).collect();
    assert_eq!(requesters, [BOB, ADA]);
}

#[tokio::test]
async fn following_a_show_asked_by_tmdb_stores_the_tvdb_id_its_source_gave_and_finds_it_by_either()
{
    let (_data, db) = ada_and_bob().await;
    let tvdb = ExternalId::new("tvdb", "334824").unwrap();
    let both = ExternalIds::from_iter([ExternalId::tmdb(DARK), tvdb.clone()]);
    let show = WantedSeasons {
        show_ids: both.clone(),
        ..dark()
    };
    let id = follow_as(&db, ADA, show, RequestState::Approved)
        .await
        .value
        .0
        .id;

    let mut conn = db.reader().acquire().await.unwrap();
    let by_tvdb = ExternalIds::from_iter([tvdb.clone()]);
    let found = approved_followed_show(&mut conn, &by_tvdb).await.unwrap();
    let open = open_followed_show(&mut conn, &tvdb).await.unwrap();

    let (kept_under, followed, ids) = found.unwrap();
    assert_eq!(
        (kept_under, followed.id, ids),
        (ExternalId::tmdb(DARK), id, both)
    );
    assert_eq!(open, Some(id));
}

#[tokio::test]
async fn a_followed_show_opened_before_its_ids_were_stored_keeps_the_id_it_was_kept_under() {
    let data_dir = tempfile::tempdir().unwrap();
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(data_dir.path().join(crate::db::FILE_NAME))
        .create_if_missing(true);
    // A raw connection, not `Database::write`: `Database::open` would run every migration.
    let mut before = sqlx::ConnectOptions::connect(&options).await.unwrap();
    sqlx::migrate!().run_to(63, &mut before).await.unwrap();
    sqlx::query(
        "INSERT INTO followed_shows (id, show_id, title, state, created_at)
         VALUES (1, 'tmdb-70523', 'Dark', 'approved', 0), (2, 'tvdb-5', 'Five', 'declined', 0)",
    )
    .execute(&mut before)
    .await
    .unwrap();

    sqlx::migrate!().run_to(64, &mut before).await.unwrap();
    let ids: Vec<(i64, String, String)> =
        sqlx::query_as("SELECT * FROM followed_show_ids ORDER BY followed_show_id")
            .fetch_all(&mut before)
            .await
            .unwrap();

    assert_eq!(
        ids,
        [
            (1, "tmdb".to_owned(), "70523".to_owned()),
            (2, "tvdb".to_owned(), "5".to_owned())
        ]
    );
}
