use std::{fs, sync::Arc, time::Duration};

use nama_plugin_log_events::LogEvents;

use crate::{
    db::{self, Database},
    plugin_settings::PluginSettings,
    plugins::Registry,
};

async fn had_at(db: &Database, plugin: &str) -> Option<i64> {
    sqlx::query_scalar!("SELECT had_at FROM plugins_had WHERE plugin_id = ?", plugin)
        .fetch_optional(db.reader())
        .await
        .unwrap()
}

#[tokio::test]
async fn a_removed_plugins_last_had_time_is_the_listing_that_dropped_it_and_stops_moving() {
    let data = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let plugins = PluginSettings::for_tests(Arc::clone(&db), Registry::new());
    plugins
        .registry()
        .list_as_installed(vec![|| Arc::new(LogEvents)]);
    plugins.list().await.unwrap();
    let found_at = had_at(&db, "log-events").await;
    // Lib tests share the clock, so they step it by seconds only.
    db::advance_clock(Duration::from_secs(2));

    plugins.registry().list_as_installed(Vec::new());
    plugins.list().await.unwrap();
    let dropped_at = had_at(&db, "log-events").await;
    db::advance_clock(Duration::from_secs(2));
    plugins.list().await.unwrap();

    assert!(
        dropped_at > found_at,
        "it was had until the listing that dropped it"
    );
    assert_eq!(had_at(&db, "log-events").await, dropped_at);
}

#[tokio::test]
async fn while_the_server_holds_a_plugin_it_refused_every_plugin_it_had_stays_had() {
    let data = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let plugins = PluginSettings::new(Arc::clone(&db), Registry::new(), data.path().to_owned());
    plugins
        .registry()
        .list_as_installed(vec![|| Arc::new(LogEvents)]);
    plugins.list().await.unwrap();
    // Its new version, which the Server can't load.
    plugins.registry().list_as_installed(Vec::new());
    fs::create_dir(data.path().join("plugins")).unwrap();
    fs::write(data.path().join("plugins/log-events.wasm"), "not wasm").unwrap();
    plugins.list().await.unwrap();
    let refused_at = had_at(&db, "log-events").await;
    db::advance_clock(Duration::from_secs(2));

    plugins.list().await.unwrap();

    assert!(had_at(&db, "log-events").await > refused_at);
    assert_eq!(had_at(&db, "log-events.wasm").await, None);
}
