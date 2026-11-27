use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
    time::Duration,
};

use nama_plugin_api::{PluginId, home_row::HomeRowMethod, metadata::ExternalId};
use tempfile::TempDir;

use super::scripted::Calls;
use super::{
    HomeRows as Kept, Kind, OnServer, Row, ScriptedRows, Title, forget_unvisited, rows::Rows, shown,
};
use crate::{
    db::{self, Database},
    library::{FoundMovie, Whereabouts},
    users::{Permissions, User, replace_personal_settings, testing::insert_user},
};

#[test]
fn a_title_a_row_names_by_two_of_its_ids_shows_once_at_its_first_place() {
    let id = |id: &str| -> ExternalId { id.parse().expect("a well-formed id") };
    let matrix = FoundMovie {
        library: 1,
        id: id("tmdb-603"),
        ids: [id("tmdb-603"), id("imdb-tt0133093")].into_iter().collect(),
        title: "The Matrix".to_owned(),
        release_date: None,
        poster: None,
        content_rating: None,
    };
    let titles = vec![
        Title(Kind::Movie, "imdb-tt0133093".to_owned()),
        Title(Kind::Movie, "tmdb-1".to_owned()),
        Title(Kind::Movie, "tmdb-603".to_owned()),
    ];
    let on_server: HashMap<Title, Whereabouts> = [
        (
            titles[0].clone(),
            Whereabouts::In(OnServer::Movie(matrix.clone())),
        ),
        (titles[2].clone(), Whereabouts::In(OnServer::Movie(matrix))),
    ]
    .into_iter()
    .collect();

    let row = Row {
        titles,
        ..Row::default()
    };

    let shown = shown(&row, &on_server, |_| true);

    let places: Vec<i64> = shown.iter().map(|(place, _)| *place).collect();
    assert_eq!(places, [0]);
}

pub(super) const SHARED: &[HomeRowMethod] = &[HomeRowMethod::Shared];
const BOTH: &[HomeRowMethod] = &[HomeRowMethod::Shared, HomeRowMethod::User];
const EVERY: &[HomeRowMethod] = &[
    HomeRowMethod::Shared,
    HomeRowMethod::User,
    HomeRowMethod::Featured,
];

/// Home rows over a fresh database, with the [`ScriptedRows`] of [`scripted`] enabled, giving
/// the answers `gives` names.
pub(super) async fn recorded(
    gives: &'static [HomeRowMethod],
) -> (TempDir, Arc<Database>, Kept, Arc<Calls>) {
    let data_dir = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data_dir.path()).await.unwrap());
    let (home, calls) = started(&db, gives);
    (data_dir, db, home, calls)
}

/// Home rows over `db`, with the [`ScriptedRows`] of [`scripted`] enabled, giving the answers
/// `gives` names, as a Server starts with that version of it.
pub(super) fn started(db: &Arc<Database>, gives: &'static [HomeRowMethod]) -> (Kept, Arc<Calls>) {
    let source = scripted(gives);
    let calls = source.calls();
    (source.home_rows(db), calls)
}

/// The source these tests script: it shares the row `shared` of TMDB Movie 1 and gives each User
/// the row `theirs` of TMDB Movie 2, neither of which is on the Server.
fn scripted(gives: &'static [HomeRowMethod]) -> ScriptedRows {
    ScriptedRows::new(gives)
        .sharing("shared", None, &[1])
        .giving_each("theirs", &[2])
}

pub(super) async fn set_handle(db: &Database, user: i64, handle: &str) {
    let rows = PluginId::from_static("rows");
    let values = vec![("handle".to_owned(), handle.to_owned(), false)];
    replace_personal_settings(db, user, &rows, values)
        .await
        .unwrap();
}

/// The Personal settings of a User who set `handle`.
pub(super) fn handle(handle: &str) -> BTreeMap<String, String> {
    [("handle".to_owned(), handle.to_owned())].into()
}

/// The User `id`, as home reads for them.
fn reader(id: i64) -> User {
    User {
        id,
        name: String::new(),
        permissions: Permissions::NEW_USER,
        created_at: 0,
    }
}

