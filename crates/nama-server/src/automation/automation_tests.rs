use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::Path,
    sync::Arc,
    time::Duration,
};

use buffa::MessageField;
use nama_plugin_api::{
    automation::{
        FUNCTIONS, Permission,
        functions::{
            DeleteMedia, ListLibraries, ListRequesters, ListTitles, ListUsers, ListWatchStates,
            ListWatchlist, StoreGet,
        },
    },
    contract::{
        self, DeleteAsked, DeleteStarted, LibrariesAsked, LibraryKind, Requesters, RequestersAsked,
        StoreKey, Titles, TitlesAsked, UsersAsked, WatchStates as ListedWatchStates,
        WatchStatesAsked, Watchlist, WatchlistAsked,
    },
    metadata::ExternalId,
    settings::{SettingValue, Settings},
};
use nama_plugin_tmdb::{
    Tmdb,
    testing::{API_KEY, FakeTmdb},
};

use super::{
    permission_name,
    testing::{
        ALIEN_TMDB_ID, DARK_TMDB_ID, LIES_TMDB_ID, SECRETS_TMDB_ID, UNREADABLE, delete,
        episode_ref, kernel, movie_ref, movies, titles_asked, two_episodes_in_one_file,
        two_episodes_in_one_file_also_known_as, users,
    },
};
use crate::{
    db::{self, Database},
    events::EventBus,
    library::{
        Contents, Libraries, Metadata, MovieOrEpisode, Progress, TitleKind, WatchStates,
        fixture::{self, MatchedMovie, tmdb_movie},
    },
    plugins::{ExtensionPoints, Registry},
    requests::Requests,
    server_settings::ServerUrl,
    users::{Permissions, User, Users},
    watchlist,
};

#[tokio::test]
async fn every_function_refuses_a_plugin_not_granted_its_permission_before_reading_the_request() {
    let data = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let (_automation, kernel) = kernel(&db, data.path(), Permission::PERMISSION_UNSPECIFIED);

    for &(function, needs) in FUNCTIONS {
        let refused = kernel.call(function, UNREADABLE.to_vec()).await;

        let needs = permission_name(needs);
        let words = format!("the Admin hasn't granted cleanup the {needs} Permission");
        assert_eq!(refused.unwrap_err().0, words);
    }
}

#[tokio::test]
async fn a_granted_function_refuses_a_request_it_cant_read_and_an_unknown_one_is_refused() {
    let data = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let (_automation, kernel) = kernel(&db, data.path(), Permission::PERMISSION_STORE);

    let unreadable = kernel.call("store_get", UNREADABLE.to_vec()).await;
    let unknown = kernel.call("list_everything", Vec::new()).await;

    let unreadable = unreadable.unwrap_err().0;
    assert_eq!(unreadable, "the Kernel couldn't read the request");
    let unknown = unknown.unwrap_err().0;
    assert_eq!(unknown, "the Kernel has no function list_everything");
}

#[tokio::test]
async fn list_libraries_lists_each_library() {
    let data_dir = tempfile::tempdir().unwrap();
    let media = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data_dir.path()).await.unwrap());
    let id = fixture::movies_library(&db, media.path()).await;
    let (_automation, kernel) = kernel(&db, data_dir.path(), Permission::PERMISSION_READ_LIBRARIES);

    let listed = kernel
        .ask::<ListLibraries>(LibrariesAsked::default())
        .await
        .unwrap();

    let [library] = listed.libraries.as_slice() else {
        panic!("{listed:?}");
    };
    // The fixture names a Library after its folder.
    let folder = media.path().canonicalize().unwrap();
    assert_eq!(
        (library.id, library.name.as_str(), library.kind),
        (
            id,
            folder.to_str().unwrap(),
            LibraryKind::LIBRARY_KIND_MOVIES.into()
        )
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
    let listed = kernel.ask::<ListTitles>(asked).await.unwrap();

    let [title] = listed.titles.as_slice() else {
        panic!("{listed:?}");
    };
    assert_eq!(title.title, "Alien");
    assert_eq!(title.show_title, None);
    assert_eq!(*title.title_ref, movie_ref(alien.library, ALIEN_TMDB_ID));
}

