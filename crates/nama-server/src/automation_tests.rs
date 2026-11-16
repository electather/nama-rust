use std::{collections::HashSet, fs, path::Path, sync::Arc, time::Duration};

use buffa::MessageField;
use nama_plugin_api::{
    PluginId,
    automation::{
        DeleteAsked, Kernel, LibrariesAsked, LibraryKind, Permission, StoreKey, TitleRef,
        TitleRefKind, TitlesAsked, UsersAsked, WatchStatesAsked,
    },
    fulfiller::TitleId,
    metadata::ExternalId,
};

use super::Automation;
use crate::{
    db::Database,
    events::EventBus,
    images::ImageStore,
    library::{
        artwork::Artwork,
        contents::{
            Contents,
            fixture::{self, MatchedMovie, tmdb_movie},
        },
        custom_images::CustomImages,
        libraries::Libraries,
        watch_states::{MovieOrEpisode, Progress, WatchStates},
    },
    plugins::{ExtensionPoints, Registry},
    scanner::Scanner,
    server_settings::ServerUrl,
    users::{Permissions, User, Users},
};

const ALIEN_TMDB_ID: i64 = 348;

/// The Automation of a Server on `db`, whose Scanner deletes and publishes nowhere.
fn automation(db: &Arc<Database>, data: &Path) -> Arc<Automation> {
    let points = ExtensionPoints::default();
    let images = ImageStore::new(data, Arc::clone(db));
    let scanner = Scanner::new(
        Arc::clone(db),
        points.clone(),
        Artwork::new(images.clone(), points.clone()),
        CustomImages::new(Arc::clone(db), images),
        EventBus::new(Arc::clone(db), points.clone(), ServerUrl::default()),
        None,
        None,
        None,
    );
    Automation::serve(Arc::clone(db), scanner, points)
}

/// The Kernel's functions for the cleanup Plugin on `db`, granted only `permission`, and the
/// Automation they need kept alive.
fn kernel(
    db: &Arc<Database>,
    data: &Path,
    permission: Permission,
) -> (Arc<Automation>, Arc<dyn Kernel>) {
    let automation = automation(db, data);
    let kernel = Automation::kernel(
        &Arc::downgrade(&automation),
        PluginId::from_static("cleanup"),
        HashSet::from([permission]),
    );
    (automation, kernel)
}

/// The Movie `tmdb` of the Library `library`, as a Plugin names it.
fn movie_ref(library: i64, tmdb: i64) -> TitleRef {
    TitleRef {
        library_id: library,
        kind: TitleRefKind::TITLE_REF_KIND_MOVIE.into(),
        id: MessageField::some(TitleId {
            namespace: "tmdb".to_owned(),
            value: tmdb.to_string(),
            ..TitleId::default()
        }),
        ..TitleRef::default()
    }
}

/// Creates a User for each of `names` on `db`, and gives their ids.
async fn users<const N: usize>(db: &Arc<Database>, names: [&str; N]) -> [i64; N] {
    let root = User {
        id: 0,
        name: "Root".to_owned(),
        permissions: Permissions::ADMIN,
        created_at: 0,
    };
    let plugins = Registry::new();
    plugins
        .enable(Arc::new(nama_plugin_local_auth::LocalAuth))
        .unwrap();
    let users = Users::new(Arc::clone(db), plugins.extension_points());
    let mut ids = [0; N];
    for (id, name) in ids.iter_mut().zip(names) {
        *id = (users.create(&root, name, "correct horse battery staple"))
            .await
            .unwrap();
    }
    ids
}

fn delete(title: TitleRef) -> DeleteAsked {
    DeleteAsked {
        title_ref: MessageField::some(title),
        ..DeleteAsked::default()
    }
}

