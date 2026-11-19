use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    contract::{
        HomeRow, HomeRowTitle, HomeRows, SharedRowsAsked, TitleId, TitleKind, UserRowsAsked,
    },
    home_row::{HomeRowMethod, HomeRowSource},
    metadata::ExternalId,
};
use tempfile::TempDir;

use super::{HomeRows as Kept, Kind, OnServer, Title, kept, lock, shown};
use crate::{
    db::Database, library::FoundMovie, plugins::Registry, users::replace_personal_settings,
};

fn movie(value: &str) -> HomeRowTitle {
    titled(TitleKind::TITLE_KIND_MOVIE, "tmdb", value)
}

fn titled(kind: TitleKind, namespace: &str, value: &str) -> HomeRowTitle {
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

fn row(key: &str, ttl_seconds: u32, titles: Vec<HomeRowTitle>) -> HomeRow {
    HomeRow {
        key: key.to_owned(),
        heading: key.to_owned(),
        ttl_seconds,
        titles,
        ..HomeRow::default()
    }
}

fn given(rows: Vec<HomeRow>) -> HomeRows {
    HomeRows {
        rows,
        ..HomeRows::default()
    }
}

#[test]
fn rows_past_three_titles_past_200_and_headings_past_40_characters_are_cut() {
    let many = (1..=250).map(|n| movie(&n.to_string())).collect();
    let long = HomeRow {
        heading: "Ü".repeat(45),
        ..row("long", 3600, many)
    };
    let rows = vec![
        long,
        row("b", 3600, vec![]),
        row("c", 3600, vec![]),
        row("d", 3600, vec![]),
    ];

    let (kept, _) = kept(given(rows));

    let keys: Vec<&str> = kept.iter().map(|row| row.key.as_str()).collect();
    assert_eq!(keys, ["long", "b", "c"]);
    assert_eq!(kept[0].heading, "Ü".repeat(40));
    assert_eq!(kept[0].titles.len(), 200);
    assert_eq!(
        kept[0].titles[199],
        Title(Kind::Movie, "tmdb-200".to_owned())
    );
}

#[test]
fn a_time_to_live_is_clamped_and_the_rows_are_fresh_for_the_shortest() {
    let (_, too_short) = kept(given(vec![row("a", 60, vec![]), row("b", 3600, vec![])]));
    let (_, too_long) = kept(given(vec![row("a", 30 * 24 * 3600, vec![])]));
    let (_, within) = kept(given(vec![row("a", 7200, vec![]), row("b", 3600, vec![])]));

    assert_eq!(too_short, Duration::from_mins(15));
    assert_eq!(too_long, Duration::from_hours(7 * 24));
    assert_eq!(within, Duration::from_hours(1));
}

#[test]
fn a_plugin_giving_no_rows_is_asked_again_in_15_minutes() {
    let (rows, fresh_for) = kept(given(vec![]));

    assert_eq!(rows, []);
    assert_eq!(fresh_for, Duration::from_mins(15));
}

#[test]
fn a_repeated_key_and_titles_without_a_kind_or_a_well_formed_id_are_dropped() {
    let titles = vec![
        titled(TitleKind::TITLE_KIND_SHOW, "tvdb", "1"),
        titled(TitleKind::TITLE_KIND_UNSPECIFIED, "tmdb", "2"),
        titled(TitleKind::TITLE_KIND_MOVIE, "TMDB", "3"),
        titled(TitleKind::TITLE_KIND_MOVIE, "tmdb", ""),
    ];
    let rows = vec![row("a", 3600, titles), row("a", 3600, vec![movie("4")])];

    let (kept, _) = kept(given(rows));

    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].titles, [Title(Kind::Show, "tvdb-1".to_owned())]);
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
    let titles = [
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

    let shown = shown(&titles, on_server);

    let places: Vec<i64> = shown.iter().map(|(place, _)| *place).collect();
    assert_eq!(places, [0]);
}

/// What a [`Recorded`] source was asked, in order: `None` for the shared rows, or the Personal
/// settings a User's call carried.
type Asked = Arc<Mutex<Vec<Option<BTreeMap<String, String>>>>>;

