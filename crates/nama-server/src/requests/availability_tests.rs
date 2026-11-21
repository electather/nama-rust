use std::{collections::HashSet, sync::Arc};

use nama_plugin_api::{
    PluginId,
    metadata::{ExternalId, ExternalIds},
};
use nama_proto::nama::v1::{RequestState, event::Event as Happened};

use super::availability;
use crate::requests::asking::{Wanted, WantedSeasons, ask, ask_movie, ask_seasons};
use crate::requests::deciding::{cancel, decline};
use crate::requests::followed::decide_followed;
use crate::requests::listing::{load, load_one};
use crate::requests::qualities;
use crate::requests::testing::{
    ADA, BOB, DARK, ada_and_bob, ask_each, asked_season, dark, dark_season, episode, follow_as,
    fulfilled, match_dark_episodes, match_episodes, match_show_episodes, matched, movie, said,
    scan_dark, season_1, season_ids, settled, tmdb,
};
use crate::{
    db::Database,
    library::{
        Change, Contents, Decisions, Item, Libraries, LibraryKind, Metadata,
        fixture::{MatchedMovie, tmdb_movie},
    },
};

const TODAY: &str = "2026-11-08";

#[test]
fn a_season_still_airing_follows_how_many_of_its_aired_episodes_are_matched() {
    let airing = [
        episode(1, Some("2026-11-01")),
        episode(2, Some(TODAY)),
        episode(3, Some("2026-11-15")),
        episode(4, None),
    ];

    let all_aired = availability(&airing, &HashSet::from([1, 2]), TODAY);
    let some_aired = availability(&airing, &HashSet::from([1, 3, 4]), TODAY);
    let none_aired = availability(&airing, &HashSet::from([3, 4]), TODAY);

    assert_eq!(all_aired, Some(RequestState::Available));
    assert_eq!(some_aired, Some(RequestState::PartiallyAvailable));
    assert_eq!(none_aired, None);
}

#[test]
fn a_season_yet_to_air_is_not_in_a_library() {
    let announced = [episode(1, Some("2027-01-01")), episode(2, None)];

    assert_eq!(
        availability(&announced, &HashSet::from([1, 2]), TODAY),
        None
    );
    assert_eq!(availability(&[], &HashSet::new(), TODAY), None);
}

#[tokio::test]
async fn an_available_season_request_stays_available_when_a_later_episode_airs() {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let root = folder.path().canonicalize().unwrap();
    let library = Libraries::new(Arc::clone(&db))
        .create(
            "Shows",
            LibraryKind::Shows,
            vec![root.to_str().unwrap().to_owned()],
        )
        .await
        .unwrap()
        .id;
    let contents = Contents::new(Arc::clone(&db));
    let pilot = Item::Episode {
        show: "Dark".to_owned(),
        year: None,
        hint: Some(ExternalId::tmdb(DARK)),
        season: 1,
        episode: 1,
    };
    let store = Change::Store {
        edition: None,
        path: "/media/Dark/Season 01/Dark - S01E01.mkv".to_owned(),
        size: 1,
        modified_ns: 0,
        items: vec![pilot],
    };
    contents.apply(library, vec![store]).await.unwrap();
    let item = contents.files(library).await.unwrap()[0].items[0].id;
    let wanted = Wanted {
        id: ExternalId::tmdb(DARK),
        season: Some(1),
        title: "Dark".to_owned(),
        release_date: None,
        ids: ExternalIds::default(),
        followed_show: None,
        quality: qualities::Ask::default(),
    };
    let request = db
        .write(move |tx| Box::pin(ask_each(tx, vec![wanted], RequestState::Pending)))
        .await
        .unwrap()[0];
    let scan = |episode_2_airs| Decisions {
        items: vec![(item, matched(ExternalIds::default(), tmdb(DARK)))],
        seasons: vec![(
            PluginId::from_static("tmdb"),
            tmdb(DARK),
            season_1(episode_2_airs),
        )],
    };
    contents.record(scan("2099-12-01")).await.unwrap();

    let episode_2_aired = contents.record(scan("2017-12-08")).await.unwrap();

    let state = db
        .write(move |tx| Box::pin(load_one(tx, request)))
        .await
        .unwrap()
        .unwrap()
        .state;
    assert_eq!(state.as_known(), Some(RequestState::Available));
    assert!(episode_2_aired.is_empty(), "{episode_2_aired:?}");
}

