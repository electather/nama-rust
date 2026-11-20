use nama_plugin_api::metadata::{ExternalId, ExternalIds};

use super::{add, has};
use crate::{db::Database, library::TitleKind};

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