#[tokio::test]
async fn list_titles_gives_genres_content_rating_and_an_episodes_show_id() {
    let data = tempfile::tempdir().unwrap();
    let movies = tempfile::tempdir().unwrap();
    let shows = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let horror = Metadata {
        genres: vec!["Horror".to_owned(), "Science Fiction".to_owned()],
        content_rating: Some("R".to_owned()),
        ..tmdb_movie(ALIEN_TMDB_ID, "Alien")
    };
    let alien = MatchedMovie::new(&db, movies.path(), horror).await;
    let (dark, _path) = two_episodes_in_one_file(&db, shows.path()).await;
    let (_automation, kernel) = kernel(&db, data.path(), Permission::PERMISSION_READ_LIBRARIES);

    let mut listed = Vec::new();
    for library in [alien.library, dark] {
        let page = kernel.ask::<ListTitles>(titles_asked(library, 0, "")).await;
        listed.extend(page.unwrap().titles.into_iter().map(|title| {
            let show = title.show_id.as_option().map(|id| id.value.clone());
            (title.genres, title.content_rating, show)
        }));
    }

    let dark_episode = (
        vec!["Drama".to_owned()],
        Some("TV-MA".to_owned()),
        Some(DARK_TMDB_ID.to_string()),
    );
    let alien_movie = (
        vec!["Horror".to_owned(), "Science Fiction".to_owned()],
        Some("R".to_owned()),
        None,
    );
    assert_eq!(listed, [alien_movie, dark_episode.clone(), dark_episode]);
}

#[tokio::test]
async fn list_titles_gives_every_id_of_a_movie_matched_under_tmdb_and_tvdb() {
    let data = tempfile::tempdir().unwrap();
    let media = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let mut alien = tmdb_movie(ALIEN_TMDB_ID, "Alien");
    alien.ids.insert(ExternalId::tvdb(1));
    let alien = MatchedMovie::new(&db, media.path(), alien).await;
    let (_automation, kernel) = kernel(&db, data.path(), Permission::PERMISSION_READ_LIBRARIES);

    let listed = kernel
        .ask::<ListTitles>(titles_asked(alien.library, 0, ""))
        .await;

    let [title] = listed.unwrap().titles.try_into().unwrap();
    let ids: Vec<_> = (title.ids.iter())
        .map(|id| format!("{}-{}", id.namespace, id.value))
        .collect();
    assert_eq!(ids, [format!("tmdb-{ALIEN_TMDB_ID}"), "tvdb-1".to_owned()]);
    assert_eq!(title.show_ids, []);
}

#[tokio::test]
async fn list_titles_gives_when_each_movie_and_episode_was_added() {
    let data = tempfile::tempdir().unwrap();
    let movies = tempfile::tempdir().unwrap();
    let shows = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let before = db::now() * 1000;
    let alien = MatchedMovie::new(&db, movies.path(), tmdb_movie(ALIEN_TMDB_ID, "Alien")).await;
    let (dark, _path) = two_episodes_in_one_file(&db, shows.path()).await;
    let after = db::now_ms();
    let (_automation, kernel) = kernel(&db, data.path(), Permission::PERMISSION_READ_LIBRARIES);

    let mut added = Vec::new();
    for library in [alien.library, dark] {
        let listed = kernel.ask::<ListTitles>(titles_asked(library, 0, "")).await;
        added.extend(listed.unwrap().titles.iter().map(|title| title.added_at_ms));
    }

    assert_eq!(added.len(), 3, "{added:?}");
    assert!(
        added.iter().all(|at| (before..=after).contains(at)),
        "{added:?} not in {before}..={after}"
    );
}

#[tokio::test]
async fn list_titles_pages_cover_every_title_once_in_order_and_the_last_has_no_token() {
    let data = tempfile::tempdir().unwrap();
    let media = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let library = movies(&db, media.path(), &[3, 1, 5, 2, 4]).await;
    let (_automation, kernel) = kernel(&db, data.path(), Permission::PERMISSION_READ_LIBRARIES);

    let mut pages = Vec::new();
    let mut token = String::new();
    loop {
        let page = kernel
            .ask::<ListTitles>(titles_asked(library, 2, &token))
            .await;
        let page = page.unwrap();
        let ids: Vec<_> = (page.titles.iter())
            .map(|title| title.title_ref.id.value.clone())
            .collect();
        pages.push(ids);
        token = page.next_page_token;
        if token.is_empty() {
            break;
        }
    }

    assert_eq!(pages, [vec!["1", "2"], vec!["3", "4"], vec!["5"]]);
}