/// Dark's Season Requests as a later read shows them, oldest first: each one's Season, state
/// and Requesters, first asker first.
async fn dark_seasons(db: &Database) -> Vec<(u32, RequestState, Vec<i64>)> {
    let newest_first = db.write(|tx| Box::pin(load(tx, None, None))).await.unwrap();
    newest_first
        .into_iter()
        .rev()
        .map(|request| {
            let requesters = request.requesters.iter().map(|one| one.user_id).collect();
            let state = request.state.as_known().unwrap_or_default();
            (asked_season(&request).season_number, state, requesters)
        })
        .collect()
}

/// The Season Requests for Seasons 1 and 2 of Dark that Ada following it in `state` opens, as
/// [`dark_seasons`] reads them.
fn opened_by_ada_following_dark(state: RequestState) -> [(u32, RequestState, Vec<i64>); 2] {
    [1, 2].map(|season| (season, state, vec![ADA]))
}

#[tokio::test]
async fn a_scan_opens_a_new_season_of_an_approved_followed_show_for_its_requesters_in_the_order_they_asked()
 {
    let (_data, db) = ada_and_bob().await;
    follow_as(&db, BOB, dark(), RequestState::Approved).await;
    follow_as(&db, ADA, dark(), RequestState::Pending).await;
    match_dark_episodes(&db, 3, &[1, 2]).await;

    let scanned = scan_dark(&db, &[3]).await;

    assert_eq!(said(&scanned), [("created", RequestState::Available)]);
    assert_eq!(
        dark_seasons(&db).await.last(),
        Some(&(3, RequestState::Available, vec![BOB, ADA]))
    );
}

#[tokio::test]
async fn a_season_a_scan_opens_is_partially_available_or_available_by_its_matched_aired_episodes() {
    let (_data, db) = ada_and_bob().await;
    follow_as(&db, ADA, dark(), RequestState::Approved).await;
    match_dark_episodes(&db, 3, &[1]).await;
    match_dark_episodes(&db, 4, &[1, 2]).await;

    let scanned = scan_dark(&db, &[3, 4]).await;

    assert_eq!(
        said(&scanned),
        [
            ("created", RequestState::PartiallyAvailable),
            ("created", RequestState::Available)
        ]
    );
}

#[tokio::test]
async fn a_scan_opens_no_season_without_matched_episodes() {
    let (_data, db) = ada_and_bob().await;
    follow_as(&db, ADA, dark(), RequestState::Approved).await;

    let scanned = scan_dark(&db, &[3]).await;

    assert_eq!(said(&scanned), []);
    assert_eq!(
        dark_seasons(&db).await,
        opened_by_ada_following_dark(RequestState::Approved)
    );
}

#[tokio::test]
async fn a_scan_opens_no_season_for_a_followed_show_not_yet_approved() {
    let (_data, db) = ada_and_bob().await;
    follow_as(&db, ADA, dark(), RequestState::Pending).await;
    match_dark_episodes(&db, 3, &[1, 2]).await;

    let scanned = scan_dark(&db, &[3]).await;

    assert_eq!(said(&scanned), []);
    assert_eq!(
        dark_seasons(&db).await,
        opened_by_ada_following_dark(RequestState::Pending)
    );
}

#[tokio::test]
async fn a_scan_opens_no_season_the_followed_show_already_covers() {
    let (_data, db) = ada_and_bob().await;
    let followed = follow_as(&db, ADA, dark(), RequestState::Approved).await;
    for season in season_ids(&followed) {
        db.write(move |tx| Box::pin(cancel(tx, season)))
            .await
            .unwrap();
    }
    match_dark_episodes(&db, 1, &[1, 2]).await;
    match_dark_episodes(&db, 2, &[1, 2]).await;

    let scanned = scan_dark(&db, &[1, 2]).await;

    assert_eq!(said(&scanned), []);
    assert_eq!(dark_seasons(&db).await, []);
}

