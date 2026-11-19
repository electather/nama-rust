//! The Home rows Plugins give (ADR 0058), those every User sees and each User's own: stored per
//! Plugin and scope, refreshed in the background so home never waits on a Plugin, and read as the
//! titles on the Server they name.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::Duration,
};

use nama_plugin_api::{
    PluginError, PluginId,
    contract::{HomeRows as Given, SharedRowsAsked, TitleKind, UserRowsAsked},
    home_row::{HomeRowMethod, HomeRowSource},
    metadata::ExternalId,
};
use nama_proto::nama::v1::{PageRequest, PageResponse};
use serde::{Deserialize, Serialize};
use sqlx::SqliteConnection;
use tokio::{
    sync::mpsc::{self, UnboundedReceiver},
    task::JoinHandle,
};
use tracing::{error, warn};

use crate::{
    db::{self, Database},
    library::{
        Contents, FoundMovie, FoundShow, Libraries, LibraryError, LibraryKind, LibraryMovie,
        LibraryShow,
    },
    page::{self, Page},
    plugins::{Answer, ExtensionPoints, Failure, without_secrets},
    users::personal_settings_of,
};

/// The most rows of one answer home shows: a Plugin's shared rows, or its rows for one User.
const MAX_ROWS: usize = 3;
/// The most titles one row holds.
const MAX_TITLES: usize = 200;
/// The longest heading, in characters.
const MAX_HEADING: usize = 40;
/// The shortest time to live, which also spaces out refreshes of a Plugin giving no rows or
/// failing.
const MIN_TTL: Duration = Duration::from_mins(15);
const MAX_TTL: Duration = Duration::from_hours(7 * 24);

/// One Plugin's row, as stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Row {
    pub(crate) key: String,
    pub(crate) heading: String,
    titles: Vec<Title>,
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