#[tokio::test]
async fn a_title_matched_between_two_pages_neither_repeats_nor_skips_a_listed_one() {
    let data = tempfile::tempdir().unwrap();
    let media = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let library = movies(&db, media.path(), &[2, 3, 5, 6]).await;
    let (_automation, kernel) = kernel(&db, data.path(), Permission::PERMISSION_READ_LIBRARIES);
    let ids = |page: &Titles| -> Vec<String> {
        (page.titles.iter())
            .map(|title| title.title_ref.id.value.clone())
            .collect()
    };

    let first = kernel.ask::<ListTitles>(titles_asked(library, 2, "")).await;
    let first = first.unwrap();
    let path = media.path().canonicalize().unwrap().join("1.mkv");
    let contents = Contents::new(Arc::clone(&db));
    MatchedMovie::store(
        &contents,
        library,
        path.to_str().unwrap(),
        tmdb_movie(1, "Movie 1"),
    )
    .await;
    let asked = titles_asked(library, 2, &first.next_page_token);
    let second = kernel.ask::<ListTitles>(asked).await.unwrap();

    assert_eq!(ids(&first), ["2", "3"]);
    assert_eq!(ids(&second), ["5", "6"]);
    assert_eq!(second.next_page_token, "");
}

#[tokio::test]
async fn list_titles_with_page_size_0_gives_every_title_without_a_token() {
    let data = tempfile::tempdir().unwrap();
    let media = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let library = movies(&db, media.path(), &[2, 1, 3]).await;
    let (_automation, kernel) = kernel(&db, data.path(), Permission::PERMISSION_READ_LIBRARIES);

    let listed = kernel.ask::<ListTitles>(titles_asked(library, 0, "")).await;

    let listed = listed.unwrap();
    let ids: Vec<_> = (listed.titles.iter())
        .map(|title| title.title_ref.id.value.as_str())
        .collect();
    assert_eq!(ids, ["1", "2", "3"]);
    assert_eq!(listed.next_page_token, "");
}

#[tokio::test]
async fn list_titles_refuses_a_page_token_it_didnt_hand_out() {
    let data = tempfile::tempdir().unwrap();
    let media = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let library = movies(&db, media.path(), &[1]).await;
    let (_automation, kernel) = kernel(&db, data.path(), Permission::PERMISSION_READ_LIBRARIES);

    let refused = kernel
        .ask::<ListTitles>(titles_asked(library, 2, "next"))
        .await;

    assert_eq!(
        refused.unwrap_err().0,
        r#"the page token "next" isn't one the Kernel handed out"#
    );
}

