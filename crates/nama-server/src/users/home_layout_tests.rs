use super::*;
use crate::users::{Permissions, insert_user};

#[tokio::test]
async fn a_first_reorder_keeps_a_row_after_the_users_own_rows_the_default_put_it_after() {
    let data_dir = tempfile::tempdir().unwrap();
    let db = Database::open(data_dir.path()).await.unwrap();
    let insert =
        db.write(|tx| Box::pin(async move { insert_user(tx, "Ada", Permissions::NEW_USER).await }));
    let ada = insert.await.unwrap().id;
    let plugin = PluginId::from_static("rows");
    let own = HomeRowId::Own {
        plugin: plugin.clone(),
        key: "for-ada".to_owned(),
    };
    let unseen = HomeRowId::Shared {
        plugin: plugin.clone(),
        key: "unseen".to_owned(),
    };
    let watching = HomeRowId::ContinueWatching;
    let default = HomeLayout {
        featured_shown: true,
        rows: [
            HomeRowId::UsersOwn { plugin },
            unseen.clone(),
            watching.clone(),
        ]
        .map(|row| PlacedRow { row, shown: true })
        .into(),
    };

    let asked = [own.clone(), watching.clone()];
    let seen = vec![watching.clone(), own.clone()];
    UserHomeLayout::reorder(&db, ada, &asked, seen, default.clone())
        .await
        .unwrap();

    let rows = vec![
        (watching.clone(), ()),
        (unseen.clone(), ()),
        (own.clone(), ()),
    ];
    let arranged = UserHomeLayout::of(&db, ada)
        .await
        .unwrap()
        .arrange(&default, rows);
    let order: Vec<HomeRowId> = arranged.into_iter().map(|((row, ()), _)| row).collect();
    assert_eq!(order, [own, unseen, watching]);
}
