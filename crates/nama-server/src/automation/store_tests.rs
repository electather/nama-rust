use std::sync::Arc;

use buffa::Message as _;
use nama_hub_proto::nama::hub::v1::{
    RecordViewingEventsRequest, TrackPreferenceChange, UserViewingEvent, WatchStateChange,
};
use nama_plugin_api::PluginId;
use sqlx::{Connection, SqliteConnection, sqlite::SqliteConnectOptions};
use tempfile::TempDir;

use super::{Error, delete, get, put, put_within, take_over_hub_viewing_events};
use crate::{
    db::{self, Database},
    link::{self, Link, Linking, ServerLink},
    users::testing::insert_user,
};

async fn open() -> (TempDir, Database) {
    let data_dir = tempfile::tempdir().unwrap();
    let db = Database::open(data_dir.path()).await.unwrap();
    (data_dir, db)
}

#[tokio::test]
async fn values_round_trip_plugin_wide_and_per_user_apart() {
    let (_dir, db) = open().await;
    let ada = insert_user(&db, "ada").await;
    let cleanup = PluginId::from_static("cleanup");

    put(&db, &cleanup, None, "seen".into(), b"all".to_vec())
        .await
        .unwrap();
    put(&db, &cleanup, Some(ada), "seen".into(), b"ada".to_vec())
        .await
        .unwrap();
    put(&db, &cleanup, None, "seen".into(), b"all again".to_vec())
        .await
        .unwrap();
    let plugin_wide = get(&db, &cleanup, None, "seen").await.unwrap();
    let adas = get(&db, &cleanup, Some(ada), "seen").await.unwrap();
    delete(&db, &cleanup, Some(ada), "seen".into())
        .await
        .unwrap();

    assert_eq!(plugin_wide.as_deref(), Some(&b"all again"[..]));
    assert_eq!(adas.as_deref(), Some(&b"ada"[..]));
    assert_eq!(get(&db, &cleanup, Some(ada), "seen").await.unwrap(), None);
    assert!(get(&db, &cleanup, None, "seen").await.unwrap().is_some());
}

#[tokio::test]
async fn one_plugin_cant_read_anothers_entries() {
    let (_dir, db) = open().await;
    put(
        &db,
        &PluginId::from_static("cleanup"),
        None,
        "seen".into(),
        b"1".to_vec(),
    )
    .await
    .unwrap();

    let other = get(&db, &PluginId::from_static("other"), None, "seen")
        .await
        .unwrap();

    assert_eq!(other, None);
}

#[tokio::test]
async fn a_long_key_or_a_large_value_is_refused() {
    let (_dir, db) = open().await;
    let cleanup = PluginId::from_static("cleanup");

    let key = put(&db, &cleanup, None, "k".repeat(257), Vec::new()).await;
    let value = put(&db, &cleanup, None, "k".into(), vec![0; 64 * 1024 + 1]).await;
    let largest = put(&db, &cleanup, None, "k".repeat(256), vec![0; 64 * 1024]).await;

    assert!(matches!(key, Err(Error::KeyTooLong(257))), "{key:?}");
    assert_eq!(
        key.unwrap_err().to_string(),
        "a Store key can't be over 256 bytes, and this one is 257"
    );
    assert!(
        matches!(value, Err(Error::ValueTooLarge(65_537))),
        "{value:?}"
    );
    largest.unwrap();
}

#[tokio::test]
async fn deleting_a_user_deletes_their_entries() {
    let (_dir, db) = open().await;
    let ada = insert_user(&db, "ada").await;
    let cleanup = PluginId::from_static("cleanup");
    put(&db, &cleanup, Some(ada), "seen".into(), b"1".to_vec())
        .await
        .unwrap();

    db.write(move |tx| {
        Box::pin(async move {
            sqlx::query("DELETE FROM users WHERE id = ?")
                .bind(ada)
                .execute(&mut **tx)
                .await
        })
    })
    .await
    .unwrap();
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM plugin_store")
        .fetch_one(db.reader())
        .await
        .unwrap();

    assert_eq!(left, 0);
}

