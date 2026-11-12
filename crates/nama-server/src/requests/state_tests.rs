use std::collections::HashSet;

use nama_proto::nama::v1::RequestState;

use super::{is_open, settle};
use crate::{
    db::Database,
    requests::testing::{
        ADA, NOTHING, ada_and_bob, answered, ask_for, fulfilled, handed_off, happened,
        insert_in_each_state, settled,
    },
};

#[tokio::test]
async fn sql_and_rust_agree_on_which_requests_are_open() {
    let data = tempfile::tempdir().unwrap();
    let db = Database::open(data.path()).await.unwrap();
    let inserted = db
        .write(|tx| Box::pin(insert_in_each_state(tx)))
        .await
        .unwrap();
    macro_rules! read {
        ($open:literal) => {
            sqlx::query_scalar!(r#"SELECT id AS "id!" FROM requests WHERE "# + $open)
        };
    }
    let open: HashSet<i64> = open_request_sql!(read)
        .fetch_all(db.reader())
        .await
        .unwrap()
        .into_iter()
        .collect();

    for (id, state, reference) in inserted {
        let expected = is_open(state);
        assert_eq!(open.contains(&id), expected, "{state:?} with {reference:?}");
    }
}

#[tokio::test]
async fn only_a_declined_followed_show_is_closed() {
    let data = tempfile::tempdir().unwrap();
    let db = Database::open(data.path()).await.unwrap();
    db.write(|tx| {
        Box::pin(
            sqlx::query(
                "INSERT INTO followed_shows (id, tmdb_id, title, state, created_at)
                 VALUES (1, 1, 'Show', 'pending', 0), (2, 2, 'Show', 'approved', 0),
                        (3, 3, 'Show', 'declined', 0)",
            )
            .execute(&mut **tx),
        )
    })
    .await
    .unwrap();
    macro_rules! read {
        ($open:literal) => {
            sqlx::query_scalar!(r#"SELECT id AS "id!" FROM followed_shows WHERE "# + $open)
        };
    }
    let open: HashSet<i64> = open_followed_show_sql!(read)
        .fetch_all(db.reader())
        .await
        .unwrap()
        .into_iter()
        .collect();

    assert_eq!(open, HashSet::from([1, 2]));
}

#[tokio::test]
async fn settling_an_approved_request_moves_it_to_processing_or_failed_once() {
    let (_data, db) = ada_and_bob().await;
    let taken = ask_for(&db, ADA, 949, RequestState::Approved)
        .await
        .value
        .id;
    let refused = ask_for(&db, ADA, 603, RequestState::Approved)
        .await
        .value
        .id;

    let processing = settled(&db, taken, Ok(fulfilled())).await;
    let failed = settled(&db, refused, Err("no space".to_owned())).await;
    let again = settled(&db, taken, Ok(fulfilled())).await;

    assert_eq!(happened(&processing), [("moved", RequestState::Processing)]);
    assert_eq!(happened(&failed), [("moved", RequestState::Failed)]);
    assert_eq!(handed_off(&processing), NOTHING);
    assert_eq!(handed_off(&failed), NOTHING);
    assert!(again.value.is_none());
    assert_eq!(again.events.len(), 0);
}

#[tokio::test]
async fn settling_a_request_names_the_plugin_whose_fulfiller_failed_it() {
    let (_data, db) = ada_and_bob().await;
    let id = ask_for(&db, ADA, 949, RequestState::Approved)
        .await
        .value
        .id;
    let failed_by_radarr = answered("radarr", Err("no space".to_owned()));

    let failed = db
        .write(move |tx| Box::pin(settle(tx, id, failed_by_radarr, None)))
        .await
        .unwrap()
        .value
        .unwrap();

    assert_eq!(failed.fulfiller_plugin_id.as_deref(), Some("radarr"));
}