#[tokio::test]
async fn a_function_without_its_permission_is_refused_in_plain_words() {
    let data_dir = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data_dir.path()).await.unwrap());
    let (_automation, kernel) = kernel(&db, data_dir.path(), Permission::PERMISSION_STORE);

    let deleted = kernel.delete_media(&DeleteAsked::default()).await;
    let listed = kernel.list_libraries(&LibrariesAsked::default()).await;

    assert_eq!(
        deleted.unwrap_err().0,
        "the Admin hasn't granted cleanup the Delete media Permission"
    );
    assert_eq!(
        listed.unwrap_err().0,
        "the Admin hasn't granted cleanup the Read Libraries Permission"
    );
    kernel.store_get(&StoreKey::default()).await.unwrap();
}

#[tokio::test]
async fn list_libraries_lists_each_library() {
    let data_dir = tempfile::tempdir().unwrap();
    let media = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data_dir.path()).await.unwrap());
    let id = fixture::movies_library(&db, media.path()).await;
    let (_automation, kernel) = kernel(&db, data_dir.path(), Permission::PERMISSION_READ_LIBRARIES);

    let listed = kernel
        .list_libraries(&LibrariesAsked::default())
        .await
        .unwrap();

    let [library] = listed.libraries.as_slice() else {
        panic!("{listed:?}");
    };
    assert_eq!(
        (library.id, library.name.as_str(), library.kind),
        (id, "Movies", LibraryKind::LIBRARY_KIND_MOVIES.into())
    );
}

#[tokio::test]
async fn list_titles_gives_each_movie_with_the_id_its_watch_state_uses() {
    let data = tempfile::tempdir().unwrap();
    let media = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let alien = MatchedMovie::new(&db, media.path(), tmdb_movie(ALIEN_TMDB_ID, "Alien")).await;
    let (_automation, kernel) = kernel(&db, data.path(), Permission::PERMISSION_READ_LIBRARIES);

    let asked = TitlesAsked {
        library_id: alien.library,
        ..TitlesAsked::default()
    };
    let listed = kernel.list_titles(&asked).await.unwrap();

    let [title] = listed.titles.as_slice() else {
        panic!("{listed:?}");
    };
    assert_eq!(title.title, "Alien");
    assert_eq!(title.show_title, None);
    assert_eq!(*title.title_ref, movie_ref(alien.library, ALIEN_TMDB_ID));
}

#[tokio::test]
async fn list_watch_states_gives_each_users_watch_state_of_each_title() {
    let data = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let [ada, bo] = users(&db, ["Ada", "Bo"]).await;
    let watch_states = WatchStates::new(Arc::clone(&db));
    let alien = MovieOrEpisode::movie(&ExternalId::tmdb(ALIEN_TMDB_ID));
    let halfway = Progress::new(500, 1000).unwrap();
    (watch_states.report(ada, vec![Some(alien.clone())], halfway))
        .await
        .unwrap();
    watch_states.mark(bo, vec![alien], true).await.unwrap();
    let (_automation, kernel) = kernel(&db, data.path(), Permission::PERMISSION_READ_WATCH_STATES);

    let asked = WatchStatesAsked {
        titles: vec![movie_ref(1, ALIEN_TMDB_ID), movie_ref(1, 1)],
        ..WatchStatesAsked::default()
    };
    let listed = kernel.list_watch_states(&asked).await.unwrap();

    let [watched, unwatched] = listed.titles.as_slice() else {
        panic!("{listed:?}");
    };
    assert_eq!(*watched.title_ref, movie_ref(1, ALIEN_TMDB_ID));
    let states: Vec<_> = (watched.states.iter())
        .map(|state| {
            (
                state.user_id,
                state.played,
                state.position_ms,
                state.duration_ms,
            )
        })
        .collect();
    assert_eq!(states, [(ada, false, 500, 1000), (bo, true, 0, 0)]);
    assert!(watched.states.iter().all(|state| state.updated_at_ms > 0));
    assert_eq!(unwatched.states, []);
}