#[tokio::test]
async fn an_entry_for_no_user_is_refused() {
    let (_dir, db) = open().await;

    let put = put(
        &db,
        &PluginId::from_static("cleanup"),
        Some(42),
        "seen".into(),
        Vec::new(),
    )
    .await;

    assert!(matches!(put, Err(Error::NoUser(42))), "{put:?}");
}

#[tokio::test]
async fn at_the_cap_a_new_key_is_refused_and_a_replacement_allowed() {
    let (_dir, db) = open().await;
    let ada = insert_user(&db, "ada").await;
    let cleanup = PluginId::from_static("cleanup");
    let put = |user, key: &str, value: &[u8]| {
        put_within(&db, &cleanup, user, key.into(), value.to_vec(), 2)
    };
    put(None, "seen", b"1").await.unwrap();
    put(Some(ada), "seen", b"1").await.unwrap();

    let new_key = put(None, "other", b"1").await;
    let replaced = put(Some(ada), "seen", b"2").await;
    let other_plugin = put_within(
        &db,
        &PluginId::from_static("other"),
        None,
        "seen".into(),
        Vec::new(),
        2,
    )
    .await;

    assert!(matches!(new_key, Err(Error::Full(2))), "{new_key:?}");
    assert_eq!(
        new_key.unwrap_err().to_string(),
        "a Plugin can't keep over 2 entries in its Store; delete one to keep a new key"
    );
    replaced.unwrap();
    other_plugin.unwrap();
    let adas = get(&db, &cleanup, Some(ada), "seen").await.unwrap();
    assert_eq!(adas.as_deref(), Some(&b"2"[..]));
    assert_eq!(get(&db, &cleanup, None, "other").await.unwrap(), None);
}

/// The version of the first migration that moves the Hub Plugin's Links into the Kernel.
const HUB_LINK: i64 = 20_261_124_195_716;

