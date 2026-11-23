use std::{collections::BTreeSet, sync::Arc};

use nama_plugin_api::PluginId;

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
    let data = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let registry = Registry::new();
    registry.describe_builtin(|| Arc::new(ScriptedSource::new("tmdb", &SourcesAsked::default())));
    let plugins = PluginSettings::for_tests(Arc::clone(&db), registry);
    let tmdb = PluginId::from_static("tmdb");
    plugins.enable(&tmdb).await.unwrap();
    let contents = Contents::new(Arc::clone(&db));
    contents.leave_title_rows("tmdb").await;

    plugins.disable(&tmdb).await.unwrap();

    let [details, similar, roles, air_dates, box_sets] = contents.title_rows_of("tmdb").await;
    assert!(!similar);
    assert!(details && roles && air_dates && box_sets);
    assert!(grants(&db).await.unwrap().contains_key("tmdb"));
}
