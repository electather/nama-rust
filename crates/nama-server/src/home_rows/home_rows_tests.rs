use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, Mutex},
};

use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    contract::{
        FeaturedSuggestion, FeaturedSuggestions, HomeRow, HomeRowTitle, HomeRows, SharedRowsAsked,
        TitleId, TitleKind, UserFeaturedAsked, UserRowsAsked,
    },
    home_row::{HomeRowMethod, HomeRowSource},
    metadata::ExternalId,
};
use tempfile::TempDir;

use super::{HomeRows as Kept, Kind, OnServer, Row, Title, rows::Rows, shown};
use crate::{
    db::Database,
    events::EventBus,
    library::FoundMovie,
    plugins::Registry,
    requests::Requests,
    server_settings::ServerUrl,
    users::{Permissions, User, replace_personal_settings},
};

pub(super) fn movie(value: &str) -> HomeRowTitle {
    titled(TitleKind::TITLE_KIND_MOVIE, "tmdb", value)
}

pub(super) fn titled(kind: TitleKind, namespace: &str, value: &str) -> HomeRowTitle {
    HomeRowTitle {
        kind: kind.into(),
        id: TitleId {
            namespace: namespace.to_owned(),
            value: value.to_owned(),
            ..TitleId::default()
        }
        .into(),
        ..HomeRowTitle::default()
    }
}

pub(super) fn row(key: &str, ttl_seconds: u32, titles: Vec<HomeRowTitle>) -> HomeRow {
    HomeRow {
        key: key.to_owned(),
        heading: key.to_owned(),
        ttl_seconds,
        titles,
        ..HomeRow::default()
    }
}

pub(super) fn given(rows: Vec<HomeRow>) -> HomeRows {
    HomeRows {
        rows,
        ..HomeRows::default()
    }
}

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
    };
    let titles = vec![
        Title(Kind::Movie, "imdb-tt0133093".to_owned()),
        Title(Kind::Movie, "tmdb-1".to_owned()),
        Title(Kind::Movie, "tmdb-603".to_owned()),
    ];
    let on_server: HashMap<Title, OnServer> = [
        (titles[0].clone(), OnServer::Movie(matrix.clone())),
        (titles[2].clone(), OnServer::Movie(matrix)),
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

/// What a [`Recorded`] source was asked, and a hold on its answers to Users.
#[derive(Default)]
pub(super) struct Calls {
    /// In order: `None` for the shared rows, or the Personal settings a User's call carried.
    pub(super) asked: Mutex<Vec<Option<BTreeMap<String, String>>>>,
    /// In order, the Personal settings each `user_featured` call carried.
    pub(super) featured_asked: Mutex<Vec<BTreeMap<String, String>>>,
    /// In order, the User id each `user_rows` and `user_featured` call carried.
    pub(super) user_ids: Mutex<Vec<i64>>,
    /// It answers only once it holds this, so a test keeps a call in flight.
    pub(super) held: tokio::sync::Mutex<()>,
}

impl Calls {
    pub(super) fn asked(&self) -> Vec<Option<BTreeMap<String, String>>> {
        self.asked.lock().unwrap().clone()
    }
}

/// A Built-in Home row source, `rows`, that records what it is asked, and gives the row `shared`
/// to every User, the row `theirs` to each, and suggests to each User who set a `handle` the
/// Movie of that TMDB id at 0.5, of those `gives` names.
struct Recorded {
    calls: Arc<Calls>,
    gives: &'static [HomeRowMethod],
}

impl Plugin for Recorded {
    fn manifest(&self) -> Manifest {
        Manifest {
            version: "1.0.0".to_owned(),
            extension_points: vec![PointKey::provide::<dyn HomeRowSource>()],
            ..Manifest::new(PluginId::from_static("rows"))
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        let source: Arc<dyn HomeRowSource> = Arc::new(Self {
            calls: Arc::clone(&self.calls),
            gives: self.gives,
        });
        registrar.provide(Priority::DEFAULT, source);
        Ok(())
    }
}

impl HomeRowSource for Recorded {
    fn shared_rows<'a>(
        &'a self,
        _asked: &'a SharedRowsAsked,
    ) -> BoxFuture<'a, Result<HomeRows, PluginError>> {
        self.calls.asked.lock().unwrap().push(None);
        Box::pin(async {
            let _held = self.calls.held.lock().await;
            Ok(given(vec![row("shared", 3600, vec![movie("1")])]))
        })
    }

    fn user_rows<'a>(
        &'a self,
        asked: &'a UserRowsAsked,
    ) -> BoxFuture<'a, Result<HomeRows, PluginError>> {
        let settings = asked.personal_settings.clone().into_iter().collect();
        self.calls.asked.lock().unwrap().push(Some(settings));
        self.calls.user_ids.lock().unwrap().push(asked.user_id);
        Box::pin(async {
            let _held = self.calls.held.lock().await;
            Ok(given(vec![row("theirs", 3600, vec![movie("2")])]))
        })
    }

    fn user_featured<'a>(
        &'a self,
        asked: &'a UserFeaturedAsked,
    ) -> BoxFuture<'a, Result<FeaturedSuggestions, PluginError>> {
        let settings = asked.personal_settings.clone().into_iter().collect();
        self.calls.featured_asked.lock().unwrap().push(settings);
        self.calls.user_ids.lock().unwrap().push(asked.user_id);
        let handle = asked.personal_settings.get("handle").cloned();
        Box::pin(async move {
            let _held = self.calls.held.lock().await;
            let suggestion = |tmdb: String| FeaturedSuggestion {
                kind: TitleKind::TITLE_KIND_MOVIE.into(),
                id: movie(&tmdb).id,
                weight: 0.5,
                ..FeaturedSuggestion::default()
            };
            Ok(FeaturedSuggestions {
                suggestions: handle.into_iter().map(suggestion).collect(),
                ttl_seconds: 3600,
                ..FeaturedSuggestions::default()
            })
        })
    }

    fn gives(&self, method: HomeRowMethod) -> bool {
        self.gives.contains(&method)
    }
}

