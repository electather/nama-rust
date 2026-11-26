use super::{is_on, set};
use crate::{db::Database, users::testing::insert_user};

#[tokio::test]
async fn deleting_a_user_deletes_their_autoplay_off_row() {
    let dir = tempfile::tempdir().unwrap();
    let db = Database::open(dir.path()).await.unwrap();
    let ada = insert_user(&db, "ada").await;
    set(&db, ada, false).await.unwrap();
    assert!(!is_on(&db, ada).await.unwrap());

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

    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM autoplay_off")
        .fetch_one(db.reader())
        .await
        .unwrap();
    assert_eq!(rows, 0);
}