#[tokio::test]
async fn a_scan_opens_no_season_that_was_once_available() {
    let (_data, db) = ada_and_bob().await;
    follow_as(&db, ADA, dark(), RequestState::Approved).await;
    let season_3 = WantedSeasons {
        seasons: vec![dark_season(3)],
        ..dark()
    };
    db.write(|tx| Box::pin(ask_seasons(tx, BOB, season_3, RequestState::Pending)))
        .await
        .unwrap();
    match_dark_episodes(&db, 3, &[1, 2]).await;
    scan_dark(&db, &[3]).await;

    let scanned_again = scan_dark(&db, &[3]).await;

    assert_eq!(said(&scanned_again), []);
    assert_eq!(
        dark_seasons(&db).await.last(),
        Some(&(3, RequestState::Available, vec![BOB]))
    );
}

#[tokio::test]
async fn a_season_a_scan_opened_does_not_open_again_once_cancelled() {
    let (_data, db) = ada_and_bob().await;
    follow_as(&db, ADA, dark(), RequestState::Approved).await;
    match_dark_episodes(&db, 3, &[1]).await;
    let opened = scan_dark(&db, &[3]).await;
    let Some(Happened::RequestCreated(created)) = opened.first().and_then(|one| one.event.as_ref())
    else {
        panic!("the scan opened no Request: {opened:?}");
    };
    let season_3 = created.request.id;
    db.write(move |tx| Box::pin(cancel(tx, season_3)))
        .await
        .unwrap();

    let scanned_again = scan_dark(&db, &[3]).await;

    assert_eq!(said(&scanned_again), []);
    assert_eq!(
        dark_seasons(&db).await,
        opened_by_ada_following_dark(RequestState::Approved)
    );
}

/// Ada follows Dark while Seasons 1 and 2 are out, Pending, then Bob joins her Followed Show
/// once Season 3 has aired. Returns the Followed Show's id and Season 3's Request id.
async fn bob_joins_following_dark_after_season_3_aired(db: &Database) -> (i64, i64) {
    follow_as(db, ADA, dark(), RequestState::Pending).await;
    let mut to_season_3 = dark();
    to_season_3.seasons.push(dark_season(3));
    let joined = follow_as(db, BOB, to_season_3, RequestState::Pending).await;
    let season_3 = joined.value.1.last().expect("Season 3's Request");
    assert_eq!(asked_season(season_3).season_number, 3);
    (joined.value.0.id, season_3.id)
}

/// Approves the Pending Followed Show `id`, and with it its Pending Season Requests.
async fn approve_followed(db: &Database, id: i64) {
    db.write(move |tx| Box::pin(decide_followed(tx, id, RequestState::Approved, None, None)))
        .await
        .unwrap();
}

#[tokio::test]
async fn a_scan_opens_no_season_asked_for_by_joining_a_followed_show_and_declined() {
    let (_data, db) = ada_and_bob().await;
    let (followed, season_3) = bob_joins_following_dark_after_season_3_aired(&db).await;
    db.write(move |tx| Box::pin(decline(tx, season_3, None)))
        .await
        .unwrap();
    approve_followed(&db, followed).await;
    match_dark_episodes(&db, 3, &[1, 2]).await;

    let scanned = scan_dark(&db, &[3]).await;

    assert_eq!(said(&scanned), []);
    assert_eq!(
        dark_seasons(&db).await.last(),
        Some(&(3, RequestState::Declined, vec![BOB]))
    );
}