/// A Built-in Home row source, `rows`, that records what it is asked, gives the row `shared` to
/// every User and, if it gives Users' rows, the row `theirs` to each.
struct Recorded {
    asked: Asked,
    gives_user_rows: bool,
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
            asked: Arc::clone(&self.asked),
            gives_user_rows: self.gives_user_rows,
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
        self.asked.lock().unwrap().push(None);
        Box::pin(async { Ok(given(vec![row("shared", 3600, vec![movie("1")])])) })
    }

    fn user_rows<'a>(
        &'a self,
        asked: &'a UserRowsAsked,
    ) -> BoxFuture<'a, Result<HomeRows, PluginError>> {
        let settings = asked.personal_settings.clone().into_iter().collect();
        self.asked.lock().unwrap().push(Some(settings));
        Box::pin(async { Ok(given(vec![row("theirs", 3600, vec![movie("2")])])) })
    }

    fn gives(&self, method: HomeRowMethod) -> bool {
        method == HomeRowMethod::Shared || self.gives_user_rows
    }
}

/// Home rows over a fresh database, with [`Recorded`] enabled.
async fn recorded(gives_user_rows: bool) -> (TempDir, Arc<Database>, Kept, Asked) {
    let data_dir = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data_dir.path()).await.unwrap());
    let asked = Asked::default();
    let plugins = Registry::new();
    let source = Recorded {
        asked: Arc::clone(&asked),
        gives_user_rows,
    };
    plugins.enable(Arc::new(source)).unwrap();
    let home = Kept::new(Arc::clone(&db), plugins.extension_points());
    (data_dir, db, home, asked)
}

async fn insert_user(db: &Database, name: &str) -> i64 {
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

async fn set_handle(db: &Database, user: i64, handle: &str) {
    let rows = PluginId::from_static("rows");
    let values = vec![("handle".to_owned(), handle.to_owned(), false)];
    replace_personal_settings(db, user, &rows, values)
        .await
        .unwrap();
}

/// Opens home as `user`, and waits for the refreshes it starts to finish.
async fn open_home(home: &Kept, user: i64) {
    home.list(user).await.unwrap();
    let started = Instant::now();
    while !lock(&home.refreshing).is_empty() {
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "still refreshing"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

/// The keys of the rows stored for `user`, the shared ones first.
async fn stored_keys(home: &Kept, user: i64) -> Vec<String> {
    let stored = home.stored(user).await.unwrap();
    stored.into_iter().map(|(_, row)| row.key).collect()
}

#[tokio::test]
async fn a_source_giving_only_shared_rows_is_never_asked_for_a_users() {
    let (_dir, db, home, asked) = recorded(false).await;
    let ada = insert_user(&db, "ada").await;
    set_handle(&db, ada, "ada").await;

    open_home(&home, ada).await;

    assert_eq!(*asked.lock().unwrap(), [None]);
    assert_eq!(stored_keys(&home, ada).await, ["shared"]);
}

#[tokio::test]
async fn each_users_rows_are_asked_with_only_their_own_personal_settings() {
    let (_dir, db, home, asked) = recorded(true).await;
    let ada = insert_user(&db, "ada").await;
    let bea = insert_user(&db, "bea").await;
    let carl = insert_user(&db, "carl").await;
    set_handle(&db, ada, "ada").await;
    set_handle(&db, bea, "bea").await;

    open_home(&home, ada).await;
    open_home(&home, bea).await;
    open_home(&home, carl).await;

    let handle = |handle: &str| Some([("handle".to_owned(), handle.to_owned())].into());
    assert_eq!(
        *asked.lock().unwrap(),
        [None, handle("ada"), handle("bea"), Some(BTreeMap::new())],
        "the shared rows once, then each User with their own settings, none for Carl"
    );
}

#[tokio::test]
async fn deleting_a_user_deletes_their_rows() {
    let (_dir, db, home, _) = recorded(true).await;
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
async fn changing_a_users_personal_settings_refreshes_their_rows_on_their_next_visit() {
    let (_dir, db, home, asked) = recorded(true).await;
    let ada = insert_user(&db, "ada").await;
    set_handle(&db, ada, "ada").await;
    open_home(&home, ada).await;

    set_handle(&db, ada, "ada-2").await;
    open_home(&home, ada).await;

    let handle = |handle: &str| Some([("handle".to_owned(), handle.to_owned())].into());
    assert_eq!(
        *asked.lock().unwrap(),
        [None, handle("ada"), handle("ada-2")],
        "the fresh shared rows aren't asked again"
    );
}
