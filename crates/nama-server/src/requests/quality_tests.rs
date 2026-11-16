use nama_plugin_api::metadata::{ExternalId, ExternalIds};
use nama_proto::nama::v1::{RequestState, RequestedQuality};

use super::{NewQuality, change_quality};
use crate::db::Database;
use crate::requests::asking::{WantedSeasons, ask_seasons};
use crate::requests::followed::{decide_followed, load_followed};
use crate::requests::testing::{ADA, BOB, ada_and_bob, dark, follow_as, read};

/// The Quality the tests change to.
fn hd() -> NewQuality {
    NewQuality {
        quality: RequestedQuality {
            id: 7,
            name: "HD".to_owned(),
            profile_id: "4".to_owned(),
            ..Default::default()
        },
        by: ADA,
    }
}

/// Ada's Pending Season Requests for Dark, kept under its TMDB id and knowing its TVDB id, and
/// Bob's Pending Followed Show of Dark, opened under its TVDB id alone. Returns the Requests'
/// ids and the Followed Show's.
async fn dark_under_two_ids(db: &Database) -> (Vec<i64>, i64) {
    let tvdb = ExternalId::new("tvdb", "334824").unwrap();
    let asked = WantedSeasons {
        show_ids: ExternalIds::from_iter([tvdb.clone()]),
        ..dark()
    };
    let seasons = db
        .write(|tx| Box::pin(ask_seasons(tx, ADA, asked, RequestState::Pending)))
        .await
        .unwrap();
    let followed = WantedSeasons {
        show_id: tvdb,
        ..dark()
    };
    let followed = follow_as(db, BOB, followed, RequestState::Pending).await;
    let seasons = seasons.value.iter().map(|request| request.id).collect();
    (seasons, followed.value.0.id)
}

/// The Quality ids the Requests `seasons` and the Followed Show `followed` are asked at.
async fn qualities_of(db: &Database, seasons: &[i64], followed: i64) -> Vec<Option<i64>> {
    let mut ids = Vec::new();
    for &season in seasons {
        ids.push(read(db, season).await.quality.as_option().map(|q| q.id));
    }
    let mut conn = db.reader().acquire().await.unwrap();
    let followed = load_followed(&mut conn, Some(followed), None)
        .await
        .unwrap();
    ids.push(followed[0].quality.as_option().map(|q| q.id));
    ids
}

#[tokio::test]
async fn changing_a_season_requests_quality_changes_a_followed_show_kept_under_another_id() {
    let (_data, db) = ada_and_bob().await;
    let (seasons, followed) = dark_under_two_ids(&db).await;

    let first = seasons[0];
    db.write(move |tx| Box::pin(change_quality(tx, first, Some(hd()))))
        .await
        .unwrap();

    assert_eq!(qualities_of(&db, &seasons, followed).await, [Some(7); 3]);
}

#[tokio::test]
async fn approving_a_followed_show_with_a_quality_changes_seasons_kept_under_another_id() {
    let (_data, db) = ada_and_bob().await;
    let (seasons, followed) = dark_under_two_ids(&db).await;

    db.write(move |tx| {
        Box::pin(decide_followed(
            tx,
            followed,
            RequestState::Approved,
            None,
            Some(hd()),
        ))
    })
    .await
    .unwrap();

    assert_eq!(qualities_of(&db, &seasons, followed).await, [Some(7); 3]);
}