#[tokio::test]
async fn a_scan_opens_no_season_asked_for_by_joining_a_followed_show_and_cancelled_once_approved() {
    let (_data, db) = ada_and_bob().await;
    let (followed, season_3) = bob_joins_following_dark_after_season_3_aired(&db).await;
    approve_followed(&db, followed).await;
    db.write(move |tx| Box::pin(cancel(tx, season_3)))
        .await
        .unwrap();
    match_dark_episodes(&db, 3, &[1, 2]).await;

    let scanned = scan_dark(&db, &[3]).await;

    assert_eq!(said(&scanned), []);
    assert_eq!(
        dark_seasons(&db).await.last(),
        Some(&(2, RequestState::Approved, vec![ADA, BOB]))
    );
}

#[tokio::test]
async fn asking_for_a_season_on_its_own_leaves_which_seasons_the_followed_show_covers() {
    let (_data, db) = ada_and_bob().await;
    follow_as(&db, ADA, dark(), RequestState::Approved).await;
    let season_3 = WantedSeasons {
        seasons: vec![dark_season(3)],
        ..dark()
    };
    let asked = db
        .write(|tx| Box::pin(ask_seasons(tx, BOB, season_3, RequestState::Pending)))
        .await
        .unwrap();
    let asked_id = asked.value[0].id;
    db.write(move |tx| Box::pin(decline(tx, asked_id, None)))
        .await
        .unwrap();
    match_dark_episodes(&db, 3, &[1, 2]).await;

    let scanned = scan_dark(&db, &[3]).await;

    assert_eq!(said(&scanned), [("created", RequestState::Available)]);
    assert_eq!(
        dark_seasons(&db).await.last(),
        Some(&(3, RequestState::Available, vec![ADA]))
    );
}

#[tokio::test]
async fn a_request_kept_under_a_made_up_namespace_becomes_available_when_a_library_matches_that_movie()
 {
    let (_data, db) = ada_and_bob().await;
    let anidb = ExternalId::new("anidb", "5101").unwrap();
    let wanted = Wanted {
        id: anidb.clone(),
        ..movie(348)
    };
    let id = db
        .write(move |tx| Box::pin(ask_movie(tx, ADA, wanted, RequestState::Pending)))
        .await
        .unwrap()
        .value
        .id;

    let folder = tempfile::tempdir().unwrap();
    let alien = Metadata {
        ids: ExternalIds::from_iter([anidb]),
        ..tmdb_movie(348, "Alien")
    };
    MatchedMovie::new(&db, folder.path(), alien).await;

    let mut conn = db.reader().acquire().await.unwrap();
    let request = load_one(&mut conn, id).await.unwrap().unwrap();
    assert_eq!(request.state, RequestState::Available);
}

#[tokio::test]
async fn a_season_request_kept_under_a_made_up_namespace_follows_how_much_of_it_a_library_has() {
    let (_data, db) = ada_and_bob().await;
    let show = ExternalId::new("anidb", "69").unwrap();
    let wanted = Wanted {
        id: show.clone(),
        season: Some(1),
        ..movie(69)
    };
    let id = db
        .write(move |tx| Box::pin(ask(tx, ADA, wanted, RequestState::Approved)))
        .await
        .unwrap()
        .value
        .id;
    settled(&db, id, Ok(fulfilled())).await;
    let scan = || {
        let seasons = vec![(
            ExternalIds::from_iter([show.clone()]),
            season_1("2017-12-08"),
        )];
        db.write(move |tx| Box::pin(super::make_seasons_available(tx, seasons)))
    };

    match_episodes(&db, show.clone(), 1, &[1]).await;
    let partly = scan().await.unwrap();
    match_episodes(&db, show.clone(), 1, &[2]).await;
    let fully = scan().await.unwrap();

    assert_eq!(said(&partly), [("moved", RequestState::PartiallyAvailable)]);
    assert_eq!(said(&fully), [("moved", RequestState::Available)]);
}

/// TVDB's id 5 and TMDB's id 9, which the tests below give one title.
fn tvdb_5_and_tmdb_9() -> (ExternalId, ExternalIds) {
    let tvdb = ExternalId::new("tvdb", "5").unwrap();
    (
        tvdb.clone(),
        ExternalIds::from_iter([tvdb, ExternalId::tmdb(9)]),
    )
}

