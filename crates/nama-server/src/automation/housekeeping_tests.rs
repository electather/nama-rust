use std::{fs, path::Path, sync::Arc};

use nama_plugin_api::automation::{
    BackUpAsked, BackedUp, ExpiredSignInsAsked, ExpiredSignInsDeleted, ForgetGonePluginsAsked,
    ForgetHomeRowsAsked, GonePluginsForgotten, HomeRowsForgotten, MetadataRefreshed, Permission,
    RefreshMetadataAsked,
};

use super::testing::{ALIEN_TMDB_ID, ask, kernel, leave_behind, left_behind, movies, users};
use crate::{
    backup,
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

#[tokio::test]
async fn back_up_writes_a_backup_named_by_the_time_into_the_data_directory_that_restores() {
    let data = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let (_automation, kernel) = kernel(&db, data.path(), Permission::PERMISSION_BACK_UP);

    let backed_up = ask::<BackedUp>(&kernel, "back_up", &BackUpAsked::default()).await;

    let backed_up = backed_up.unwrap();
    let name = &backed_up.name;
    assert!(
        name.starts_with("nama-2") && name.ends_with("Z.tar"),
        "{name}"
    );
    let path = data.path().join("backups").join(name);
    assert_eq!(fs::metadata(&path).unwrap().len(), backed_up.size_bytes);
    let restored = tempfile::tempdir().unwrap();
    let config = restored.path().join("nama.toml");
    backup::restore(restored.path(), &config, &path)
        .await
        .unwrap();
    assert!(restored.path().join("nama.db").is_file());
}

#[tokio::test]
async fn back_up_keeps_the_newest_7_it_wrote_and_never_deletes_another_file() {
    let data = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let (_automation, kernel) = kernel(&db, data.path(), Permission::PERMISSION_BACK_UP);
    let folder = data.path().join("backups");
    let older = older_backups(&folder, 7);
    let others = [
        "before-upgrade.tar",
        "nama-by-hand.tar",
        "nama-2020Z.tar",
        "notes.txt",
    ];
    for other in others {
        fs::write(folder.join(other), "mine").unwrap();
    }

    let backed_up = ask::<BackedUp>(&kernel, "back_up", &BackUpAsked::default()).await;

    let mut kept = older[1..].to_vec();
    kept.push(backed_up.unwrap().name);
    kept.extend(others.map(str::to_owned));
    kept.sort();
    assert_eq!(names_in(&folder), kept);
}

#[tokio::test]
async fn a_back_up_that_fails_deletes_nothing_and_leaves_no_backup() {
    let data = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    // A data directory holding no database, so the copy fails.
    let elsewhere = tempfile::tempdir().unwrap();
    let (_automation, kernel) = kernel(&db, elsewhere.path(), Permission::PERMISSION_BACK_UP);
    let folder = elsewhere.path().join("backups");
    let older = older_backups(&folder, 8);

    let refused = ask::<BackedUp>(&kernel, "back_up", &BackUpAsked::default()).await;

    let refused = refused.unwrap_err().0;
    assert!(
        refused.starts_with("writing the Backup failed: "),
        "{refused}"
    );
    assert_eq!(names_in(&folder), older);
}

/// Writes `count` Backups named as the Server names them into `folder`, from years ago, and gives
/// their names, oldest first.
fn older_backups(folder: &Path, count: u32) -> Vec<String> {
    fs::create_dir_all(folder).unwrap();
    (1..=count)
        .map(|day| {
            let name = format!("nama-202001{day:02}T030000Z.tar");
            fs::write(folder.join(&name), "old").unwrap();
            name
        })
        .collect()
}

/// The name of everything in `folder`, sorted.
fn names_in(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = (fs::read_dir(folder).unwrap())
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}
