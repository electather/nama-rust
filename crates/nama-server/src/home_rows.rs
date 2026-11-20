//! The Home rows Plugins give (ADR 0058), those every User sees and each User's own: stored per
//! Plugin and scope, refreshed in the background so home never waits on a Plugin, and read as the
//! titles on the Server they name and, for a reader who may Request them, those missing from it.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    sync::{Arc, Mutex},
    time::Duration,
};

use nama_plugin_api::{
    PluginError, PluginId,
    contract::{HomeRows as Given, SharedRowsAsked, TitleKind, UserRowsAsked},
    home_row::{HomeRowMethod, HomeRowSource},
    metadata::ExternalId,
};
use nama_proto::nama::v1::{PageRequest, PageResponse, Permission, Request};
use serde::{Deserialize, Serialize};
use tokio::{
    sync::mpsc::{self, UnboundedReceiver},
    task::JoinHandle,
};
use tracing::{error, warn};

mod store;
use store::{
    Refreshing, Scope, add_found, found_movie, found_show, from_json, lock, replace_if_marked,
    set_stale_at,
};

use crate::{
    db::{self, Database},
    library::{Contents, FoundMovie, FoundShow, ImageRef, Libraries, LibraryError, LibraryKind},
    page::{self, Page},
    plugins::{Answer, ExtensionPoints, Failure, without_secrets},
    requests::{self, Listed, Requests},
    users::{User, personal_settings_of},
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
    /// Looks up the titles not on the Server as a Request would, at refresh.
    requests: Requests,
    /// The Plugins and scopes whose rows are being refreshed, so one refresh per Plugin and scope
    /// runs at a time.
    refreshing: Arc<Mutex<HashSet<Scope>>>,
    /// The loop [`HomeRows::refresh_as_plugins_come_up`] starts, until
    /// [`HomeRows::shutdown`] stops it.
    passing_on: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl HomeRows {
    pub(crate) fn new(db: Arc<Database>, points: ExtensionPoints, requests: Requests) -> Self {
        Self {
            libraries: Libraries::new(Arc::clone(&db)),
            contents: Contents::new(Arc::clone(&db)),
            db,
            points,
            requests,
            refreshing: Arc::default(),
            passing_on: Arc::default(),
        }
    }

    /// The rows `reader` sees from each enabled Home row source, in Plugin id order, then each
    /// Plugin's shared rows before `reader`'s own, each in the Plugin's order, leaving out a row
    /// with no title to show `reader` and one of `reader`'s whose key a shared row has. Starts a
    /// background refresh of each of these scopes whose rows are stale or never were refreshed,
    /// and lists what is stored meanwhile. Deletes the rows of a Plugin no longer serving them.
    pub(crate) async fn list(&self, reader: &User) -> Result<Vec<(PluginId, Row)>, Error> {
        // Stored first, so rows a refresh stores meanwhile aren't taken for a disabled Plugin's.
        let stored = self.stored(reader.id).await?;
        let serving = self.points.providers::<dyn HomeRowSource>();
        self.refresh_stale(&serving, reader.id).await?;
        let (served, gone): (Vec<_>, Vec<_>) =
            (stored.into_iter()).partition(|(plugin, _)| serving.contains(plugin));
        // ponytail: deleted on the next listing rather than as the Plugin is disabled, which also
        // catches a crash's disable; until then they're only hidden. Delete them from
        // `PluginSettings::disable` if a disabled Plugin's stored rows ever matter sooner.
        let gone: BTreeSet<PluginId> = gone.into_iter().map(|(plugin, _)| plugin).collect();
        for plugin in gone {
            self.forget(plugin).await?;
        }
        let mut keys = HashSet::new();
        let served: Vec<_> = (served.into_iter())
            .filter(|(plugin, row)| keys.insert((plugin.clone(), row.key.clone())))
            .collect();
        let titles: Vec<Title> = (served.iter())
            .flat_map(|(_, row)| row.titles.iter().cloned())
            .collect();
        let on_server = self.on_server(&titles).await?;
        Ok((served.into_iter())
            .filter(|(_, row)| !shown(row, &on_server, may_request(reader)).is_empty())
            .collect())
    }

    /// The `asked` page of the titles of the row `key` of the Plugin `plugin` that `reader` sees,
    /// its shared one before `reader`'s own, that are on the Server or that `reader` may Request,
    /// in the row's order, a title it names twice, or by two of its ids, once. A row no enabled
    /// source gives has none.
    pub(crate) async fn titles(
        &self,
        reader: &User,
        plugin: &PluginId,
        key: &str,
        asked: &PageRequest,
    ) -> Result<(Vec<RowTitle>, PageResponse), Error> {
        let list = serde_json::json!(["ListHomeRowTitles", plugin.as_str(), key]).to_string();
        let page = Page::<()>::read(asked, list.as_bytes())?;
        let row = if self.serves(plugin) {
            self.stored_row(reader.id, plugin, key).await?
        } else {
            Row::default()
        };
        let on_server = self.on_server(&row.titles).await?;
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
                this.refresh(&plugin, None).await;
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
    /// nothing if it isn't an enabled Home row source giving those rows, or they're already being
    /// refreshed.
    async fn refresh(&self, plugin: &PluginId, user: Option<i64>) {
        if !self.gives(plugin, user).await {
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

    /// Whether the Plugin `plugin` is an enabled Home row source giving rows for `user`, or
    /// shared rows for `None`.
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
        // Put off before the Plugin is asked, so one whose service is down isn't called on every
        // visit to home. A User's change to their Personal settings meanwhile deletes the put-off
        // (the triggers on `personal_settings`), and `replace` then drops this answer, built from
        // the old settings, rather than store it as fresh.
        if let Err(err) = self.put_off(plugin, user).await {
            warn!(plugin = %plugin, user, "could not put off refreshing the Plugin's Home rows: {err}");
            return;
        }
        let answer = match user {
            None => self.ask_shared(plugin).await,
            Some(user) => match personal_settings_of(&self.db, user, plugin).await {
                Ok(settings) => self.ask_user_rows(plugin, settings).await,
                Err(err) => {
                    warn!(plugin = %plugin, user, "could not read Personal settings to refresh Home rows: {err}");
                    return;
                }
            },
        };
        let Some(answer) = answer else {
            return;
        };
        let given = match answer.result {
            Ok(given) => given,
            Err(Failure { message, .. }) => {
                warn!(plugin = %plugin, user, error = message, "refreshing the Plugin's Home rows failed");
                return;
            }
        };
        let (mut rows, mut fresh_for) = kept(given);
        let unanswered = match self.look_up_missing(plugin, user, &mut rows).await {
            Ok(unanswered) => unanswered,
            Err(err) => {
                warn!(plugin = %plugin, user, "could not look up the titles of the Plugin's Home rows: {err}");
                return;
            }
        };
        // A title source that is down isn't one that lacks the titles, so the next refresh, the
        // soonest allowed, asks it again rather than the rows' own, up to a week away.
        if unanswered.is_some() {
            fresh_for = MIN_TTL;
        }
        if let Err(err) = self.replace(plugin, user, rows, fresh_for).await {
            warn!(plugin = %plugin, user, "could not store the Plugin's Home rows: {err}");
            return;
        }
        // After the rows are stored, so a reader of the log knows they are.
        if let Some(err) = unanswered {
            warn!(plugin = %plugin, user, "the title source couldn't look up the Plugin's Home row titles, which keep what the last refresh looked up until a refresh in 15 minutes: {err}");
        }
    }

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
    async fn ask_user_rows(
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

    /// Fills in the titles of `rows`, the Plugin `plugin`'s for `user`, or its shared rows for
    /// `None`, that aren't on the Server, as the
    /// title source has them, leaving out those it doesn't. Once the title source fails, it asks
    /// no more and the rest keep what the last refresh looked up; the failure is returned.
    async fn look_up_missing(
        &self,
        plugin: &PluginId,
        user: Option<i64>,
        rows: &mut [Row],
    ) -> Result<Option<requests::Error>, Error> {
        let titles: HashSet<Title> = (rows.iter())
            .flat_map(|row| row.titles.iter().cloned())
            .collect();
        let titles: Vec<Title> = titles.into_iter().collect();
        let on_server = self.on_server(&titles).await?;
        let mut before: HashMap<Title, Missing> = (self.stored_missing(plugin, user).await?)
            .into_iter()
            .map(|missing| (missing.title.clone(), missing))
            .collect();
        let mut looked_up = HashMap::new();
        let mut unanswered = None;
        // ponytail: one title-source call per missing title, one after another, at every refresh;
        // keep the last refresh's lookups if refreshes grow slow.
        for title in titles
            .iter()
            .filter(|title| !on_server.contains_key(*title))
        {
            let asked = match unanswered {
                None => Some(self.look_up(title).await),
                Some(_) => None,
            };
            let found = match asked {
                Some(Ok(found)) => found,
                Some(Err(err)) => {
                    unanswered = Some(err);
                    before.remove(title)
                }
                None => before.remove(title),
            };
            looked_up.extend(found.map(|missing| (title, missing)));
        }
        for row in rows {
            row.missing = (row.titles.iter())
                .filter_map(|title| looked_up.get(title).cloned())
                .collect();
        }
        Ok(unanswered)
    }

    /// `title` as a Request would look it up, `None` if the title source doesn't have it.
    async fn look_up(&self, title: &Title) -> Result<Option<Missing>, requests::Error> {
        let Some(id) = title.id() else {
            return Ok(None);
        };
        let listed = match title.0 {
            Kind::Movie => self.requests.listed_movie(&id).await,
            Kind::Show => self.requests.listed_show(&id).await,
        };
        match listed {
            Ok(listed) => Ok(Some(Missing::new(title.clone(), listed))),
            Err(requests::Error::NoSuchMovie(_) | requests::Error::NoSuchShow(_)) => Ok(None),
            Err(err) => Err(err),
        }
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
            self.refresh(&plugin, whose).await;
        }
        Ok(())
    }

    /// Every stored row `user` sees, in Plugin id order, then each Plugin's shared rows before
    /// `user`'s own, each in the Plugin's order.
    async fn stored(&self, user: i64) -> Result<Vec<(PluginId, Row)>, sqlx::Error> {
        let rows = sqlx::query!(
            "SELECT plugin_id, key, heading, titles, missing FROM home_rows
             WHERE user_id IS NULL OR user_id = ?
             ORDER BY plugin_id, user_id IS NOT NULL, position",
            user
        )
        .fetch_all(self.db.reader())
        .await?;
        Ok((rows.into_iter())
            .map(|row| {
                let row_of = Row {
                    key: row.key,
                    heading: row.heading,
                    titles: from_json(&row.titles),
                    missing: from_json(&row.missing),
                };
                (PluginId::from(row.plugin_id), row_of)
            })
            .collect())
    }

    /// The titles missing from the Server that the Plugin `plugin`'s rows for `user`, or its
    /// shared rows for `None`, stored at their last refresh.
    async fn stored_missing(
        &self,
        plugin: &PluginId,
        user: Option<i64>,
    ) -> Result<Vec<Missing>, sqlx::Error> {
        let plugin = plugin.as_str();
        let rows = sqlx::query_scalar!(
            "SELECT missing FROM home_rows WHERE plugin_id = ? AND user_id IS ?",
            plugin,
            user
        )
        .fetch_all(self.db.reader())
        .await?;
        Ok((rows.iter()).flat_map(|json| from_json(json)).collect())
    }

    /// The titles of the Plugin `plugin`'s row `key` that `user` sees, its shared one before
    /// `user`'s own, and those of them missing from the Server, as a [`Row`] with no key or
    /// heading, an empty one if it has no such row.
    async fn stored_row(
        &self,
        user: i64,
        plugin: &PluginId,
        key: &str,
    ) -> Result<Row, sqlx::Error> {
        let plugin = plugin.as_str();
        let row = sqlx::query!(
            "SELECT titles, missing FROM home_rows
             WHERE plugin_id = ? AND key = ? AND (user_id IS NULL OR user_id = ?)
             ORDER BY user_id IS NOT NULL LIMIT 1",
            plugin,
            key,
            user
        )
        .fetch_optional(self.db.reader())
        .await?;
        Ok(row.map_or_default(|row| Row {
            titles: from_json(&row.titles),
            missing: from_json(&row.missing),
            ..Row::default()
        }))
    }

    /// Replaces the Plugin `plugin`'s rows for `user`, or its shared rows for `None`, with
    /// `rows`, fresh for `fresh_for`, in one write. Stores nothing once the put-off its refresh
    /// started with is gone, as the Plugin was forgotten or the User changed their Personal
    /// settings meanwhile.
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
                Box::pin(async move { replace_if_marked(tx, &plugin, user, rows, stale_at).await })
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
            missing: Vec::new(),
        });
    }
    if rows.is_empty() {
        fresh_for = MIN_TTL;
    }
    (rows, fresh_for)
}