/// Opens home as `user`, and waits for the refreshes it starts to finish.
async fn open_home(home: &Kept, user: i64) {
    home.list(&reader(user)).await.unwrap();
    wait_for_refreshes(home).await;
}

async fn wait_for_refreshes(home: &Kept) {
    home.refreshes.settled().await;
}

/// The keys of the rows stored for `user`, the shared ones first.
async fn stored_keys(home: &Kept, user: i64) -> Vec<String> {
    let stored = Rows(home.clone()).stored(user).await.unwrap();
    stored.into_iter().map(|(_, row)| row.key).collect()
}

#[tokio::test]
async fn a_source_giving_only_users_rows_is_never_asked_for_shared_ones() {
    let (_dir, db, home, calls) = recorded(&[HomeRowMethod::User]).await;
    let ada = insert_user(&db, "ada").await;
    let (came_up, enabled) = tokio::sync::mpsc::unbounded_channel();
    let mut passed_on = home.refresh_as_plugins_come_up(enabled);

    came_up.send(PluginId::from_static("rows")).unwrap();
    passed_on.recv().await.unwrap();
    wait_for_refreshes(&home).await;
    assert!(
        calls.asked().is_empty(),
        "enabling it asks for no shared rows"
    );
    open_home(&home, ada).await;

    assert_eq!(calls.asked(), [Some(BTreeMap::new())]);
    assert_eq!(stored_keys(&home, ada).await, ["theirs"]);
    home.shutdown().await;
}

#[tokio::test]
async fn each_users_rows_are_asked_with_their_id_and_only_their_own_personal_settings() {
    let (_dir, db, home, calls) = recorded(BOTH).await;
    let ada = insert_user(&db, "ada").await;
    let bea = insert_user(&db, "bea").await;
    let carl = insert_user(&db, "carl").await;
    set_handle(&db, ada, "ada").await;
    set_handle(&db, bea, "bea").await;

    open_home(&home, ada).await;
    open_home(&home, bea).await;
    open_home(&home, carl).await;

    assert_eq!(
        calls.asked(),
        [
            None,
            Some(handle("ada")),
            Some(handle("bea")),
            Some(BTreeMap::new())
        ],
        "the shared rows once, then each User with their own settings, none for Carl"
    );
    assert_eq!(*calls.user_ids.lock().unwrap(), [ada, bea, carl]);
}

#[tokio::test]
async fn deleting_a_user_deletes_their_rows() {
    let (_dir, db, home, _) = recorded(BOTH).await;
    let ada = insert_user(&db, "ada").await;
    open_home(&home, ada).await;
    assert_eq!(stored_keys(&home, ada).await, ["shared", "theirs"]);

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

    assert_eq!(stored_keys(&home, ada).await, ["shared"]);
}

#[tokio::test]
async fn forgetting_unvisited_users_deletes_their_own_rows_and_suggestions_until_they_open_home_again()
 {
    let (_dir, db, home, _) = recorded(EVERY).await;
    let ada = insert_user(&db, "ada").await;
    let bea = insert_user(&db, "bea").await;
    for (user, tmdb) in [(ada, "603"), (bea, "604")] {
        set_handle(&db, user, tmdb).await;
    }
    open_home(&home, ada).await;
    home.featured(ada).await.unwrap();
    wait_for_refreshes(&home).await;
    // Lib tests share the clock, so they step it by seconds only; the margin outlasts their steps.
    db::advance_clock(Duration::from_secs(10));
    open_home(&home, bea).await;
    home.featured(bea).await.unwrap();
    wait_for_refreshes(&home).await;

    let forgotten = forget_unvisited(&db, Duration::from_secs(5)).await.unwrap();

    assert_eq!(forgotten, 1, "only Ada had rows to forget");
    assert_eq!(stored_keys(&home, ada).await, ["shared"]);
    assert_eq!(stored_keys(&home, bea).await, ["shared", "theirs"]);
    assert_eq!(home.featured(bea).await.unwrap().len(), 1);
    assert!(home.featured(ada).await.unwrap().is_empty());
    wait_for_refreshes(&home).await;
    open_home(&home, ada).await;
    assert_eq!(stored_keys(&home, ada).await, ["shared", "theirs"]);
    assert_eq!(home.featured(ada).await.unwrap().len(), 1);
}
