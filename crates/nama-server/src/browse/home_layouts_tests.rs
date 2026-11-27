use std::sync::Arc;

use nama_plugin_api::{PluginId, home_row::HomeRowMethod};
use tempfile::TempDir;

use super::{HomeLayouts, HomeRow, SeenHomeLayout, ShownRow};
use crate::{
    browse::{Browsing, Featured},
    db::Database,
    home_rows::{HomeRows, ScriptedRows},
    library::fixture::{MatchedMovie, movies_library, tmdb_movie},
    plugins::ExtensionPoints,
    server_settings::{HomeLayout, HomeRowId, PlacedRow, Settings},
    users::{AskedHomeLayout, LibraryAccess, Permissions, User, Users, testing::insert_user},
};

const ALIEN: i64 = 348;
const SHARED: &[HomeRowMethod] = &[HomeRowMethod::Shared];

/// A Server's database with three Movies Libraries, oldest first, Alien in the first, and its
/// Home, whose Plugin `rows` gives what a test scripts.
struct Fixture {
    _folders: [TempDir; 4],
    db: Arc<Database>,
    libraries: [i64; 3],
    home_rows: HomeRows,
    settings: Settings,
    layouts: HomeLayouts,
}

impl Fixture {
    /// With the Plugin `rows` giving what `rows` scripts, given the three Libraries' ids.
    async fn new(rows: impl FnOnce([i64; 3]) -> ScriptedRows) -> Self {
        let folders = [(); 4].map(|()| tempfile::tempdir().unwrap());
        let db = Arc::new(Database::open(folders[0].path()).await.unwrap());
        let alien = MatchedMovie::new(&db, folders[1].path(), tmdb_movie(ALIEN, "Alien")).await;
        let libraries = [
            alien.library,
            movies_library(&db, folders[2].path()).await,
            movies_library(&db, folders[3].path()).await,
        ];
        let home_rows = rows(libraries).home_rows(&db);
        let settings = Settings::new(Arc::clone(&db), folders[0].path().to_owned());
        let browsing = Browsing::new(Arc::clone(&db), ExtensionPoints::default());
        let layouts = HomeLayouts::new(
            Arc::clone(&db),
            browsing,
            home_rows.clone(),
            settings.clone(),
        );
        Self {
            _folders: folders,
            db,
            libraries,
            home_rows,
            settings,
            layouts,
        }
    }

    /// A User named `name` who sees every Library, or only those `only` names.
    async fn user(&self, name: &str, only: Option<&[i64]>) -> User {
        let id = insert_user(&self.db, name).await;
        if let Some(only) = only {
            self.see(id, LibraryAccess::Only(only.iter().copied().collect()))
                .await;
        }
        User {
            id,
            name: name.to_owned(),
            permissions: Permissions::NEW_USER,
            created_at: 0,
        }
    }

    /// Gives the User `user` `access`, as an Admin does.
    async fn see(&self, user: i64, access: LibraryAccess) {
        let admin = User {
            id: 0,
            name: "Root".to_owned(),
            permissions: Permissions::ADMIN,
            created_at: 0,
        };
        let users = Users::new(Arc::clone(&self.db), ExtensionPoints::default());
        users
            .set_library_access(&admin, user, access)
            .await
            .unwrap();
    }

    /// `user`'s Home rows once their Plugin rows were refreshed.
    async fn refreshed_home(&self, user: &User) -> Vec<String> {
        self.layouts.home_rows(user).await.unwrap();
        self.home_rows.settled().await;
        let rows = self.layouts.home_rows(user).await.unwrap();
        rows.iter().map(|row| self.named(row)).collect()
    }

    /// `layout`'s rows as a test names them, each marked if it is hidden.
    fn named_layout(&self, layout: &SeenHomeLayout) -> Vec<String> {
        (layout.rows.iter())
            .map(|(row, shown)| {
                let hidden = if *shown { "" } else { " (hidden)" };
                format!("{}{hidden}", self.named(row))
            })
            .collect()
    }

    /// `row` as a test names it, by the Library a Recently Added row shows or a Plugin's row
    /// links to, in the order created.
    fn named(&self, (id, shown): &HomeRow) -> String {
        let library = |library: i64| {
            let at = self.libraries.iter().position(|each| *each == library);
            ["A", "B", "C"][at.unwrap()]
        };
        match shown {
            ShownRow::ContinueWatching => "Continue Watching".to_owned(),
            ShownRow::RecentlyAdded(of) => format!("Recently Added {}", library(of.id)),
            ShownRow::Plugin {
                key,
                library: linked,
                ..
            } => {
                let linked = linked.map_or_default(|to| format!(" to {}", library(to)));
                format!("{key}{linked}")
            }
            ShownRow::UsersOwn => format!("{}'s own", id.plugin().unwrap()),
        }
    }
}

#[tokio::test]
async fn a_users_home_lists_continue_watching_then_a_recently_added_row_per_seen_library_then_the_plugins_rows()
 {
    let fixture =
        Fixture::new(|_| ScriptedRows::new(SHARED).sharing("picks", None, &[ALIEN])).await;
    let [a, _, c] = fixture.libraries;
    let ada = fixture.user("ada", Some(&[c, a])).await;

    let home = fixture.refreshed_home(&ada).await;

    assert_eq!(
        home,
        [
            "Continue Watching",
            "Recently Added A",
            "Recently Added C",
            "picks"
        ]
    );
}

#[tokio::test]
async fn a_plugin_row_links_its_library_only_for_a_user_who_sees_that_library() {
    let fixture =
        Fixture::new(|[_, b, _]| ScriptedRows::new(SHARED).sharing("picks", Some(b), &[ALIEN]))
            .await;
    let [a, ..] = fixture.libraries;
    let ada = fixture.user("ada", None).await;
    let bea = fixture.user("bea", Some(&[a])).await;

    let adas = fixture.refreshed_home(&ada).await;
    let beas = fixture.refreshed_home(&bea).await;

    assert_eq!(adas.last().unwrap(), "picks to B");
    assert_eq!(beas.last().unwrap(), "picks");
}