#[tokio::test]
async fn list_users_without_its_permission_is_refused_naming_it() {
    let data = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let (_automation, kernel) = kernel(&db, data.path(), Permission::PERMISSION_READ_WATCH_STATES);

    let refused = kernel.list_users(&UsersAsked::default()).await;

    assert_eq!(
        refused.unwrap_err().0,
        "the Admin hasn't granted cleanup the Read Users Permission"
    );
}

#[tokio::test]
async fn list_users_gives_every_user_with_their_id_and_display_name() {
    let data = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let [ada, bo] = users(&db, ["Ada", "Bo"]).await;
    let alien = MovieOrEpisode::movie(&ExternalId::tmdb(ALIEN_TMDB_ID));
    (WatchStates::new(Arc::clone(&db)).mark(ada, vec![alien], true))
        .await
        .unwrap();
    let (_automation, kernel) = kernel(&db, data.path(), Permission::PERMISSION_READ_USERS);

    let listed = kernel.list_users(&UsersAsked::default()).await.unwrap();

    let users: Vec<_> = (listed.users.iter())
        .map(|user| (user.id, user.display_name.as_str()))
        .collect();
    assert_eq!(users, [(ada, "Ada"), (bo, "Bo")]);
}

#[tokio::test]
async fn delete_media_refuses_a_title_the_library_doesnt_have() {
    let data = tempfile::tempdir().unwrap();
    let media = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let library = fixture::movies_library(&db, media.path()).await;
    let (_automation, kernel) = kernel(&db, data.path(), Permission::PERMISSION_DELETE_MEDIA);

    let refused = kernel
        .delete_media(&delete(movie_ref(library, ALIEN_TMDB_ID)))
        .await;

    assert_eq!(
        refused.unwrap_err().0,
        "nothing in the Library is Matched to that id"
    );
}

#[tokio::test]
async fn delete_media_refuses_media_in_a_read_only_folder() {
    let data = tempfile::tempdir().unwrap();
    let media = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let alien = MatchedMovie::new(&db, media.path(), tmdb_movie(ALIEN_TMDB_ID, "Alien")).await;
    let folder = media.path().canonicalize().unwrap();
    let folder = folder.to_str().unwrap().to_owned();
    (Libraries::new(Arc::clone(&db)).set_read_only(alien.library, vec![folder.clone()]))
        .await
        .unwrap();
    let (_automation, kernel) = kernel(&db, data.path(), Permission::PERMISSION_DELETE_MEDIA);

    let refused = kernel
        .delete_media(&delete(movie_ref(alien.library, ALIEN_TMDB_ID)))
        .await;

    assert_eq!(
        refused.unwrap_err().0,
        format!("media in {folder} can't be deleted: the Server can't write to it")
    );
}

#[tokio::test]
async fn delete_media_removes_the_movies_versions_in_the_background() {
    let data = tempfile::tempdir().unwrap();
    let media = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let alien = MatchedMovie::new(&db, media.path(), tmdb_movie(ALIEN_TMDB_ID, "Alien")).await;
    let file = media.path().join("Alien.mkv");
    fs::write(&file, "alien").unwrap();
    let (_automation, kernel) = kernel(&db, data.path(), Permission::PERMISSION_DELETE_MEDIA);

    kernel
        .delete_media(&delete(movie_ref(alien.library, ALIEN_TMDB_ID)))
        .await
        .unwrap();

    let contents = Contents::new(Arc::clone(&db));
    tokio::time::timeout(Duration::from_secs(10), async {
        while !contents.movies(alien.library).await.unwrap().is_empty() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the Movie's Versions were never removed");
    assert!(!file.exists());
}

#[tokio::test]
async fn a_function_refuses_once_the_server_is_stopping() {
    let data = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let (automation, kernel) = kernel(&db, data.path(), Permission::PERMISSION_STORE);

    drop(automation);

    let refused = kernel.store_get(&StoreKey::default()).await;
    assert_eq!(refused.unwrap_err().0, "the Server is stopping");
}
