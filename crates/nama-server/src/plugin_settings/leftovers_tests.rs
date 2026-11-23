use std::{collections::BTreeSet, sync::Arc};

use nama_plugin_api::PluginId;
use tempfile::TempDir;
use tokio::sync::mpsc;

use super::{LEFTOVERS, OLDER_COLUMNS};
use crate::{
    db::Database,
    library::{
        Contents,
        scripted::{ScriptedSource, SourcesAsked},
    },
    plugin_settings::{PluginSettings, grants},
    plugins::Registry,
};

// "One owner for what a Plugin leaves behind" in the crate's AGENTS.md.
#[tokio::test]
async fn every_table_naming_a_plugin_is_listed_with_when_its_rows_go_or_why_they_stay() {
    let data = tempfile::tempdir().unwrap();
    let db = Database::open(data.path()).await.unwrap();
    let columns: Vec<(String, String)> = sqlx::query_as(
        "SELECT m.name, p.name FROM sqlite_schema AS m, pragma_table_info(m.name) AS p
         WHERE m.type = 'table'",
    )
    .fetch_all(db.reader())
    .await
    .unwrap();

    let naming_a_plugin: BTreeSet<&str> = (columns.iter())
        .filter(|(table, column)| {
            column.contains("plugin") || OLDER_COLUMNS.contains(&(table.as_str(), column.as_str()))
        })
        .map(|(table, _)| table.as_str())
        .collect();
    let listed: BTreeSet<&str> = LEFTOVERS.iter().map(|(table, _)| *table).collect();

    assert_eq!(naming_a_plugin, listed);
}

#[tokio::test]
async fn disabling_a_plugin_deletes_its_similar_titles_and_keeps_its_settings_and_other_rows() {
    let (_data, db, plugins) = tmdb_with_title_rows().await;

    plugins.disable(&tmdb()).await.unwrap();

    assert_only_similar_titles_gone(&db).await;
}

#[tokio::test]
async fn a_crash_deletes_the_plugins_similar_titles_and_keeps_its_settings_and_other_rows() {
    let (_data, db, plugins) = tmdb_with_title_rows().await;
    // As the registry does for a crashing Plugin: disables it, then hands its id over to save.
    plugins.registry().disable(&tmdb()).await;
    let (crashing, crashed) = mpsc::unbounded_channel();
    crashing.send(tmdb()).unwrap();
    drop(crashing);

    plugins.clone().save_crashed(crashed).await;

    assert_only_similar_titles_gone(&db).await;
}

fn tmdb() -> PluginId {
    PluginId::from_static("tmdb")
}

/// An enabled `tmdb` that left a row in each per-title store.
async fn tmdb_with_title_rows() -> (TempDir, Arc<Database>, PluginSettings) {
    let data = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let registry = Registry::new();
    registry.describe_builtin(|| Arc::new(ScriptedSource::new("tmdb", &SourcesAsked::default())));
    let plugins = PluginSettings::for_tests(Arc::clone(&db), registry);
    plugins.enable(&tmdb()).await.unwrap();
    Contents::new(Arc::clone(&db))
        .leave_title_rows("tmdb")
        .await;
    (data, db, plugins)
}

async fn assert_only_similar_titles_gone(db: &Arc<Database>) {
    let [details, similar, roles, air_dates, box_sets] =
        Contents::new(Arc::clone(db)).title_rows_of("tmdb").await;
    assert!(!similar);
    assert!(details && roles && air_dates && box_sets);
    assert!(grants(db).await.unwrap().contains_key("tmdb"));
}