#[tokio::test]
async fn a_movie_request_kept_under_tvdb_becomes_available_when_a_match_prefers_tmdb() {
    let (_data, db) = ada_and_bob().await;
    let (tvdb, both) = tvdb_5_and_tmdb_9();
    let wanted = Wanted {
        id: tvdb,
        ..movie(9)
    };
    let id = db
        .write(move |tx| Box::pin(ask_movie(tx, ADA, wanted, RequestState::Pending)))
        .await
        .unwrap()
        .value
        .id;

    let folder = tempfile::tempdir().unwrap();
    let matched = Metadata {
        ids: both,
        ..tmdb_movie(9, "Nine")
    };
    MatchedMovie::new(&db, folder.path(), matched).await;

    let mut conn = db.reader().acquire().await.unwrap();
    let request = load_one(&mut conn, id).await.unwrap().unwrap();
    assert_eq!(request.state, RequestState::Available);
}

#[tokio::test]
async fn a_season_request_kept_under_tvdb_becomes_available_when_a_match_prefers_tmdb() {
    let (_data, db) = ada_and_bob().await;
    let (tvdb, both) = tvdb_5_and_tmdb_9();
    let wanted = Wanted {
        id: tvdb,
        season: Some(1),
        ..movie(9)
    };
    let id = db
        .write(move |tx| Box::pin(ask(tx, ADA, wanted, RequestState::Approved)))
        .await
        .unwrap()
        .value
        .id;
    match_show_episodes(&db, both.clone(), 1, &[1, 2]).await;

    let seasons = vec![(both, season_1("2017-12-08"))];
    let scanned = db
        .write(move |tx| Box::pin(super::make_seasons_available(tx, seasons)))
        .await
        .unwrap();

    assert_eq!(said(&scanned), [("moved", RequestState::Available)]);
    let mut conn = db.reader().acquire().await.unwrap();
    let request = load_one(&mut conn, id).await.unwrap().unwrap();
    assert_eq!(request.state, RequestState::Available);
}

#[tokio::test]
async fn a_scan_opens_a_season_of_a_followed_show_kept_under_tvdb_when_a_match_prefers_tmdb() {
    let (_data, db) = ada_and_bob().await;
    let (tvdb, both) = tvdb_5_and_tmdb_9();
    let followed = WantedSeasons {
        show_id: tvdb,
        ..dark()
    };
    follow_as(&db, ADA, followed, RequestState::Approved).await;
    match_show_episodes(&db, both.clone(), 3, &[1, 2]).await;

    let seasons = vec![(both, dark_season(3))];
    let scanned = db
        .write(move |tx| Box::pin(super::make_seasons_available(tx, seasons)))
        .await
        .unwrap();

    assert_eq!(said(&scanned), [("created", RequestState::Available)]);
    assert_eq!(
        dark_seasons(&db).await.last(),
        Some(&(3, RequestState::Available, vec![ADA]))
    );
}

#[tokio::test]
async fn a_season_a_scan_opens_carries_the_tvdb_id_its_followed_show_stores_for_a_fulfiller_needing_it()
 {
    let (_data, db) = ada_and_bob().await;
    let tvdb = ExternalId::new("tvdb", "334824").unwrap();
    let followed = WantedSeasons {
        show_ids: ExternalIds::from_iter([ExternalId::tmdb(DARK), tvdb.clone()]),
        ..dark()
    };
    follow_as(&db, ADA, followed, RequestState::Approved).await;
    // The Library's source gave the Show only its tmdb id.
    match_dark_episodes(&db, 3, &[1, 2]).await;

    scan_dark(&db, &[3]).await;

    let newest = db.write(|tx| Box::pin(load(tx, None, None))).await.unwrap();
    let opened = &newest[0];
    assert_eq!(asked_season(opened).season_number, 3);
    let show_ids = &asked_season(opened).show_ids;
    assert!(
        (show_ids.iter()).any(|id| id.namespace == "tvdb" && id.value == "334824"),
        "{show_ids:?}"
    );
}
