use std::{collections::HashSet, sync::Arc};

use nama_plugin_api::{
    fulfiller::Movies,
    metadata::{ExternalId, ExternalIds, IMDB},
};
use nama_plugin_tmdb::testing::FakeTmdb;
use nama_proto::nama::v1::RequestState;

use super::{Wanted, ask_movie, ask_seasons};
use crate::db::Database;
use crate::library::contents::{Metadata, fixture::tmdb_movie};
use crate::plugins::Registry;
use crate::requests::deciding::{cancel, decline};
use crate::requests::listing::load_one;
use crate::requests::testing::{
    ADA, Asked, BOB, DARK, NOTHING, ada, ada_and_bob, approved_on, ask_each, ask_for, asked_movie,
    asked_season, dark, enable_tmdb, fulfilled, handed_off, happened, match_in_a_library, movie,
    settled,
};
use crate::requests::{Error, qualities};
use crate::users::Permissions;

#[tokio::test]
async fn a_movie_and_a_show_with_the_same_tmdb_id_have_requests_of_their_own() {
    let data = tempfile::tempdir().unwrap();
    let db = Database::open(data.path()).await.unwrap();
    let wanted = [None, Some(0), Some(1), None].map(|season| Wanted {
        id: ExternalId::tmdb(1),
        season,
        title: "Shared".to_owned(),
        release_date: None,
        ids: ExternalIds::default(),
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
        matches!(&in_a_library, Err(Error::InLibrary(id)) if *id == ExternalId::tmdb(603)),
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

#[tokio::test]
async fn a_season_request_lists_every_id_its_show_was_asked_with_and_is_kept_under_tmdb() {
    let data = tempfile::tempdir().unwrap();
    let db = Database::open(data.path()).await.unwrap();
    let show_ids = ExternalIds::from_iter([
        ExternalId::tmdb(DARK),
        "imdb-tt5753856".parse().unwrap(),
        ExternalId::tvdb(334_824),
    ]);
    let wanted = Wanted {
        id: ExternalId::tmdb(DARK),
        season: Some(1),
        title: "Dark".to_owned(),
        release_date: None,
        ids: show_ids,
        followed_show: None,
        quality: qualities::Ask::default(),
    };

    let id = db
        .write(move |tx| Box::pin(ask_each(tx, vec![wanted], RequestState::Pending)))
        .await
        .unwrap()[0];

    let request = db
        .write(move |tx| Box::pin(load_one(tx, id)))
        .await
        .unwrap()
        .unwrap();
    let season = asked_season(&request);
    let text =
        |pair: &nama_proto::nama::v1::ExternalId| format!("{}-{}", pair.namespace, pair.value);
    assert_eq!(
        season.show_id.as_option().map(text).as_deref(),
        Some("tmdb-70523")
    );
    assert_eq!(
        season.show_ids.iter().map(text).collect::<Vec<_>>(),
        ["imdb-tt5753856", "tmdb-70523", "tvdb-334824"]
    );
}

#[tokio::test]
async fn the_database_refuses_a_second_open_request_for_a_title_and_season() {
    let data = tempfile::tempdir().unwrap();
    let db = Database::open(data.path()).await.unwrap();
    let insert = |state: &'static str| {
        db.write(move |tx| {
            Box::pin(
                sqlx::query(
                    "INSERT INTO requests (title_id, season, title, state, created_at)
                     VALUES ('tmdb-70523', 1, 'Dark', ?, 0)",
                )
                .bind(state)
                .execute(&mut **tx),
            )
        })
    };

    insert("declined").await.unwrap();
    insert("pending").await.unwrap();
    let second = insert("approved").await;

    assert!(second.is_err(), "{second:?}");
}

#[tokio::test]
async fn asking_for_a_movie_by_its_imdb_id_joins_the_request_kept_under_its_tmdb_id() {
    let tmdb = FakeTmdb::start().await;
    let plugins = Registry::new();
    enable_tmdb(&plugins, &tmdb);
    let (_data, requests) = approved_on::<Movies>(&plugins, &[], Arc::new(Asked::default())).await;
    let asker = ada(Permissions::ADMIN);
    let imdb = ExternalId::new(IMDB, "tt0078748").unwrap();

    let opened = requests
        .create(&asker, &ExternalId::tmdb(348), None)
        .await
        .unwrap();
    let joined = requests.create(&asker, &imdb, None).await.unwrap();

    let kept_under = &asked_movie(&opened).id;
    assert_eq!(
        (kept_under.namespace.as_str(), kept_under.value.as_str()),
        ("tmdb", "348")
    );
    assert_eq!(joined.id, opened.id);
}

#[test]
fn a_movie_the_source_gives_no_ids_for_is_kept_under_the_id_it_was_asked_by() {
    let anidb = ExternalId::new("anidb", "1").unwrap();
    let movie = Metadata {
        ids: ExternalIds::default(),
        ..tmdb_movie(1, "Made Up")
    };

    let wanted = Wanted::movie(&anidb, movie, qualities::Ask::default());

    assert_eq!(wanted.id, anidb);
}