pub(super) const SHARED: &[HomeRowMethod] = &[HomeRowMethod::Shared];
const BOTH: &[HomeRowMethod] = &[HomeRowMethod::Shared, HomeRowMethod::User];

/// Home rows over a fresh database, with [`Recorded`] enabled, giving the rows `gives` names.
pub(super) async fn recorded(
    gives: &'static [HomeRowMethod],
) -> (TempDir, Arc<Database>, Kept, Arc<Calls>) {
    let data_dir = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data_dir.path()).await.unwrap());
    let (home, calls) = started(&db, gives);
    (data_dir, db, home, calls)
}

/// Home rows over `db`, with [`Recorded`] enabled, giving the rows `gives` names, as a Server
/// starts with that version of it.
pub(super) fn started(db: &Arc<Database>, gives: &'static [HomeRowMethod]) -> (Kept, Arc<Calls>) {
    let calls = Arc::new(Calls::default());
    let plugins = Registry::new();
    let source = Recorded {
        calls: Arc::clone(&calls),
        gives,
    };
    plugins.enable(Arc::new(source)).unwrap();
    let points = plugins.extension_points();
    let events = EventBus::new(Arc::clone(db), points.clone(), ServerUrl::default());
    let requests = Requests::new(Arc::clone(db), events, points.clone());
    (Kept::new(Arc::clone(db), points, requests), calls)
}

pub(super) async fn insert_user(db: &Database, name: &str) -> i64 {
    let name = name.to_owned();
    db.write(move |tx| {
        Box::pin(async move {
            sqlx::query_scalar(
                "INSERT INTO users (username, display_username, permissions, created_at,
                                    updated_at)
                 VALUES (?, ?, 0, 0, 0) RETURNING id",
            )
            .bind(&name)
            .bind(&name)
            .fetch_one(&mut **tx)
            .await
        })
    })
    .await
    .unwrap()
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
