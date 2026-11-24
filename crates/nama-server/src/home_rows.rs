//! The Home rows Plugins give (ADR 0058), those every User sees and each User's own: stored per
//! Plugin and scope, refreshed in the background so home never waits on a Plugin, and read as the
//! titles on the Server they name and, for a reader who may Request them, those missing from it.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::Duration,
};

use nama_plugin_api::{
    PluginError, PluginId,
    contract::TitleKind,
    home_row::{HomeRowMethod, HomeRowSource},
    metadata::ExternalId,
};
use nama_proto::nama::v1::{PageRequest, PageResponse, Permission, Request};
use serde::{Deserialize, Serialize};
use sqlx::SqliteConnection;
use tokio::{
    sync::mpsc::{self, UnboundedReceiver},
    task::JoinHandle,
};
use tracing::{error, warn};

mod featured;
mod placed;
mod refreshes;
mod rows;
pub(crate) use featured::Suggested;
use featured::Suggestions;
use placed::placed_by;
use refreshes::{Refreshes, Scope};
use rows::Rows;

use crate::{
    db::{self, Database},
    library::{Contents, ImageRef, Libraries, LibraryError, LibraryKind, OnServer},
    page::{self, Page},
    plugins::{Answer, ExtensionPoints, Failure, without_secrets},
    requests::{Listed, Requests},
    server_settings::HomeRowId,
    users::{LibraryAccess, User, library_access, personal_settings_of},
};

/// The most rows of one answer home shows: a Plugin's shared rows, or its rows for one User.
const MAX_ROWS: usize = 20;
/// The most titles one row holds.
const MAX_TITLES: usize = 200;
/// The longest heading, in characters.
const MAX_HEADING: usize = 40;

/// One Plugin's row, as stored.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Row {
    pub(crate) key: String,
    pub(crate) heading: String,
    titles: Vec<Title>,
    /// Those of `titles` the title source had that weren't on the Server at the last refresh.
    missing: Vec<Missing>,
}

/// A title of a row that wasn't on the Server at its refresh, as the title source gave it then,
/// so reading it never asks the title source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Missing {
    /// The title it is in its row.
    title: Title,
    /// The id its Request is kept under.
    id: String,
    name: String,
    year: Option<i32>,
    /// The Plugin whose title source serves the poster, and its path there.
    poster: Option<(String, String)>,
}

/// A title a row shows a reader.
#[derive(Debug)]
pub(crate) enum RowTitle {
    OnServer(OnServer),
    /// With its open Request, if it has one the reader may see.
    MissingMovie(Listed, Option<Box<Request>>),
    MissingShow(Listed),
}

/// A title a row names, by the kind and id its Plugin gave.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct Title(Kind, String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Kind {
    Movie,
    Show,
}

