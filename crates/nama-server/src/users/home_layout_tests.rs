use super::*;
use crate::users::{Permissions, insert_user};

const PLUGIN: &str = "rows";

/// A database holding one User, with their id.
async fn with_a_user() -> (tempfile::TempDir, Database, i64) {
    let data_dir = tempfile::tempdir().unwrap();
    let db = Database::open(data_dir.path()).await.unwrap();
    let insert =
        db.write(|tx| Box::pin(async move { insert_user(tx, "Ada", Permissions::NEW_USER).await }));
    let ada = insert.await.unwrap().id;
    (data_dir, db, ada)
}

fn own(key: &str) -> HomeRowId {
    HomeRowId::Own {
        plugin: PluginId::from_static(PLUGIN),
        key: key.to_owned(),
    }
}

fn users_own() -> HomeRowId {
    HomeRowId::UsersOwn {
        plugin: PluginId::from_static(PLUGIN),
    }
}

fn shared(key: &str) -> HomeRowId {
    HomeRowId::Shared {
        plugin: PluginId::from_static(PLUGIN),
        key: key.to_owned(),
    }
}

/// A default Home layout showing `rows`, in that order.
fn default_of<const N: usize>(rows: [HomeRowId; N]) -> HomeLayout {
    HomeLayout {
        featured_shown: true,
        rows: rows.map(|row| PlacedRow { row, shown: true }).into(),
    }
}

/// `user`'s Home rows under `default` once they're given `rows`.
async fn arranged(
    db: &Database,
    user: i64,
    default: &HomeLayout,
    rows: Vec<HomeRowId>,
) -> Vec<HomeRowId> {
    let rows = rows.into_iter().map(|row| (row, ())).collect();
    let layout = UserHomeLayout::of(db, user).await.unwrap();
    let arranged = layout.arrange(default, rows).into_iter();
    arranged.map(|((row, ()), _)| row).collect()
}

#[tokio::test]
async fn a_first_reorder_keeps_a_row_after_the_users_own_rows_the_default_put_it_after() {
    let (_data_dir, db, ada) = with_a_user().await;
    let watching = HomeRowId::ContinueWatching;
    let default = default_of([users_own(), shared("unseen"), watching.clone()]);

    let asked = [own("for-ada"), watching.clone()];
    let seen = vec![watching.clone(), own("for-ada")];
    UserHomeLayout::reorder(&db, ada, &asked, seen, default.clone())
        .await
        .unwrap();

    let rows = vec![watching.clone(), shared("unseen"), own("for-ada")];
    let order = arranged(&db, ada, &default, rows).await;
    assert_eq!(order, [own("for-ada"), shared("unseen"), watching]);
}

#[tokio::test]
async fn a_first_reorder_while_a_plugin_is_disabled_keeps_where_the_users_own_rows_go() {
    let (_data_dir, db, ada) = with_a_user().await;
    let watching = HomeRowId::ContinueWatching;
    let default = default_of([users_own(), watching.clone()]);

    let asked = [watching.clone()];
    UserHomeLayout::reorder(&db, ada, &asked, vec![watching.clone()], default.clone())
        .await
        .unwrap();

    let rows = vec![watching.clone(), own("for-ada")];
    let order = arranged(&db, ada, &default, rows).await;
    assert_eq!(order, [own("for-ada"), watching]);
}

#[tokio::test]
async fn a_row_after_own_rows_a_user_has_none_of_goes_at_the_end() {
    let (_data_dir, db, ada) = with_a_user().await;
    let watching = HomeRowId::ContinueWatching;
    let default = default_of([users_own(), shared("unseen"), watching.clone()]);

    let asked = [shared("picks"), watching.clone()];
    let seen = vec![watching.clone(), shared("picks")];
    UserHomeLayout::reorder(&db, ada, &asked, seen, default.clone())
        .await
        .unwrap();

    let rows = vec![watching.clone(), shared("picks"), shared("unseen")];
    let order = arranged(&db, ada, &default, rows).await;
    assert_eq!(order, [shared("picks"), watching, shared("unseen")]);
}

#[test]
fn a_save_leaving_featured_unset_keeps_the_users_featured_flag() {
    let hidden = UserHomeLayout {
        featured_shown: Some(false),
        ..UserHomeLayout::default()
    };
    let asked = AskedHomeLayout {
        featured_shown: None,
        rows: Vec::new(),
    };

    let saved = hidden.asking(&asked, &default_of([]));

    assert_eq!(saved.featured_shown, Some(false));
}
