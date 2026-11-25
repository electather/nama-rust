use buffa::EnumValue;
use nama_plugin_api::{
    PluginId,
    contract::{SyncedWatchState, TitleId, TitleRefKind},
    points::WatchStateChange,
};

use super::{delete_plugins_import_cursors, import_cursor, imported, keep_import_cursor, synced};
use crate::db::Database;

fn episode_change() -> WatchStateChange {
    WatchStateChange {
        episode: true,
        ids: vec!["tmdb-62085".to_owned(), "tvdb-349232".to_owned()],
        played: true,
        position_ms: 0,
        duration_ms: 0,
        changed_at_ms: 1_700_000_000_000,
    }
}

#[test]
fn a_change_told_to_a_plugin_comes_back_as_the_same_change() {
    let change = episode_change();

    assert_eq!(imported(&synced(&change)), Some(change));
}

#[test]
fn an_imported_state_of_no_kind_or_without_a_well_formed_id_is_dropped() {
    let state = synced(&episode_change());
    let no_kind = SyncedWatchState {
        kind: EnumValue::Known(TitleRefKind::TITLE_REF_KIND_UNSPECIFIED),
        ..state.clone()
    };
    let malformed = TitleId {
        namespace: "TMDB!".to_owned(),
        value: "1".to_owned(),
        ..TitleId::default()
    };
    let no_good_id = SyncedWatchState {
        ids: vec![malformed],
        ..state
    };

    assert_eq!(imported(&no_kind), None);
    assert_eq!(imported(&no_good_id), None);
}

const TRACKER: PluginId = PluginId::from_static("tracker");

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
async fn an_import_cursor_survives_a_restart_and_goes_with_its_user() {
    let data = tempfile::tempdir().unwrap();
    let before = Database::open(data.path()).await.unwrap();
    let ada = insert_user(&before).await;
    (keep_import_cursor(&before, ada, &TRACKER, "page-2".to_owned()).await).unwrap();
    drop(before);

    let db = Database::open(data.path()).await.unwrap();
    let kept = import_cursor(&db, ada, &TRACKER).await.unwrap();
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

    assert_eq!(kept, "page-2");
    assert_eq!(import_cursor(&db, ada, &TRACKER).await.unwrap(), "");
}

#[tokio::test]
async fn deleting_a_plugins_import_cursors_leaves_another_plugins() {
    let data = tempfile::tempdir().unwrap();
    let db = Database::open(data.path()).await.unwrap();
    let ada = insert_user(&db).await;
    let other = PluginId::from_static("other-tracker");
    for plugin in [&TRACKER, &other] {
        (keep_import_cursor(&db, ada, plugin, "page-2".to_owned()).await).unwrap();
    }

    db.write(|tx| Box::pin(delete_plugins_import_cursors(tx, "tracker")))
        .await
        .unwrap();

    assert_eq!(import_cursor(&db, ada, &TRACKER).await.unwrap(), "");
    assert_eq!(import_cursor(&db, ada, &other).await.unwrap(), "page-2");
}