#[tokio::test]
async fn a_row_their_home_layout_hides_is_left_out_of_their_home_and_listed_hidden_in_their_layout()
{
    let fixture = Fixture::new(|_| ScriptedRows::new(SHARED)).await;
    let [a, ..] = fixture.libraries;
    let ada = fixture.user("ada", Some(&[a])).await;

    let saved = fixture.layouts.save(&ada, hiding(a)).await.unwrap();

    assert_eq!(
        fixture.named_layout(&saved),
        ["Continue Watching", "Recently Added A (hidden)"]
    );
    assert_eq!(fixture.refreshed_home(&ada).await, ["Continue Watching"]);
    let read = fixture.layouts.of(&ada).await.unwrap();
    assert_eq!(
        fixture.named_layout(&read),
        ["Continue Watching", "Recently Added A (hidden)"]
    );
}

#[tokio::test]
async fn a_user_following_a_default_that_hides_featured_gets_none_and_one_who_shows_it_gets_it() {
    let fixture = Fixture::new(|_| ScriptedRows::new(SHARED)).await;
    let ada = fixture.user("ada", None).await;
    let bea = fixture.user("bea", None).await;
    let hides_featured = HomeLayout {
        featured_shown: false,
        rows: Vec::new(),
    };
    fixture
        .settings
        .set_default_home_layout(hides_featured)
        .await
        .unwrap();
    let shows_featured = AskedHomeLayout {
        featured_shown: Some(true),
        rows: Vec::new(),
    };
    fixture.layouts.save(&bea, shows_featured).await.unwrap();

    let adas = fixture.layouts.featured(&ada).await.unwrap();
    let beas = fixture.layouts.featured(&bea).await.unwrap();

    assert!(adas.is_empty());
    let titles: Vec<&str> = (beas.iter())
        .map(|featured| match featured {
            Featured::Movie { movie, .. } => movie.title.as_str(),
            Featured::Show { show, .. } => show.title.as_str(),
        })
        .collect();
    assert_eq!(titles, ["Alien"], "added in the last 14 days");
}

#[tokio::test]
async fn a_save_naming_the_recently_added_row_of_a_library_the_user_doesnt_see_changes_nothing_for_it()
 {
    let fixture = Fixture::new(|_| ScriptedRows::new(SHARED)).await;
    let [a, b, _] = fixture.libraries;
    let bea = fixture.user("bea", Some(&[a])).await;

    fixture.layouts.save(&bea, hiding(b)).await.unwrap();
    fixture
        .see(bea.id, LibraryAccess::Only([a, b].into()))
        .await;

    let layout = fixture.layouts.of(&bea).await.unwrap();
    assert_eq!(
        fixture.named_layout(&layout),
        ["Continue Watching", "Recently Added A", "Recently Added B"]
    );
}

#[tokio::test]
async fn a_reorder_naming_rows_the_user_cant_see_places_only_the_rows_they_could_see() {
    let fixture = Fixture::new(|_| ScriptedRows::new(SHARED)).await;
    let [a, b, _] = fixture.libraries;
    let bea = fixture.user("bea", Some(&[a])).await;
    let asked = [
        recently_added(b),
        recently_added(a),
        HomeRowId::ContinueWatching,
    ];

    let reordered = fixture.layouts.reorder(&bea, &asked).await.unwrap();
    fixture
        .see(bea.id, LibraryAccess::Only([a, b].into()))
        .await;

    assert_eq!(
        fixture.named_layout(&reordered),
        ["Recently Added A", "Continue Watching"]
    );
    let layout = fixture.layouts.of(&bea).await.unwrap();
    assert_eq!(
        fixture.named_layout(&layout),
        ["Recently Added A", "Continue Watching", "Recently Added B"],
        "B, unplaced, goes at the end"
    );
}

#[tokio::test]
async fn the_default_lists_every_librarys_recently_added_row_and_each_plugins_users_own_rows_as_one_entry_in_its_order()
 {
    let both = &[HomeRowMethod::Shared, HomeRowMethod::User];
    let fixture = Fixture::new(|_| ScriptedRows::new(both).sharing("picks", None, &[ALIEN])).await;
    let [a, b, _] = fixture.libraries;
    let ada = fixture.user("ada", Some(&[a])).await;
    fixture.refreshed_home(&ada).await;
    let default = HomeLayout {
        featured_shown: true,
        rows: vec![
            PlacedRow {
                row: HomeRowId::UsersOwn {
                    plugin: PluginId::from_static("rows"),
                },
                shown: true,
            },
            PlacedRow {
                row: recently_added(b),
                shown: false,
            },
        ],
    };
    fixture
        .settings
        .set_default_home_layout(default)
        .await
        .unwrap();

    let layout = fixture.layouts.default_layout().await.unwrap();

    assert!(layout.featured_shown);
    assert_eq!(
        fixture.named_layout(&layout),
        [
            "rows's own",
            "Recently Added B (hidden)",
            "Continue Watching",
            "Recently Added A",
            "Recently Added C",
            "picks",
        ]
    );
}

fn recently_added(library: i64) -> HomeRowId {
    HomeRowId::RecentlyAdded { library }
}

/// A save hiding the Recently Added row of `library`, leaving Featured as it is.
fn hiding(library: i64) -> AskedHomeLayout {
    AskedHomeLayout {
        featured_shown: None,
        rows: vec![PlacedRow {
            row: recently_added(library),
            shown: false,
        }],
    }
}
