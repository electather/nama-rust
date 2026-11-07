//! Plugin settings in the database, imported once from the config's `[plugins.*]` tables.

use std::collections::BTreeMap;

use nama_plugin_api::settings::SettingValue;
use nama_server::{config::PluginConfig, db::Database, plugin_settings};

#[tokio::test]
async fn nothing_is_stored_until_the_config_is_imported_and_then_the_import_is_what_is_stored() {
    let data_dir = tempfile::tempdir().unwrap();
    let db = Database::open(data_dir.path()).await.unwrap();
    let imported = BTreeMap::from([
        (
            "tmdb".to_owned(),
            PluginConfig {
                enabled: true,
                settings: BTreeMap::from([
                    (
                        "api_key".to_owned(),
                        SettingValue::Text("the key".to_owned()),
                    ),
                    ("retries".to_owned(), SettingValue::Integer(3)),
                    ("adult".to_owned(), SettingValue::Bool(false)),
                ]),
            },
        ),
        (
            "log-events".to_owned(),
            PluginConfig {
                enabled: false,
                settings: BTreeMap::new(),
            },
        ),
    ]);

    assert_eq!(plugin_settings::stored(&db).await.unwrap(), None);
    plugin_settings::import(&db, &imported).await.unwrap();

    assert_eq!(plugin_settings::stored(&db).await.unwrap(), Some(imported));
    db.close().await;
}

#[tokio::test]
async fn importing_no_tables_still_records_the_import() {
    let data_dir = tempfile::tempdir().unwrap();
    let db = Database::open(data_dir.path()).await.unwrap();

    plugin_settings::import(&db, &BTreeMap::new())
        .await
        .unwrap();

    assert_eq!(
        plugin_settings::stored(&db).await.unwrap(),
        Some(BTreeMap::new())
    );
    db.close().await;
}