#[tokio::test]
async fn list_watch_states_gives_each_users_watch_state_of_each_title() {
    let data = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let [ada, bo] = users(&db, ["Ada", "Bo"]).await;
    let watch_states = WatchStates::new(Arc::clone(&db), ExtensionPoints::default());
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
    let listed: ListedWatchStates = kernel.ask::<ListWatchStates>(asked).await.unwrap();

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
async fn list_watch_states_for_one_user_gives_no_other_users_watch_state() {
    let data = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let [ada, bo] = users(&db, ["Ada", "Bo"]).await;
    let alien = MovieOrEpisode::movie(&ExternalId::tmdb(ALIEN_TMDB_ID));
    let watch_states = WatchStates::new(Arc::clone(&db), ExtensionPoints::default());
    for user in [ada, bo] {
        watch_states
            .mark(user, vec![alien.clone()], true)
            .await
            .unwrap();
    }
    let (_automation, kernel) = kernel(&db, data.path(), Permission::PERMISSION_READ_WATCH_STATES);

    let asked = WatchStatesAsked {
        titles: vec![movie_ref(1, ALIEN_TMDB_ID)],
        user_id: Some(bo),
        ..WatchStatesAsked::default()
    };
    let listed = kernel.ask::<ListWatchStates>(asked).await.unwrap();

    assert_eq!(users_played(&listed), [(bo, true)]);
}

/// The Users and what they did of each Watch State `listed` gives of its one title.
fn users_played(listed: &ListedWatchStates) -> Vec<(i64, bool)> {
    let [title] = listed.titles.as_slice() else {
        panic!("{listed:?}");
    };
    (title.states.iter())
        .map(|state| (state.user_id, state.played))
        .collect()
}

#[tokio::test]
async fn list_watch_states_finds_a_movies_watch_state_kept_under_another_of_its_ids() {
    let data = tempfile::tempdir().unwrap();
    let media = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let [ada] = users(&db, ["Ada"]).await;
    let imdb = ExternalId::new("imdb", "tt0078748").unwrap();
    let mut matched = tmdb_movie(ALIEN_TMDB_ID, "Alien");
    matched.ids.insert(imdb.clone());
    let alien = MatchedMovie::new(&db, media.path(), matched).await;
    let under_imdb = MovieOrEpisode::movie(&imdb);
    (WatchStates::new(Arc::clone(&db), ExtensionPoints::default()).mark(
        ada,
        vec![under_imdb],
        true,
    ))
    .await
    .unwrap();
    let (_automation, kernel) = kernel(&db, data.path(), Permission::PERMISSION_READ_WATCH_STATES);

    let asked = WatchStatesAsked {
        titles: vec![movie_ref(alien.library, ALIEN_TMDB_ID)],
        ..WatchStatesAsked::default()
    };
    let listed: ListedWatchStates = kernel.ask::<ListWatchStates>(asked).await.unwrap();

    assert_eq!(users_played(&listed), [(ada, true)]);
}

#[tokio::test]
async fn list_watch_states_finds_an_episodes_watch_state_kept_under_another_of_its_ids() {
    let data = tempfile::tempdir().unwrap();
    let media = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let [ada] = users(&db, ["Ada"]).await;
    let imdb = |tmdb| ExternalId::new("imdb", format!("tt{tmdb}")).unwrap();
    let (library, _path) =
        two_episodes_in_one_file_also_known_as(&db, media.path(), |tmdb| Some(imdb(tmdb))).await;
    let under_imdb = MovieOrEpisode::episode(&imdb(LIES_TMDB_ID));
    (WatchStates::new(Arc::clone(&db), ExtensionPoints::default()).mark(
        ada,
        vec![under_imdb],
        true,
    ))
    .await
    .unwrap();
    let (_automation, kernel) = kernel(&db, data.path(), Permission::PERMISSION_READ_WATCH_STATES);

    let asked = WatchStatesAsked {
        titles: vec![episode_ref(library, LIES_TMDB_ID)],
        ..WatchStatesAsked::default()
    };
    let listed: ListedWatchStates = kernel.ask::<ListWatchStates>(asked).await.unwrap();

    assert_eq!(users_played(&listed), [(ada, true)]);
}

#[tokio::test]
async fn list_users_gives_every_user_with_their_id_and_display_name() {
    let data = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let [ada, bo] = users(&db, ["Ada", "Bo"]).await;
    let alien = MovieOrEpisode::movie(&ExternalId::tmdb(ALIEN_TMDB_ID));
    (WatchStates::new(Arc::clone(&db), ExtensionPoints::default()).mark(ada, vec![alien], true))
        .await
        .unwrap();
    let (_automation, kernel) = kernel(&db, data.path(), Permission::PERMISSION_READ_USERS);

    let listed = kernel
        .ask::<ListUsers>(UsersAsked::default())
        .await
        .unwrap();

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

    let refused = delete(&kernel, movie_ref(library, ALIEN_TMDB_ID)).await;

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

    let refused = delete(&kernel, movie_ref(alien.library, ALIEN_TMDB_ID)).await;

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

    delete(&kernel, movie_ref(alien.library, ALIEN_TMDB_ID))
        .await
        .unwrap();

    let contents = Contents::new(Arc::clone(&db));
    tokio::time::timeout(Duration::from_secs(10), async {
        while !contents
            .movies(alien.library, None)
            .await
            .unwrap()
            .is_empty()
        {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the Movie's Versions were never removed");
    assert!(!file.exists());
}

#[tokio::test(flavor = "current_thread")]
async fn shutting_down_right_after_delete_media_waits_for_the_delete() {
    let data = tempfile::tempdir().unwrap();
    let media = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let alien = MatchedMovie::new(&db, media.path(), tmdb_movie(ALIEN_TMDB_ID, "Alien")).await;
    let file = media.path().join("Alien.mkv");
    fs::write(&file, "alien").unwrap();
    let (automation, kernel) = kernel(&db, data.path(), Permission::PERMISSION_DELETE_MEDIA);

    delete(&kernel, movie_ref(alien.library, ALIEN_TMDB_ID))
        .await
        .unwrap();
    automation.scanner.shutdown().await;

    assert!(!file.exists());
}

#[tokio::test]
async fn delete_media_refuses_an_episode_whose_file_holds_one_not_named_and_deletes_nothing() {
    let data = tempfile::tempdir().unwrap();
    let media = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let (library, path) = two_episodes_in_one_file(&db, media.path()).await;
    let (_automation, kernel) = kernel(&db, data.path(), Permission::PERMISSION_DELETE_MEDIA);

    let refused = delete(&kernel, episode_ref(library, SECRETS_TMDB_ID)).await;

    assert_eq!(
        refused.unwrap_err().0,
        "the title's files also hold Dark S01E02 (Lies); name each in `also` to delete it too, \
         or keep the files"
    );
    let shows = Contents::new(Arc::clone(&db))
        .shows(library, None)
        .await
        .unwrap();
    assert_eq!(shows[0].episodes().count(), 2);
    assert!(Path::new(&path).exists());
}

#[tokio::test]
async fn delete_media_of_an_episode_naming_the_other_in_also_starts_and_gives_it() {
    let data = tempfile::tempdir().unwrap();
    let media = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let (library, path) = two_episodes_in_one_file(&db, media.path()).await;
    let (_automation, kernel) = kernel(&db, data.path(), Permission::PERMISSION_DELETE_MEDIA);

    let asked = DeleteAsked {
        title_ref: MessageField::some(episode_ref(library, SECRETS_TMDB_ID)),
        also: vec![episode_ref(library, LIES_TMDB_ID)],
        ..DeleteAsked::default()
    };
    let started: DeleteStarted = kernel.ask::<DeleteMedia>(asked).await.unwrap();

    assert_eq!(started.taken_along, [episode_ref(library, LIES_TMDB_ID)]);
    tokio::time::timeout(Duration::from_secs(10), async {
        while Path::new(&path).exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the file was never deleted");
}

#[tokio::test]
async fn a_function_refuses_once_the_server_is_stopping() {
    let data = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let (automation, kernel) = kernel(&db, data.path(), Permission::PERMISSION_STORE);

    drop(automation);

    let refused = kernel.ask::<StoreGet>(StoreKey::default()).await;
    assert_eq!(refused.unwrap_err().0, "the Server is stopping");
}

/// Requests on `db` looking titles up in `tmdb`, and the Plugins they need kept enabled.
fn requests(db: &Arc<Database>, tmdb: &FakeTmdb) -> (Registry, Requests) {
    let plugins = Registry::new();
    let settings = Settings::new(BTreeMap::from([(
        "api_key".to_owned(),
        SettingValue::Text(API_KEY.to_owned()),
    )]));
    let tmdb = Tmdb::with_endpoints(tmdb.endpoints());
    (plugins.enable_with_settings(Arc::new(tmdb), settings)).unwrap();
    let points = plugins.extension_points();
    let events = EventBus::new(Arc::clone(db), points.clone(), ServerUrl::default());
    (plugins, Requests::new(Arc::clone(db), events, points))
}

/// The User `id`, as Requests sees them asking.
fn asker(id: i64) -> User {
    User {
        id,
        name: String::new(),
        permissions: Permissions::NEW_USER,
        created_at: 0,
    }
}

/// Whether each title `listed` gives was Requested, and by whom.
fn requested(listed: &Requesters) -> Vec<(bool, Vec<i64>)> {
    (listed.titles.iter())
        .map(|title| (title.requested, title.user_ids.clone()))
        .collect()
}

#[tokio::test]
async fn list_requesters_gives_who_requested_each_movie_and_episode() {
    let data = tempfile::tempdir().unwrap();
    let movie_folder = tempfile::tempdir().unwrap();
    let show_folder = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let [ada, bo] = users(&db, ["Ada", "Bo"]).await;
    let tmdb = FakeTmdb::start().await;
    let (_plugins, requests) = requests(&db, &tmdb);
    let alien = ExternalId::tmdb(ALIEN_TMDB_ID);
    requests.create(&asker(ada), &alien, None).await.unwrap();
    requests.create(&asker(bo), &alien, None).await.unwrap();
    let dark = ExternalId::tmdb(DARK_TMDB_ID);
    requests.follow(&asker(bo), &dark, None).await.unwrap();
    let movies = movies(&db, movie_folder.path(), &[ALIEN_TMDB_ID, 1]).await;
    let (shows, _path) = two_episodes_in_one_file(&db, show_folder.path()).await;
    let (_automation, kernel) = kernel(&db, data.path(), Permission::PERMISSION_READ_REQUESTS);

    let asked = RequestersAsked {
        titles: vec![
            movie_ref(movies, ALIEN_TMDB_ID),
            movie_ref(movies, 1),
            episode_ref(shows, LIES_TMDB_ID),
        ],
        ..RequestersAsked::default()
    };
    let listed = kernel.ask::<ListRequesters>(asked).await.unwrap();

    assert_eq!(
        requested(&listed),
        [(true, vec![ada, bo]), (false, vec![]), (true, vec![bo])]
    );
    assert_eq!(
        *listed.titles[2].title_ref,
        episode_ref(shows, LIES_TMDB_ID)
    );
}

#[tokio::test]
async fn list_requesters_gives_a_title_whose_only_requester_was_deleted_as_requested() {
    let data = tempfile::tempdir().unwrap();
    let media = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let [admin, ada] = users(&db, ["Admin", "Ada"]).await;
    let tmdb = FakeTmdb::start().await;
    let (_plugins, requests) = requests(&db, &tmdb);
    let alien = ExternalId::tmdb(ALIEN_TMDB_ID);
    requests.create(&asker(ada), &alien, None).await.unwrap();
    let library = movies(&db, media.path(), &[ALIEN_TMDB_ID]).await;
    let root = User {
        permissions: Permissions::ADMIN,
        ..asker(0)
    };
    let users = Users::new(Arc::clone(&db), ExtensionPoints::default());
    // An Admin must be left once Ada goes.
    (users.set_permissions(&root, admin, Permissions::ADMIN))
        .await
        .unwrap();
    users.delete(&root, ada).await.unwrap();
    let (_automation, kernel) = kernel(&db, data.path(), Permission::PERMISSION_READ_REQUESTS);

    let asked = RequestersAsked {
        titles: vec![movie_ref(library, ALIEN_TMDB_ID)],
        ..RequestersAsked::default()
    };
    let listed = kernel.ask::<ListRequesters>(asked).await.unwrap();

    assert_eq!(requested(&listed), [(true, vec![])]);
}

#[tokio::test]
async fn list_watchlist_gives_that_users_watchlist_only() {
    let data = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let [ada, bo] = users(&db, ["Ada", "Bo"]).await;
    let (dark, alien) = (
        ExternalId::tmdb(DARK_TMDB_ID),
        ExternalId::tmdb(ALIEN_TMDB_ID),
    );
    watchlist::add(&db, ada, TitleKind::Show, &dark)
        .await
        .unwrap();
    watchlist::add(&db, ada, TitleKind::Movie, &alien)
        .await
        .unwrap();
    watchlist::add(&db, bo, TitleKind::Movie, &ExternalId::tmdb(1))
        .await
        .unwrap();
    let (_automation, kernel) = kernel(&db, data.path(), Permission::PERMISSION_READ_WATCHLISTS);

    let asked = WatchlistAsked {
        user_id: ada,
        ..WatchlistAsked::default()
    };
    let listed: Watchlist = kernel.ask::<ListWatchlist>(asked).await.unwrap();

    let listed: HashSet<_> = (listed.titles.iter())
        .map(|title| (title.kind, title.id.value.clone()))
        .collect();
    let show = contract::TitleKind::TITLE_KIND_SHOW.into();
    let movie = contract::TitleKind::TITLE_KIND_MOVIE.into();
    let expected = HashSet::from([
        (show, DARK_TMDB_ID.to_string()),
        (movie, ALIEN_TMDB_ID.to_string()),
    ]);
    assert_eq!(listed, expected);
}