#[tokio::test]
async fn upgrading_hands_the_kernel_the_links_the_hub_plugin_kept_but_those_it_ignored() {
    let data_dir = tempfile::tempdir().unwrap();
    let options = SqliteConnectOptions::new()
        .filename(data_dir.path().join(db::FILE_NAME))
        .create_if_missing(true);
    let mut conn = SqliteConnection::connect_with(&options).await.unwrap();
    let migrations = sqlx::migrate!();
    let before = (migrations.iter().map(|migration| migration.version))
        .filter(|&version| version < HUB_LINK)
        .max();
    migrations.run_to(before.unwrap(), &mut conn).await.unwrap();
    // Grace Linked under the Server's Link, which the SHA-256 of its credential names, Hedy under
    // an earlier one, and a User since deleted.
    sqlx::raw_sql(
        r#"INSERT INTO users (id, username, display_username, permissions, created_at, updated_at)
           VALUES (1, 'grace', 'Grace', 0, 0, 0), (2, 'hedy', 'Hedy', 0, 0, 0);
           INSERT INTO plugin_store (plugin_id, user_id, key, value)
           VALUES ('hub', NULL, 'link',
                   CAST('{"credential":"credential-1","account":"ada@example.com"}' AS BLOB)),
                  ('hub', NULL, 'key-set', CAST('{"keys":[]}' AS BLOB)),
                  ('hub', NULL, 'user-link/1',
                   CAST('{"server_link":"JcTPDqMYbHP4z7nvSOvqBu_FBOr4UZ_MB7tyZOu3xJE",
                          "account":"grace@example.com"}' AS BLOB)),
                  ('hub', NULL, 'user-link/2',
                   CAST('{"server_link":"earlier","account":"hedy@example.com"}' AS BLOB)),
                  ('hub', NULL, 'user-link/9',
                   CAST('{"server_link":"earlier","account":"gone@example.com"}' AS BLOB));"#,
    )
    .execute(&mut conn)
    .await
    .unwrap();
    conn.close().await.unwrap();

    let db = Arc::new(Database::open(data_dir.path()).await.unwrap());
    // Nothing listens there: the kept Link is read without asking the Hub.
    let server = Arc::new(ServerLink::new("http://127.0.0.1:1", Arc::clone(&db)).unwrap());
    let credential = server.credential().await;
    let linking = Linking::new(Some(server));
    let grace = linking.is_user_linked(1).await.unwrap();
    let hedy = linking.is_user_linked(2).await.unwrap();
    let link = linking.server_link().await.unwrap();
    let left: Vec<String> = sqlx::query_scalar("SELECT key FROM plugin_store")
        .fetch_all(db.reader())
        .await
        .unwrap();

    assert_eq!(credential.as_deref(), Some("credential-1"));
    assert_eq!(
        link,
        Link::Linked {
            account: "ada@example.com".to_owned(),
            reachability: None,
        }
    );
    assert!(grace, "Grace's Link was lost");
    assert!(!hedy, "Hedy's Link under an earlier Server Link shows");
    assert_eq!(left, Vec::<String>::new());
}

/// The version of the migration that moves the Hub Plugin's queues into the Kernel.
const HUB_SYNC_QUEUE: i64 = 20_261_124_215_157;

/// The version of the migration that moves the Hub Plugin's queued Viewing Events into the Kernel.
const HUB_VIEWING_EVENTS: i64 = 20_261_125_020_924;

/// A database migrated up to just before the migration `version`, holding `store` in its Plugin
/// Store, then opened, which runs the rest.
async fn upgraded_with_queues(version: i64, store: &'static str) -> (TempDir, Database) {
    let data_dir = tempfile::tempdir().unwrap();
    let options = SqliteConnectOptions::new()
        .filename(data_dir.path().join(db::FILE_NAME))
        .create_if_missing(true);
    let mut conn = SqliteConnection::connect_with(&options).await.unwrap();
    let migrations = sqlx::migrate!();
    let before = (migrations.iter().map(|migration| migration.version))
        .filter(|&before| before < version)
        .max();
    migrations.run_to(before.unwrap(), &mut conn).await.unwrap();
    sqlx::raw_sql(
        "INSERT INTO users (id, username, display_username, permissions, created_at, updated_at)
         VALUES (7, 'ada', 'Ada', 0, 0, 0)",
    )
    .execute(&mut conn)
    .await
    .unwrap();
    sqlx::raw_sql(store).execute(&mut conn).await.unwrap();
    conn.close().await.unwrap();
    let db = Database::open(data_dir.path()).await.unwrap();
    (data_dir, db)
}

/// What waits of a kind, as the Hub API's messages.
async fn waiting_as<T: serde::de::DeserializeOwned>(
    db: &Database,
    kind: link::QueuedKind,
) -> Vec<T> {
    (link::queued_for_hub(db, kind).await.unwrap().iter())
        .map(|queued| serde_json::from_str(&queued.change).unwrap())
        .collect()
}

#[tokio::test]
async fn upgrading_queues_what_the_hub_plugin_had_queued_in_order_and_empties_its_store() {
    // Pages as the Plugin kept them: a header, then numbered JSON arrays, and an older Plugin's one
    // array of Preferences. User 8 is gone.
    let (_dir, db) = upgraded_with_queues(
        HUB_SYNC_QUEUE,
        r#"INSERT INTO plugin_store (plugin_id, user_id, key, value) VALUES
           ('hub', NULL, 'watch-state-queue', CAST('{"first":9,"next":11}' AS BLOB)),
           ('hub', NULL, 'watch-state-queue/10', CAST('[{"userId":"7","ids":["tmdb-2"],"changedAtMs":"2000"}]' AS BLOB)),
           ('hub', NULL, 'watch-state-queue/9', CAST('[{"userId":"7","ids":["tmdb-1"],"positionMs":"60000","changedAtMs":"1000"},{"userId":"8","ids":["tmdb-3"],"changedAtMs":"1500"}]' AS BLOB)),
           ('hub', NULL, 'track-preference-queue', CAST('[{"userId":"7","language":"eng","changedAtMs":"500"}]' AS BLOB)),
           ('hub', NULL, 'track-preferences', CAST('{"first":0,"next":1}' AS BLOB)),
           ('hub', NULL, 'track-preferences/0', CAST('[{"userId":"7","audio":true,"language":"jpn","changedAtMs":"700"}]' AS BLOB)),
           ('hub', NULL, 'similar-cache', CAST('{}' AS BLOB))"#,
    )
    .await;

    let watch_states: Vec<WatchStateChange> = waiting_as(&db, link::QueuedKind::WatchState).await;
    let preferences: Vec<TrackPreferenceChange> =
        waiting_as(&db, link::QueuedKind::Preference).await;
    let left: Vec<String> = sqlx::query_scalar("SELECT key FROM plugin_store")
        .fetch_all(db.reader())
        .await
        .unwrap();

    let titles: Vec<_> = (watch_states.iter())
        .map(|change| (change.user_id, change.ids[0].as_str(), change.changed_at_ms))
        .collect();
    assert_eq!(titles, [(7, "tmdb-1", 1000), (7, "tmdb-2", 2000)]);
    assert_eq!(watch_states[0].position_ms, 60_000);
    let languages: Vec<_> = (preferences.iter())
        .map(|change| (change.audio, change.language.as_str()))
        .collect();
    assert_eq!(languages, [(false, "eng"), (true, "jpn")]);
    assert_eq!(left, ["similar-cache"]);
}

#[tokio::test]
async fn upgrading_queues_the_viewing_events_the_hub_plugin_had_queued_and_empties_its_store() {
    // Pages as the Plugin kept them. User 8 is gone.
    let (_dir, db) = upgraded_with_queues(
        HUB_VIEWING_EVENTS,
        r#"INSERT INTO plugin_store (plugin_id, user_id, key, value) VALUES
           ('hub', NULL, 'viewing-event-queue', CAST('{"first":9,"next":11}' AS BLOB)),
           ('hub', NULL, 'viewing-event-queue/10', CAST('[{"userId":"7","key":"c","event":{"externalId":"tmdb-3"}}]' AS BLOB)),
           ('hub', NULL, 'viewing-event-queue/9', CAST('[{"userId":"7","key":"a","event":{"externalId":"tmdb-1","startedAt":"1700000000"}},{"userId":"8","key":"b"}]' AS BLOB)),
           ('hub', NULL, 'similar-cache', CAST('{}' AS BLOB))"#,
    )
    .await;
    // As a Plugin of v0.2.0 kept them, the request that sends them, which the Kernel takes over as
    // it starts, once the migrations have run.
    let unsent = RecordViewingEventsRequest {
        events: [(7, "z"), (8, "y")]
            .map(|(user_id, key)| UserViewingEvent {
                user_id,
                key: key.to_owned(),
                ..Default::default()
            })
            .into(),
        ..Default::default()
    };
    let hub = PluginId::from_static("hub");
    (put(
        &db,
        &hub,
        None,
        "viewing-events".into(),
        unsent.encode_to_vec(),
    ))
    .await
    .unwrap();

    take_over_hub_viewing_events(&db).await.unwrap();

    let events: Vec<UserViewingEvent> = waiting_as(&db, link::QueuedKind::ViewingEvent).await;
    let left: Vec<String> = sqlx::query_scalar("SELECT key FROM plugin_store")
        .fetch_all(db.reader())
        .await
        .unwrap();
    let keys: Vec<_> = (events.iter())
        .map(|queued| (queued.user_id, queued.key.as_str()))
        .collect();
    assert_eq!(keys, [(7, "a"), (7, "c"), (7, "z")]);
    assert_eq!(events[0].event.started_at, 1_700_000_000);
    assert_eq!(left, ["similar-cache"]);
}
