use std::sync::Arc;

use nama_plugin_api::automation::{
    ExpiredSignInsAsked, ExpiredSignInsDeleted, ForgetGonePluginsAsked, ForgetHomeRowsAsked,
    GonePluginsForgotten, HomeRowsForgotten, MetadataRefreshed, Permission, RefreshMetadataAsked,
};

use super::testing::{ALIEN_TMDB_ID, ask, kernel, leave_behind, left_behind, movies, users};
use crate::{
    db::Database,
    library::scripted::{ScriptedSource, SourcesAsked},
};

#[tokio::test]
async fn forget_home_rows_refuses_0_days_and_answers_how_many_users_it_forgot() {
    let data = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let (_automation, kernel) = kernel(&db, data.path(), Permission::PERMISSION_FORGET_HOME_ROWS);
    let asked = |days| ForgetHomeRowsAsked {
        days,
        ..ForgetHomeRowsAsked::default()
    };

    let refused = ask::<HomeRowsForgotten>(&kernel, "forget_home_rows", &asked(0)).await;
    let forgotten = ask::<HomeRowsForgotten>(&kernel, "forget_home_rows", &asked(30)).await;

    assert_eq!(refused.unwrap_err().0, "days must be at least 1");
    assert_eq!(forgotten.unwrap().users, 0, "nobody has opened home");
}

#[tokio::test]
async fn delete_expired_sign_ins_answers_how_many_devices_and_invites_it_deleted() {
    let data = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let needs = Permission::PERMISSION_DELETE_EXPIRED_SIGN_INS;
    let (_automation, kernel) = kernel(&db, data.path(), needs);

    let asked = ExpiredSignInsAsked::default();
    let deleted = ask::<ExpiredSignInsDeleted>(&kernel, "delete_expired_sign_ins", &asked).await;

    let deleted = deleted.unwrap();
    assert_eq!(
        (deleted.devices, deleted.invites),
        (0, 0),
        "nothing has expired"
    );
}

#[tokio::test]
async fn forget_gone_plugins_deletes_what_a_long_gone_plugin_left_and_keeps_a_recent_or_disabled_ones()
 {
    let data = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let [ada] = users(&db, ["Ada"]).await;
    let (automation, kernel) = kernel(&db, data.path(), Permission::PERMISSION_FORGET_GONE_PLUGINS);
    let registry = automation.plugins.registry();
    registry.list_as_installed(vec![|| Arc::new(nama_plugin_log_events::LogEvents)]);
    automation.plugins.list().await.unwrap();
    // The disabled Plugin as long unseen as the gone one: the Server still having it keeps it.
    for (plugin, days_ago) in [("long-gone", 31), ("recently-gone", 29), ("log-events", 31)] {
        leave_behind(&db, plugin, ada, days_ago).await;
    }
    let asked = ForgetGonePluginsAsked {
        days: 30,
        ..ForgetGonePluginsAsked::default()
    };

    let forgotten = ask::<GonePluginsForgotten>(&kernel, "forget_gone_plugins", &asked).await;

    assert_eq!(forgotten.unwrap().plugin_ids, ["long-gone"]);
    assert_eq!(left_behind(&db, "long-gone", ada).await, [false; 8]);
    assert_eq!(left_behind(&db, "recently-gone", ada).await, [true; 8]);
    assert_eq!(left_behind(&db, "log-events", ada).await, [true; 8]);
}

#[tokio::test]
async fn refresh_metadata_answers_how_many_titles_it_fetched_again() {
    let data = tempfile::tempdir().unwrap();
    let media = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    movies(&db, media.path(), &[ALIEN_TMDB_ID]).await;
    let (automation, kernel) = kernel(&db, data.path(), Permission::PERMISSION_REFRESH_METADATA);
    let tmdb =
        ScriptedSource::new("tmdb", &SourcesAsked::default()).with_movie(&["tmdb-348"], "Alien");
    automation
        .plugins
        .registry()
        .enable(Arc::new(tmdb))
        .unwrap();

    let asked = RefreshMetadataAsked::default();
    let refreshed = ask::<MetadataRefreshed>(&kernel, "refresh_metadata", &asked).await;

    let refreshed = refreshed.unwrap();
    assert_eq!(
        (refreshed.titles, refreshed.unavailable_plugin_ids),
        (1, vec![])
    );
}