/// A title a row names that is on the Server, in the oldest Library that has it.
#[derive(Debug, Clone)]
pub(crate) enum OnServer {
    Movie(FoundMovie),
    Show(FoundShow),
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
    /// The Plugins and scopes whose rows are being refreshed, so one refresh per Plugin and scope
    /// runs at a time.
    refreshing: Arc<Mutex<HashSet<Scope>>>,
    /// The loop [`HomeRows::refresh_as_plugins_come_up`] starts, until
    /// [`HomeRows::shutdown`] stops it.
    passing_on: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl HomeRows {
    pub(crate) fn new(db: Arc<Database>, points: ExtensionPoints) -> Self {
        Self {
            libraries: Libraries::new(Arc::clone(&db)),
            contents: Contents::new(Arc::clone(&db)),
            db,
            points,
            refreshing: Arc::default(),
            passing_on: Arc::default(),
        }
    }

    /// The rows `user` sees from each enabled Home row source, in Plugin id order, then each
    /// Plugin's shared rows before `user`'s own, each in the Plugin's order, leaving out a row
    /// with no title on the Server and one of `user`'s whose key a shared row has. Starts a
    /// background refresh of each of these scopes whose rows are stale or never were refreshed,
    /// and lists what is stored meanwhile. Deletes the rows of a Plugin no longer serving them.
    pub(crate) async fn list(&self, user: i64) -> Result<Vec<(PluginId, Row)>, Error> {
        // Stored first, so rows a refresh stores meanwhile aren't taken for a disabled Plugin's.
        let stored = self.stored(user).await?;
        let serving = self.points.providers::<dyn HomeRowSource>();
        self.refresh_stale(&serving, user).await?;
        let (shown, gone): (Vec<_>, Vec<_>) =
            (stored.into_iter()).partition(|(plugin, _)| serving.contains(plugin));
        // ponytail: deleted on the next listing rather than as the Plugin is disabled, which also
        // catches a crash's disable; until then they're only hidden. Delete them from
        // `PluginSettings::disable` if a disabled Plugin's stored rows ever matter sooner.
        let gone: BTreeSet<PluginId> = gone.into_iter().map(|(plugin, _)| plugin).collect();
        for plugin in gone {
            self.forget(plugin).await?;
        }
        let mut keys = HashSet::new();
        let shown: Vec<_> = (shown.into_iter())
            .filter(|(plugin, row)| keys.insert((plugin.clone(), row.key.clone())))
            .collect();
        let titles: Vec<Title> = (shown.iter())
            .flat_map(|(_, row)| row.titles.iter().cloned())
            .collect();
        let on_server = self.on_server(&titles).await?;
        Ok((shown.into_iter())
            .filter(|(_, row)| row.titles.iter().any(|title| on_server.contains_key(title)))
            .collect())
    }

    /// The `asked` page of the titles on the Server of the row `key` of the Plugin `plugin` that
    /// `user` sees, its shared one before `user`'s own, in the row's order, a title it names
    /// twice, or by two of its ids, once. A row no enabled source gives has none.
    pub(crate) async fn titles(
        &self,
        user: i64,
        plugin: &PluginId,
        key: &str,
        asked: &PageRequest,
    ) -> Result<(Vec<OnServer>, PageResponse), Error> {
        let list = serde_json::json!(["ListHomeRowTitles", plugin.as_str(), key]).to_string();
        let page = Page::<()>::read(asked, list.as_bytes())?;
        let titles = if self.serves(plugin) {
            self.stored_titles(user, plugin, key).await?
        } else {
            Vec::new()
        };
        let on_server = self.on_server(&titles).await?;
        // ponytail: pages in memory over at most `MAX_TITLES`, by place in the stored row, so a
        // refresh between two pages can skip or repeat a title. Store the rows' titles as rows
        // of their own if a row ever grows.
        let shown = shown(&titles, on_server);
        let total = shown.len();
        let mut listed: Vec<_> = (shown.into_iter())
            .filter(|&(place, _)| page.after.is_none_or(|((), after)| place > after))
            .collect();
        let mut paging = page.finish(&mut listed, |&(place, _)| ((), place));
        if page.is_first() {
            paging.total_size = Some(i32::try_from(total).unwrap_or(i32::MAX));
        }
        Ok((listed.into_iter().map(|(_, title)| title).collect(), paging))
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
                this.refresh(&plugin, None);
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

    /// Refreshes the Plugin `plugin`'s rows for `user`, or its shared rows for `None`, in the
    /// background, in the Plugin's scope, so disabling it cancels the call. Rows a cancelled
    /// refresh still stores stay hidden, as [`Self::list`] shows only enabled Plugins' rows. Does
    /// nothing if it isn't an enabled Home row source, or those rows are already being refreshed.
    pub(crate) fn refresh(&self, plugin: &PluginId, user: Option<i64>) {
        if !self.serves(plugin) {
            return;
        }
        let Some(refreshing) = Refreshing::start(&self.refreshing, (plugin.clone(), user)) else {
            return;
        };
        let this = self.clone();
        let id = plugin.clone();
        // Not spawned when the Plugin was disabled meanwhile, which drops `refreshing` with it.
        self.points.spawn_in(plugin, async move {
            this.refresh_now(&id, user).await;
            drop(refreshing);
        });
    }

    /// Whether the Plugin `plugin` is an enabled Home row source.
    fn serves(&self, plugin: &PluginId) -> bool {
        self.points
            .providers::<dyn HomeRowSource>()
            .contains(plugin)
    }

    /// Whether the Plugin `plugin` gives rows for `user`, or shared rows for `None`.
    async fn gives(&self, plugin: &PluginId, user: Option<i64>) -> bool {
        let method = match user {
            None => HomeRowMethod::Shared,
            Some(_) => HomeRowMethod::User,
        };
        let gives =
            (self.points).read_provider(plugin, |source: &dyn HomeRowSource| source.gives(method));
        gives.await.unwrap_or(false)
    }

    async fn refresh_now(&self, plugin: &PluginId, user: Option<i64>) {
        let answer = match user {
            None => self.ask_shared(plugin).await,
            Some(user) => match personal_settings_of(&self.db, user, plugin).await {
                Ok(settings) => self.ask_for(plugin, settings).await,
                Err(err) => {
                    warn!(plugin = %plugin, user, "could not read Personal settings to refresh Home rows: {err}");
                    return;
                }
            },
        };
        let Some(answer) = answer else {
            return;
        };
        // A failure keeps the stored rows. It puts off the next try, so a Plugin whose service
        // is down isn't called on every visit to home, and before the failure is logged, so a
        // reader of the log knows the next try is put off.
        let given = match answer.result {
            Ok(given) => given,
            Err(Failure { message, .. }) => {
                if let Err(err) = self.put_off(plugin, user).await {
                    warn!(plugin = %plugin, user, "could not put off refreshing the Plugin's Home rows: {err}");
                }
                warn!(plugin = %plugin, user, error = message, "refreshing the Plugin's Home rows failed");
                return;
            }
        };
        let (rows, fresh_for) = kept(given);
        if let Err(err) = self.replace(plugin, user, rows, fresh_for).await {
            warn!(plugin = %plugin, user, "could not store the Plugin's Home rows: {err}");
        }
    }

    /// Asks the Plugin `plugin` for its shared rows.
    async fn ask_shared(&self, plugin: &PluginId) -> Option<Answer<Given>> {
        let asked = SharedRowsAsked::default();
        (self.points)
            .ask(plugin, |source: Arc<dyn HomeRowSource>| {
                let asked = &asked;
                async move { source.shared_rows(asked).await }
            })
            .await
    }

    /// Asks the Plugin `plugin` for a User's rows with only that User's Personal `settings`, each
    /// value by key with whether it is secret. A secret one is blanked from the failure the
    /// Plugins page and the log show.
    async fn ask_for(
        &self,
        plugin: &PluginId,
        settings: BTreeMap<String, (String, bool)>,
    ) -> Option<Answer<Given>> {
        let secrets: Vec<String> = (settings.values())
            .filter(|(_, secret)| *secret)
            .map(|(value, _)| value.clone())
            .collect();
        let asked = UserRowsAsked {
            personal_settings: (settings.into_iter())
                .map(|(key, (value, _))| (key, value))
                .collect(),
            ..UserRowsAsked::default()
        };
        (self.points)
            .ask(plugin, |source: Arc<dyn HomeRowSource>| {
                let (asked, secrets) = (&asked, &secrets);
                async move {
                    let answer = source.user_rows(asked).await;
                    answer
                        .map_err(|err| PluginError::from(without_secrets(err.to_string(), secrets)))
                }
            })
            .await
    }

    /// Refreshes the rows `user` sees from each Plugin of `serving`, its shared ones and
    /// `user`'s own, that are stale or were never refreshed, and that it gives.
    async fn refresh_stale(
        &self,
        serving: &BTreeSet<PluginId>,
        user: i64,
    ) -> Result<(), sqlx::Error> {
        let now = db::now();
        let fresh: HashSet<Scope> = sqlx::query!(
            "SELECT plugin_id, user_id FROM home_row_refreshes
             WHERE stale_at > ? AND (user_id IS NULL OR user_id = ?)",
            now,
            user
        )
        .fetch_all(self.db.reader())
        .await?
        .into_iter()
        .map(|row| (PluginId::from(row.plugin_id), row.user_id))
        .collect();
        let scopes = serving
            .iter()
            .flat_map(|plugin| [(plugin.clone(), None), (plugin.clone(), Some(user))]);
        for (plugin, whose) in scopes.filter(|scope| !fresh.contains(scope)) {
            if self.gives(&plugin, whose).await {
                self.refresh(&plugin, whose);
            }
        }
        Ok(())
    }

    /// Every stored row `user` sees, in Plugin id order, then each Plugin's shared rows before
    /// `user`'s own, each in the Plugin's order.
    async fn stored(&self, user: i64) -> Result<Vec<(PluginId, Row)>, sqlx::Error> {
        let rows = sqlx::query!(
            "SELECT plugin_id, key, heading, titles FROM home_rows
             WHERE user_id IS NULL OR user_id = ?
             ORDER BY plugin_id, user_id IS NOT NULL, position",
            user
        )
        .fetch_all(self.db.reader())
        .await?;
        Ok((rows.into_iter())
            .map(|row| {
                let titles = titles_from_json(&row.titles);
                let row_of = Row {
                    key: row.key,
                    heading: row.heading,
                    titles,
                };
                (PluginId::from(row.plugin_id), row_of)
            })
            .collect())
    }

    /// The titles of the Plugin `plugin`'s row `key` that `user` sees, its shared one before
    /// `user`'s own, none if it has no such row.
    async fn stored_titles(
        &self,
        user: i64,
        plugin: &PluginId,
        key: &str,
    ) -> Result<Vec<Title>, sqlx::Error> {
        let plugin = plugin.as_str();
        let titles = sqlx::query_scalar!(
            "SELECT titles FROM home_rows
             WHERE plugin_id = ? AND key = ? AND (user_id IS NULL OR user_id = ?)
             ORDER BY user_id IS NOT NULL LIMIT 1",
            plugin,
            key,
            user
        )
        .fetch_optional(self.db.reader())
        .await?;
        Ok(titles.as_deref().map_or_default(titles_from_json))
    }

    /// Replaces the Plugin `plugin`'s rows for `user`, or its shared rows for `None`, with
    /// `rows`, fresh for `fresh_for`, in one write.
    async fn replace(
        &self,
        plugin: &PluginId,
        user: Option<i64>,
        rows: Vec<Row>,
        fresh_for: Duration,
    ) -> Result<(), sqlx::Error> {
        let plugin = plugin.as_str().to_owned();
        let stale_at = db::now().saturating_add(i64::try_from(fresh_for.as_secs()).unwrap_or(0));
        self.db
            .write(move |tx| {
                Box::pin(async move {
                    sqlx::query!(
                        "DELETE FROM home_rows WHERE plugin_id = ? AND user_id IS ?",
                        plugin,
                        user
                    )
                    .execute(&mut **tx)
                    .await?;
                    insert_rows(tx, &plugin, user, rows).await?;
                    set_stale_at(tx, &plugin, user, stale_at).await?;
                    Ok(())
                })
            })
            .await
    }

    /// Keeps the Plugin `plugin`'s rows for `user`, or its shared rows for `None`, as they are,
    /// and refreshes them no sooner than [`MIN_TTL`] from now.
    async fn put_off(&self, plugin: &PluginId, user: Option<i64>) -> Result<(), sqlx::Error> {
        let plugin = plugin.as_str().to_owned();
        let stale_at = db::now().saturating_add(i64::try_from(MIN_TTL.as_secs()).unwrap_or(0));
        self.db
            .write(move |tx| {
                Box::pin(async move {
                    set_stale_at(tx, &plugin, user, stale_at).await?;
                    Ok(())
                })
            })
            .await
    }

    /// Deletes the Plugin `plugin`'s rows in every scope, and when they were refreshed, so its
    /// next enable starts afresh.
    async fn forget(&self, plugin: PluginId) -> Result<(), sqlx::Error> {
        self.db
            .write(move |tx| {
                Box::pin(async move {
                    let plugin = plugin.as_str();
                    sqlx::query!("DELETE FROM home_rows WHERE plugin_id = ?", plugin)
                        .execute(&mut **tx)
                        .await?;
                    sqlx::query!("DELETE FROM home_row_refreshes WHERE plugin_id = ?", plugin)
                        .execute(&mut **tx)
                        .await?;
                    Ok(())
                })
            })
            .await
    }

    /// Those of `titles` that are on the Server, each in the oldest Library Matched to it.
    async fn on_server(&self, titles: &[Title]) -> Result<HashMap<Title, OnServer>, Error> {
        let ids_of = |kind: Kind| -> Vec<ExternalId> {
            (titles.iter())
                .filter(|title| title.0 == kind)
                .filter_map(Title::id)
                .collect()
        };
        let (movies, shows) = (ids_of(Kind::Movie), ids_of(Kind::Show));
        let mut found = HashMap::new();
        if titles.is_empty() {
            return Ok(found);
        }
        // Oldest first, so a title in two Libraries is found in the older.
        for library in self.libraries.list().await? {
            let (kind, wanted, listed) = match library.kind {
                LibraryKind::Movies if !movies.is_empty() => {
                    let listed = self.contents.movies(library.id, Some(&movies)).await?;
                    let listed = listed.iter().map(|movie| found_movie(library.id, movie));
                    (Kind::Movie, &movies, listed.collect::<Vec<_>>())
                }
                LibraryKind::Shows if !shows.is_empty() => {
                    let listed = self.contents.shows(library.id, Some(&shows)).await?;
                    let listed = listed.iter().map(|show| found_show(library.id, show));
                    (Kind::Show, &shows, listed.collect())
                }
                LibraryKind::Movies | LibraryKind::Shows => continue,
            };
            for title in listed {
                add_found(&mut found, kind, wanted, &title);
            }
        }
        Ok(found)
    }
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

/// What of `given` the Kernel keeps (ADR 0058): the first [`MAX_ROWS`] rows whose keys don't
/// repeat, each with its first [`MAX_TITLES`] titles less those it can't read, and its heading
/// cut to [`MAX_HEADING`] characters; and how long they stay fresh, the shortest of their times
/// to live, each clamped to between [`MIN_TTL`] and [`MAX_TTL`].
fn kept(given: Given) -> (Vec<Row>, Duration) {
    let mut keys = HashSet::new();
    let given_rows = (given.rows.into_iter())
        .filter(|row| keys.insert(row.key.clone()))
        .take(MAX_ROWS);
    let mut fresh_for = MAX_TTL;
    let mut rows = Vec::new();
    for row in given_rows {
        let ttl = Duration::from_secs(u64::from(row.ttl_seconds)).clamp(MIN_TTL, MAX_TTL);
        fresh_for = fresh_for.min(ttl);
        let titles = (row.titles.iter().take(MAX_TITLES))
            .filter_map(|title| {
                let kind = title.kind.as_known()?;
                let id = title.id.as_option()?;
                Title::given(kind, &id.namespace, &id.value)
            })
            .collect();
        rows.push(Row {
            key: row.key,
            heading: row.heading.chars().take(MAX_HEADING).collect(),
            titles,
        });
    }
    if rows.is_empty() {
        fresh_for = MIN_TTL;
    }
    (rows, fresh_for)
}

/// Those of `titles` that `on_server` has, each with its place in `titles`, in their order. A
/// title `titles` names twice, or by two of its ids, shows once, at its first place.
fn shown(titles: &[Title], mut on_server: HashMap<Title, OnServer>) -> Vec<(i64, OnServer)> {
    let mut seen = HashSet::new();
    (titles.iter().zip(0..))
        .filter_map(|(title, place)| Some((place, on_server.remove(title)?)))
        .filter(|(_, found)| seen.insert(found.browsed_as()))
        .collect()
}

impl OnServer {
    /// The Library and id browsing finds it by, which no other title on the Server shares.
    fn browsed_as(&self) -> (Kind, i64, ExternalId) {
        match self {
            Self::Movie(movie) => (Kind::Movie, movie.library, movie.id.clone()),
            Self::Show(show) => (Kind::Show, show.library, show.id.clone()),
        }
    }
}

/// The titles a row stores as JSON, `json`, or none if it doesn't read.
fn titles_from_json(json: &str) -> Vec<Title> {
    serde_json::from_str(json).unwrap_or_default()
}

/// Adds `title` to `found` under each of its ids `wanted` names that an older Library's title
/// didn't take.
fn add_found(
    found: &mut HashMap<Title, OnServer>,
    kind: Kind,
    wanted: &[ExternalId],
    title: &OnServer,
) {
    let ids = match title {
        OnServer::Movie(movie) => &movie.ids,
        OnServer::Show(show) => &show.ids,
    };
    for id in ids.iter().filter(|id| wanted.contains(id)) {
        let key = Title(kind, id.to_string());
        found.entry(key).or_insert_with(|| title.clone());
    }
}

/// Refreshes the Plugin `plugin`'s rows for `user`, or its shared rows for `None`, once a User
/// they're for opens home after `stale_at`.
async fn set_stale_at(
    conn: &mut SqliteConnection,
    plugin: &str,
    user: Option<i64>,
    stale_at: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO home_row_refreshes (plugin_id, user_id, stale_at) VALUES (?, ?, ?)
         ON CONFLICT (plugin_id, coalesce(user_id, 0)) DO UPDATE SET stale_at = excluded.stale_at",
        plugin,
        user,
        stale_at
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// Stores `rows` as the Plugin `plugin`'s for `user`, or its shared rows for `None`, in their
/// order.
async fn insert_rows(
    conn: &mut SqliteConnection,
    plugin: &str,
    user: Option<i64>,
    rows: Vec<Row>,
) -> Result<(), sqlx::Error> {
    for (row, position) in rows.into_iter().zip(0_i64..) {
        let titles =
            serde_json::to_string(&row.titles).map_err(|err| sqlx::Error::Encode(err.into()))?;
        sqlx::query!(
            "INSERT INTO home_rows (plugin_id, user_id, position, key, heading, titles)
             VALUES (?, ?, ?, ?, ?, ?)",
            plugin,
            user,
            position,
            row.key,
            row.heading,
            titles
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

fn found_movie(library: i64, movie: &LibraryMovie) -> OnServer {
    OnServer::Movie(FoundMovie {
        library,
        id: movie.id.clone(),
        ids: movie.ids.clone(),
        title: movie.title.clone(),
        release_date: movie.release_date.clone(),
        poster: movie.poster.clone(),
    })
}

fn found_show(library: i64, show: &LibraryShow) -> OnServer {
    OnServer::Show(FoundShow {
        library,
        id: show.id.clone(),
        ids: show.ids.clone(),
        title: show.title.clone(),
        poster: show.poster().cloned(),
    })
}

/// A Plugin and whose of its rows: one User's, or its shared rows for `None`.
type Scope = (PluginId, Option<i64>);

/// A scope's place in [`HomeRows::refreshing`], given up when dropped, even by a refresh
/// cancelled as its Plugin is disabled.
struct Refreshing {
    set: Arc<Mutex<HashSet<Scope>>>,
    scope: Scope,
}

impl Refreshing {
    /// Takes `scope`'s place in `set`, or `None` if a refresh holds it.
    fn start(set: &Arc<Mutex<HashSet<Scope>>>, scope: Scope) -> Option<Self> {
        let taken = lock(set).insert(scope.clone());
        taken.then(|| Self {
            set: Arc::clone(set),
            scope,
        })
    }
}

impl Drop for Refreshing {
    fn drop(&mut self) {
        lock(&self.set).remove(&self.scope);
    }
}

// No code that could panic runs while one is held, so a poisoned lock's value is still whole.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
#[path = "home_rows_tests.rs"]
mod tests;