/// Why the rows couldn't be read.
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error(transparent)]
    PageTokenInvalid(#[from] page::TokenInvalid),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("could not list the Libraries: {0}")]
    Libraries(#[from] LibraryError),
}

/// The Home rows Plugins give, which it stores, refreshes and reads.
///
/// Cloning gives another handle to the same rows and refreshes.
#[derive(Debug, Clone)]
pub(crate) struct HomeRows {
    db: Arc<Database>,
    points: ExtensionPoints,
    libraries: Libraries,
    contents: Contents,
    /// Looks up the titles not on the Server as a Request would, at refresh.
    requests: Requests,
    refreshes: Refreshes,
    /// The loop [`HomeRows::refresh_as_plugins_come_up`] starts, until
    /// [`HomeRows::shutdown`] stops it.
    passing_on: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl HomeRows {
    pub(crate) fn new(db: Arc<Database>, points: ExtensionPoints, requests: Requests) -> Self {
        Self {
            libraries: Libraries::new(Arc::clone(&db)),
            contents: Contents::new(Arc::clone(&db)),
            refreshes: Refreshes::new(Arc::clone(&db), points.clone()),
            db,
            points,
            requests,
            passing_on: Arc::default(),
        }
    }

    /// The rows `reader` sees from each enabled Home row source, in Plugin id order, then each
    /// Plugin's shared rows before `reader`'s own, each in the Plugin's order, leaving out a row
    /// with no title to show `reader` and one of `reader`'s whose key a shared row has. Starts a
    /// background refresh of each of these scopes whose rows are stale or never were refreshed,
    /// and lists what is stored meanwhile. Deletes the rows of a Plugin no longer giving them.
    /// Each row comes with what a Home layout places it by. Records the visit first, which
    /// [`forget_unvisited`] reads; failing to record it is logged, not answered.
    pub(crate) async fn list(&self, reader: &User) -> Result<Vec<(HomeRowId, Row)>, Error> {
        if let Err(err) = self.refreshes.visited(reader.id).await {
            warn!(
                user = reader.id,
                "could not record the visit to home: {err}"
            );
        }
        let rows = Rows(self.clone());
        let stored = rows.stored(reader.id).await?;
        self.refreshes.refresh_stale(&rows, reader.id).await?;
        // ponytail: deleted on the next listing rather than as the Plugin is disabled or comes up
        // as a new version, which also catches a crash's disable; until then they're only
        // hidden. Delete them from `PluginSettings::forget_disabled` if a disabled Plugin's
        // stored rows ever matter sooner.
        self.refreshes.forget(&rows).await?;
        let methods: HashSet<_> = (stored.iter())
            .map(|(scope, _)| (scope.plugin.clone(), scope.method))
            .collect();
        let mut given = HashSet::new();
        for (plugin, method) in methods {
            if self.refreshes.gives(&plugin, method).await {
                given.insert((plugin, method));
            }
        }
        let mut keys = HashSet::new();
        let served: Vec<_> = (stored.into_iter())
            .filter(|(scope, _)| given.contains(&(scope.plugin.clone(), scope.method)))
            .filter(|(scope, row)| keys.insert((scope.plugin.clone(), row.key.clone())))
            .map(|(scope, row)| (placed_by(scope, &row), row))
            .collect();
        let titles: Vec<Title> = (served.iter())
            .flat_map(|(_, row)| row.titles.iter().cloned())
            .collect();
        let seen = library_access(&self.db, reader.id).await?;
        let on_server = self.on_server(&titles, &seen).await?;
        Ok((served.into_iter())
            .filter(|(_, row)| !shown(row, &on_server, may_request(reader)).is_empty())
            .collect())
    }

    /// The `asked` page of the titles of the row `key` of the Plugin `plugin` that `reader` sees,
    /// its shared one before `reader`'s own, that are on the Server or that `reader` may Request,
    /// in the row's order, a title it names twice, or by two of its ids, once. A row the Plugin
    /// doesn't give, as it is disabled or its version stopped giving that scope, has none.
    pub(crate) async fn titles(
        &self,
        reader: &User,
        plugin: &PluginId,
        key: &str,
        asked: &PageRequest,
    ) -> Result<(Vec<RowTitle>, PageResponse), Error> {
        let list = serde_json::json!(["ListHomeRowTitles", plugin.as_str(), key]).to_string();
        let page = Page::<()>::read(asked, list.as_bytes())?;
        let row = Rows(self.clone())
            .stored_row(reader.id, plugin, key)
            .await?;
        let seen = library_access(&self.db, reader.id).await?;
        let on_server = self.on_server(&row.titles, &seen).await?;
        // ponytail: pages in memory over at most `MAX_TITLES`, by place in the stored row, so a
        // refresh between two pages can skip or repeat a title. Store the rows' titles as rows
        // of their own if a row ever grows.
        let shown = shown(&row, &on_server, may_request(reader));
        let total = shown.len();
        let mut listed: Vec<_> = (shown.into_iter())
            .filter(|&(place, _)| page.after.is_none_or(|((), after)| place > after))
            .collect();
        let mut paging = page.finish(&mut listed, |&(place, _)| ((), place));
        if page.is_first() {
            paging.total_size = Some(i32::try_from(total).unwrap_or(i32::MAX));
        }
        let mut titles = Vec::with_capacity(listed.len());
        // ponytail: one query per missing Movie, on a page a client asks for 10 of.
        for (_, title) in listed {
            titles.push(self.with_request(reader, title).await?);
        }
        Ok((titles, paging))
    }

    /// `title` with its open Request, if it is a missing Movie whose Request `reader` may see.
    async fn with_request(&self, reader: &User, title: RowTitle) -> Result<RowTitle, Error> {
        let RowTitle::MissingMovie(movie, _) = title else {
            return Ok(title);
        };
        let request = self.requests.open_movie_request(reader, &movie.id).await?;
        Ok(RowTitle::MissingMovie(movie, request.map(Box::new)))
    }

    /// Refreshes the shared rows of each Plugin `came_up` names as it is enabled, at startup or
    /// later, or its settings change, and passes its id on to the queue returned, for as long as
    /// `came_up` is open.
    pub(crate) fn refresh_as_plugins_come_up(
        &self,
        mut came_up: UnboundedReceiver<PluginId>,
    ) -> UnboundedReceiver<PluginId> {
        let (pass_on, passed) = mpsc::unbounded_channel();
        let this = self.clone();
        let passing_on = tokio::spawn(async move {
            while let Some(plugin) = came_up.recv().await {
                let shared = Scope {
                    plugin: plugin.clone(),
                    method: HomeRowMethod::Shared,
                    user: None,
                };
                this.refreshes.refresh(&Rows(this.clone()), shared).await;
                // Fails only once the reader stopped, when the Server no longer serves.
                let _: Result<(), _> = pass_on.send(plugin);
            }
        });
        *lock(&self.passing_on) = Some(passing_on);
        passed
    }

    /// Stops what [`Self::refresh_as_plugins_come_up`] started, closing the queue it returned,
    /// and logs a panic from it. Does nothing before it.
    pub(crate) async fn shutdown(&self) {
        let Some(passing_on) = lock(&self.passing_on).take() else {
            return;
        };
        passing_on.abort();
        if let Err(err) = passing_on.await
            && err.is_panic()
        {
            error!("refreshing Home rows as Plugins come up panicked: {err}");
        }
    }

    /// Asks the Plugin `plugin` for the User `user`'s answer through `export`, with only that
    /// User's Personal settings. A secret one is blanked from the failure the Plugins page and
    /// the log show. `None` if it isn't asked, logging a failure to read the settings as one to
    /// refresh `noun`.
    async fn ask_for_user<R, F>(
        &self,
        plugin: &PluginId,
        user: i64,
        noun: &str,
        export: impl Fn(Arc<dyn HomeRowSource>, Vec<(String, String)>) -> F,
    ) -> Option<Answer<R>>
    where
        F: Future<Output = Result<R, PluginError>>,
    {
        let settings = match personal_settings_of(&self.db, user, plugin).await {
            Ok(settings) => settings,
            Err(err) => {
                warn!(plugin = %plugin, user, "could not read Personal settings to refresh {noun}: {err}");
                return None;
            }
        };
        let (settings, secrets) = personal(settings);
        (self.points)
            .ask(plugin, |source| {
                let (answer, secrets) = (export(source, settings.clone()), &secrets);
                async move {
                    (answer.await)
                        .map_err(|err| PluginError::from(without_secrets(err.to_string(), secrets)))
                }
            })
            .await
    }

    /// Those of `titles` that are on the Server, each in the oldest Library `seen` keeps that has
    /// it, or `None` where only Libraries it doesn't keep have it.
    async fn on_server(
        &self,
        titles: &[Title],
        seen: &LibraryAccess,
    ) -> Result<HashMap<Title, Option<OnServer>>, Error> {
        let mut found = HashMap::new();
        if titles.is_empty() {
            return Ok(found);
        }
        let mut libraries = self.libraries.list().await?;
        // `find` takes each title from the first Library that has it, so those seen go first.
        libraries.sort_by_key(|library| !seen.sees(library.id));
        for (kind, in_libraries) in [
            (Kind::Movie, LibraryKind::Movies),
            (Kind::Show, LibraryKind::Shows),
        ] {
            let ids: Vec<ExternalId> = (titles.iter())
                .filter(|title| title.0 == kind)
                .filter_map(Title::id)
                .collect();
            let on_server = OnServer::find(&self.contents, &libraries, in_libraries, &ids).await?;
            found.extend((on_server.into_iter()).map(|(id, title)| {
                let shown = seen.sees(title.library()).then_some(title);
                (Title(kind, id.to_string()), shown)
            }));
        }
        Ok(found)
    }
}

/// Deletes the own Home rows and Featured suggestions of each User who hasn't opened home in at
/// least `unvisited_for`, with their put-offs, so their next visit refreshes them as a stale one
/// does. Answers how many Users had rows or suggestions.
pub(crate) async fn forget_unvisited(
    db: &Database,
    unvisited_for: Duration,
) -> Result<u32, sqlx::Error> {
    let unvisited_for = i64::try_from(unvisited_for.as_secs()).unwrap_or(i64::MAX);
    let before = db::now().saturating_sub(unvisited_for);
    db.write(move |tx| {
        Box::pin(async move {
            let mut forgotten = 0;
            for user in refreshes::take_unvisited(tx, before).await? {
                forgotten += u32::from(forget_user(tx, user).await?);
            }
            Ok(forgotten)
        })
    })
    .await
}

/// Deletes the User `user`'s own rows, suggestions and put-offs, and returns whether they had rows
/// or suggestions.
async fn forget_user(conn: &mut SqliteConnection, user: i64) -> Result<bool, sqlx::Error> {
    let had_rows = refreshes::forget_user::<Rows>(conn, user).await?;
    let had_suggestions = refreshes::forget_user::<Suggestions>(conn, user).await?;
    Ok(had_rows || had_suggestions)
}

impl Title {
    /// The title the Plugin gave as `given`, unless its kind is unset or its id malformed.
    fn given(kind: TitleKind, namespace: &str, value: &str) -> Option<Self> {
        let kind = match kind {
            TitleKind::TITLE_KIND_MOVIE => Kind::Movie,
            TitleKind::TITLE_KIND_SHOW => Kind::Show,
            TitleKind::TITLE_KIND_UNSPECIFIED => return None,
        };
        let id = ExternalId::new(namespace, value).ok()?;
        Some(Self(kind, id.to_string()))
    }

    fn id(&self) -> Option<ExternalId> {
        self.1.parse().ok()
    }
}

/// What `answer`, `scope`'s, gave, `None` if it wasn't asked or failed, logging the failure as
/// one refreshing `noun`.
fn answered<R>(answer: Option<Answer<R>>, scope: &Scope, noun: &str) -> Option<R> {
    match answer?.result {
        Ok(given) => Some(given),
        Err(Failure { message, .. }) => {
            let (plugin, user) = (&scope.plugin, scope.user);
            warn!(plugin = %plugin, user, error = message, "refreshing the Plugin's {noun} failed");
            None
        }
    }
}

/// The titles of `row` that `on_server` has in a Library the reader sees, or that are missing and
/// of a kind `may_request`, each with its place in the row, in its order. A title the row names twice, or by two of its
/// ids, shows once, at its first place.
fn shown(
    row: &Row,
    on_server: &HashMap<Title, Option<OnServer>>,
    may_request: impl Fn(Kind) -> bool,
) -> Vec<(i64, RowTitle)> {
    let missing: HashMap<&Title, &Missing> = (row.missing.iter())
        .map(|missing| (&missing.title, missing))
        .collect();
    let mut seen = HashSet::new();
    (row.titles.iter().zip(0..))
        .filter_map(|(title, place)| {
            let shown = match on_server.get(title) {
                Some(Some(found)) => RowTitle::OnServer(found.clone()),
                None if may_request(title.0) => missing.get(title)?.row_title()?,
                // Only a Library the reader doesn't see has it, or they may not Request it.
                Some(None) | None => return None,
            };
            Some((place, shown))
        })
        .filter(|(_, shown)| seen.insert(shown.shown_once_as()))
        .collect()
}

/// A User's Personal `settings`, each value by key with whether it is secret, as the values by
/// key a Plugin is asked with, and the secret values to blank from its failure.
fn personal(settings: BTreeMap<String, (String, bool)>) -> (Vec<(String, String)>, Vec<String>) {
    let secrets = (settings.values())
        .filter(|(_, secret)| *secret)
        .map(|(value, _)| value.clone())
        .collect();
    let values = (settings.into_iter())
        .map(|(key, (value, _))| (key, value))
        .collect();
    (values, secrets)
}

/// Whether `reader` may Request a title of each kind.
fn may_request(reader: &User) -> impl Fn(Kind) -> bool {
    let permissions = reader.permissions;
    move |kind| match kind {
        Kind::Movie => permissions.allow(Permission::RequestMovie),
        Kind::Show => permissions.allow(Permission::RequestTv),
    }
}

/// What a stored answer holds as JSON, `json`, or none if it doesn't read.
fn from_json<T: for<'de> Deserialize<'de>>(json: &str) -> Vec<T> {
    serde_json::from_str(json).unwrap_or_default()
}

fn to_json(value: &impl Serialize) -> Result<String, sqlx::Error> {
    serde_json::to_string(value).map_err(|err| sqlx::Error::Encode(err.into()))
}

// No code that could panic runs while one is held, so a poisoned lock's value is still whole.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl RowTitle {
    /// What no other title a row shows shares: for one on the Server, the Library and id browsing
    /// finds it by; for a missing one, the id its Request is kept under.
    fn shown_once_as(&self) -> (Kind, Option<i64>, ExternalId) {
        match self {
            Self::OnServer(OnServer::Movie(movie)) => {
                (Kind::Movie, Some(movie.library), movie.id.clone())
            }
            Self::OnServer(OnServer::Show(show)) => {
                (Kind::Show, Some(show.library), show.id.clone())
            }
            Self::MissingMovie(movie, _) => (Kind::Movie, None, movie.id.clone()),
            Self::MissingShow(show) => (Kind::Show, None, show.id.clone()),
        }
    }
}

impl Missing {
    fn new(title: Title, listed: Listed) -> Self {
        Self {
            title,
            id: listed.id.to_string(),
            name: listed.title,
            year: listed.year,
            poster: (listed.poster).map(|poster| (poster.plugin.as_str().to_owned(), poster.path)),
        }
    }

    /// It as a row shows it, unless its id no longer reads.
    fn row_title(&self) -> Option<RowTitle> {
        let listed = Listed {
            id: self.id.parse().ok()?,
            title: self.name.clone(),
            year: self.year,
            poster: (self.poster.clone()).map(|(plugin, path)| ImageRef {
                plugin: PluginId::from(plugin),
                path,
            }),
        };
        Some(match self.title.0 {
            Kind::Movie => RowTitle::MissingMovie(listed, None),
            Kind::Show => RowTitle::MissingShow(listed),
        })
    }
}

#[cfg(test)]
#[path = "home_rows/home_rows_tests.rs"]
mod tests;
