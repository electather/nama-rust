use std::{path::Path, sync::Arc};

use buffa::EnumValue;
use nama_plugin_api::{
    PluginId,
    contract::{SyncedWatchState, TitleId, TitleRefKind},
    metadata::{ExternalId, ExternalIds},
    points::WatchStateChange,
};

use super::{
    delete_plugins_import_cursors, import_cursor, imported, keep_import_cursor, numbered, synced,
};
use crate::{
    db::Database,
    library::{
        Change, Contents, Decided, Item, Libraries, LibraryKind, Metadata,
        fixture::{decide, tmdb_movie},
        title_source::title_id,
    },
};

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

const DARK_TMDB_ID: i64 = 70_523;
const DARK_TVDB_ID: i64 = 121_361;
const LIES_TMDB_ID: i64 = 1_350_210;

fn dark_ids() -> ExternalIds {
    ExternalIds::from_iter([
        ExternalId::tmdb(DARK_TMDB_ID),
        ExternalId::tvdb(DARK_TVDB_ID),
    ])
}

/// The Library in `data`, holding in a Shows Library on `media` one file of Dark's Season 1
/// Episode 1, Matched to `tmdb-1`, and Episode 2, Matched to Lies.
async fn library_with_dark(data: &Path, media: &Path) -> Contents {
    let db = Arc::new(Database::open(data).await.unwrap());
    let root = media.canonicalize().unwrap();
    let root = root.to_str().unwrap();
    let library = (Libraries::new(Arc::clone(&db)))
        .create(root, LibraryKind::Shows, vec![root.to_owned()])
        .await
        .unwrap()
        .id;
    let path = format!("{root}/Dark - S01E01-E02.mkv");
    let episode = |number| Item::Episode {
        show: "Dark".to_owned(),
        year: None,
        hint: None,
        season: 1,
        episode: number,
    };
    let file = Change::Store {
        edition: None,
        path: path.clone(),
        size: 10,
        modified_ns: 0,
        items: vec![episode(1), episode(2)],
    };
    let contents = Contents::new(db);
    contents.apply(library, vec![file]).await.unwrap();
    let matched = |tmdb, title| {
        Decided::Matched(Box::new(Metadata {
            show_ids: dark_ids(),
            ..tmdb_movie(tmdb, title)
        }))
    };
    let decided = vec![matched(1, "Secrets"), matched(LIES_TMDB_ID, "Lies")];
    decide(&contents, library, &path, decided).await;
    contents
}

fn played(episode: bool, id: &str) -> WatchStateChange {
    WatchStateChange {
        episode,
        ids: vec![id.to_owned()],
        ..episode_change()
    }
}

#[tokio::test]
async fn an_episode_in_a_library_is_told_with_its_shows_ids_season_and_number() {
    let (data, media) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let contents = library_with_dark(data.path(), media.path()).await;
    let lies = played(true, &format!("tmdb-{LIES_TMDB_ID}"));

    let told = numbered(&contents, std::slice::from_ref(&lies)).await;

    let show_ids: Vec<TitleId> = dark_ids().iter().map(|id| title_id(&id)).collect();
    let expected = SyncedWatchState {
        show_ids,
        season_number: Some(1),
        episode_number: Some(2),
        ..synced(&lies)
    };
    assert_eq!(told, [expected]);
}

#[tokio::test]
async fn an_episode_no_library_has_and_a_movie_are_told_without_a_show() {
    let (data, media) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let contents = library_with_dark(data.path(), media.path()).await;
    let gone = played(true, "tmdb-404");
    // A Movie's TMDB ids are a separate id space, so one may equal an Episode's.
    let movie = played(false, &format!("tmdb-{LIES_TMDB_ID}"));

    let told = numbered(&contents, &[gone.clone(), movie.clone()]).await;

    assert_eq!(told, [synced(&gone), synced(&movie)]);
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
