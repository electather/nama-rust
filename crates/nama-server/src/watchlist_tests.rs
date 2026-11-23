use std::sync::Arc;

use nama_plugin_api::{
    metadata::{ExternalId, ExternalIds},
    server_link::WatchlistEntry,
};

use super::{add, entries, has, list, remove, take_synced};
use crate::{
    db::Database,
    library::{
        Metadata, TitleKind,
        fixture::{MatchedMovie, tmdb_movie},
    },
};

async fn insert_user(db: &Database) -> i64 {
    db.write(|tx| {
        Box::pin(async move {
            sqlx::query_scalar(
                "INSERT INTO users (username, display_username, permissions, created_at,
                                    updated_at)
                 VALUES ('ada', 'Ada', 0, 0, 0) RETURNING id",
            )
            .fetch_one(&mut **tx)
            .await
        })
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn deleting_a_user_deletes_their_watchlist() {
    let dir = tempfile::tempdir().unwrap();
    let db = Database::open(dir.path()).await.unwrap();
    let ada = insert_user(&db).await;
    let alien = ExternalId::new("tmdb", "348").unwrap();
    add(&db, ada, TitleKind::Movie, &alien).await.unwrap();
    let ids = ExternalIds::from_iter([alien]);
    assert!(has(&db, ada, TitleKind::Movie, &ids).await.unwrap());

    db.write(move |tx| {
        Box::pin(async move {
            sqlx::query("DELETE FROM users WHERE id = ?")
                .bind(ada)
                .execute(&mut **tx)
                .await
                .map(drop)
        })
    })
    .await
    .unwrap();

    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM watchlist")
        .fetch_one(db.reader())
        .await
        .unwrap();
    assert_eq!(rows, 0);
}

#[tokio::test]
async fn list_answers_the_newest_added_first() {
    let dir = tempfile::tempdir().unwrap();
    let db = Database::open(dir.path()).await.unwrap();
    let ada = insert_user(&db).await;
    let [alien, heat, up] = ["348", "949", "14160"].map(|id| ExternalId::new("tmdb", id).unwrap());
    for id in [&up, &alien, &heat] {
        add(&db, ada, TitleKind::Movie, id).await.unwrap();
    }
    remove(&db, ada, TitleKind::Movie, &up).await.unwrap();
    add(&db, ada, TitleKind::Movie, &up).await.unwrap();

    let listed = list(&db, ada).await.unwrap();

    let ids: Vec<_> = listed.iter().map(|(_, id)| id.to_string()).collect();
    assert_eq!(ids, ["tmdb-14160", "tmdb-949", "tmdb-348"]);
}

#[tokio::test]
async fn a_title_taken_off_is_kept_as_a_removal_that_nothing_but_sync_reads() {
    let dir = tempfile::tempdir().unwrap();
    let db = Database::open(dir.path()).await.unwrap();
    let ada = insert_user(&db).await;
    let alien = ExternalId::tmdb(348);
    add(&db, ada, TitleKind::Movie, &alien).await.unwrap();

    remove(&db, ada, TitleKind::Movie, &alien).await.unwrap();

    let ids = ExternalIds::from_iter([alien]);
    assert!(!has(&db, ada, TitleKind::Movie, &ids).await.unwrap());
    assert_eq!(list(&db, ada).await.unwrap(), []);
    let kept = entries(&db, ada).await.unwrap();
    let kept: Vec<_> = (kept.iter())
        .map(|entry| (entry.id.as_str(), entry.removed))
        .collect();
    assert_eq!(kept, [("tmdb-348", true)]);
}

/// The Movie `tmdb-<id>`, on the Watchlist or taken off, as of `changed_at_ms`.
fn movie(id: u64, removed: bool, changed_at_ms: i64) -> WatchlistEntry {
    WatchlistEntry {
        show: false,
        id: format!("tmdb-{id}"),
        removed,
        changed_at_ms,
    }
}

#[tokio::test]
async fn taking_synced_entries_keeps_the_latest_change_removals_included() {
    let dir = tempfile::tempdir().unwrap();
    let db = Database::open(dir.path()).await.unwrap();
    let ada = insert_user(&db).await;
    let take = async |synced: Vec<WatchlistEntry>| {
        take_synced(&db, ada, synced).await.unwrap();
        entries(&db, ada).await.unwrap()
    };

    assert_eq!(
        take(vec![movie(348, false, 1_000)]).await,
        [movie(348, false, 1_000)]
    );
    assert_eq!(
        take(vec![movie(348, true, 500)]).await,
        [movie(348, false, 1_000)]
    );
    assert_eq!(
        take(vec![movie(348, true, 2_000)]).await,
        [movie(348, true, 2_000)]
    );
    assert_eq!(
        take(vec![movie(348, false, 1_500)]).await,
        [movie(348, true, 2_000)]
    );
    assert_eq!(
        take(vec![movie(348, false, 3_000)]).await,
        [movie(348, false, 3_000)]
    );
}

#[tokio::test]
async fn a_synced_title_the_server_lacks_is_kept_by_its_id() {
    let dir = tempfile::tempdir().unwrap();
    let db = Database::open(dir.path()).await.unwrap();
    let ada = insert_user(&db).await;
    let dark = WatchlistEntry {
        show: true,
        id: "tvdb-81189".to_owned(),
        removed: false,
        changed_at_ms: 1_000,
    };

    take_synced(&db, ada, vec![dark, movie(603, true, 1_000)])
        .await
        .unwrap();

    let listed = list(&db, ada).await.unwrap();
    assert_eq!(listed, [(TitleKind::Show, "tvdb-81189".parse().unwrap())]);
}

#[tokio::test]
async fn a_title_the_server_matched_is_taken_off_under_each_of_its_ids() {
    let dir = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(dir.path()).await.unwrap());
    let imdb = ExternalId::new("imdb", "tt0078748").unwrap();
    let alien = Metadata {
        ids: ExternalIds::from_iter([ExternalId::tmdb(348), imdb]),
        ..tmdb_movie(348, "Alien")
    };
    MatchedMovie::new(&db, dir.path(), alien).await;
    let ada = insert_user(&db).await;
    add(&db, ada, TitleKind::Movie, &ExternalId::tmdb(348))
        .await
        .unwrap();

    remove(&db, ada, TitleKind::Movie, &ExternalId::tmdb(348))
        .await
        .unwrap();

    let mut kept: Vec<_> = (entries(&db, ada).await.unwrap().into_iter())
        .map(|entry| (entry.id, entry.removed))
        .collect();
    kept.sort();
    assert_eq!(
        kept,
        [
            ("imdb-tt0078748".to_owned(), true),
            ("tmdb-348".to_owned(), true)
        ]
    );
}