/// The titles of `row` that `on_server` has, or that are missing and of a kind `may_request`,
/// each with its place in the row, in its order. A title the row names twice, or by two of its
/// ids, shows once, at its first place.
fn shown(
    row: &Row,
    on_server: &HashMap<Title, OnServer>,
    may_request: impl Fn(Kind) -> bool,
) -> Vec<(i64, RowTitle)> {
    let missing: HashMap<&Title, &Missing> = (row.missing.iter())
        .map(|missing| (&missing.title, missing))
        .collect();
    let mut seen = HashSet::new();
    (row.titles.iter().zip(0..))
        .filter_map(|(title, place)| {
            let shown = match on_server.get(title) {
                Some(found) => RowTitle::OnServer(found.clone()),
                None if may_request(title.0) => missing.get(title)?.row_title()?,
                None => return None,
            };
            Some((place, shown))
        })
        .filter(|(_, shown)| seen.insert(shown.shown_once_as()))
        .collect()
}

/// Whether `reader` may Request a title of each kind.
fn may_request(reader: &User) -> impl Fn(Kind) -> bool {
    let permissions = reader.permissions;
    move |kind| match kind {
        Kind::Movie => permissions.allow(Permission::RequestMovie),
        Kind::Show => permissions.allow(Permission::RequestTv),
    }
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
