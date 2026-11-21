//! Home rows as an [`Adapter`] of [`Refreshes`](super::refreshes::Refreshes): asking a Plugin for
//! its shared rows and a User's, what of them is kept, and every read and write of `home_rows`.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};

use nama_plugin_api::{
    PluginId,
    contract::{HomeRows as Given, SharedRowsAsked, UserRowsAsked},
    home_row::{HomeRowMethod, HomeRowSource},
};
use sqlx::SqliteConnection;
use tracing::warn;

use super::{
    Error, HomeRows, Kind, MAX_HEADING, MAX_ROWS, MAX_TITLES, Missing, Row, Title, answered,
    from_json,
    refreshes::{Adapter, MIN_TTL, Scope},
    to_json,
};
use crate::{plugins::Answer, requests, server_settings::HomeRowId};

/// The Home rows Plugins give, shared and each User's own.
#[derive(Debug, Clone)]
pub(super) struct Rows(pub(super) HomeRows);

/// The rows of a scope as kept, with why the title source couldn't look up their titles, if it
/// couldn't.
type Kept = (Vec<Row>, Option<String>);

impl Adapter for Rows {
    type Kept = Kept;
    const NOUN: &'static str = "Home rows";
    const METHODS: &'static [HomeRowMethod] = &[HomeRowMethod::Shared, HomeRowMethod::User];

    fn scopes(&self, reader: i64) -> Vec<(HomeRowMethod, Option<i64>)> {
        vec![
            (HomeRowMethod::Shared, None),
            (HomeRowMethod::User, Some(reader)),
        ]
    }

    async fn ask(&self, scope: &Scope) -> Option<(Kept, Duration)> {
        let answer = match scope.user {
            None => self.ask_shared(&scope.plugin).await,
            Some(user) => {
                let ask = |source: Arc<dyn HomeRowSource>, settings: Vec<(String, String)>| async move {
                    let asked = UserRowsAsked {
                        personal_settings: settings.into_iter().collect(),
                        user_id: user,
                        ..UserRowsAsked::default()
                    };
                    source.user_rows(&asked).await
                };
                (self.0)
                    .ask_for_user(&scope.plugin, user, Self::NOUN, ask)
                    .await
            }
        };
        let (mut rows, mut fresh_for) = kept(answered(answer, scope, Self::NOUN)?);
        let unanswered = match self.look_up_missing(scope, &mut rows).await {
            Ok(unanswered) => unanswered,
            Err(err) => {
                let (plugin, user) = (&scope.plugin, scope.user);
                warn!(plugin = %plugin, user, "could not look up the titles of the Plugin's Home rows: {err}");
                return None;
            }
        };
        // A title source that is down isn't one that lacks the titles, so the next refresh, the
        // soonest allowed, asks it again rather than the rows' own, up to a week away.
        if unanswered.is_some() {
            fresh_for = MIN_TTL;
        }
        Some(((rows, unanswered.map(|err| err.to_string())), fresh_for))
    }

    async fn store(
        &self,
        conn: &mut SqliteConnection,
        scope: &Scope,
        (rows, unanswered): Kept,
    ) -> Result<(), sqlx::Error> {
        let (plugin, user) = (scope.plugin.as_str(), scope.user);
        sqlx::query!(
            "DELETE FROM home_rows WHERE plugin_id = ? AND user_id IS ?",
            plugin,
            user
        )
        .execute(&mut *conn)
        .await?;
        for (row, position) in rows.into_iter().zip(0_i64..) {
            let titles = to_json(&row.titles)?;
            let missing = to_json(&row.missing)?;
            sqlx::query!(
                "INSERT INTO home_rows (plugin_id, user_id, position, key, heading, titles, missing)
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
                plugin,
                user,
                position,
                row.key,
                row.heading,
                titles,
                missing
            )
            .execute(&mut *conn)
            .await?;
        }
        // In the write, after the guard, so an answer dropped as its put-off is gone logs nothing.
        if let Some(err) = unanswered {
            warn!(plugin = %plugin, user, "the title source couldn't look up the Plugin's Home row titles, which keep what the last refresh looked up until a refresh in 15 minutes: {err}");
        }
        Ok(())
    }

    async fn delete(
        &self,
        conn: &mut SqliteConnection,
        plugin: &str,
        method: HomeRowMethod,
    ) -> Result<(), sqlx::Error> {
        let users = method == HomeRowMethod::User;
        sqlx::query!(
            "DELETE FROM home_rows WHERE plugin_id = ? AND (user_id IS NOT NULL) = ?",
            plugin,
            users
        )
        .execute(conn)
        .await?;
        Ok(())
    }

    async fn delete_users_own(conn: &mut SqliteConnection, user: i64) -> Result<bool, sqlx::Error> {
        let deleted = sqlx::query!("DELETE FROM home_rows WHERE user_id = ?", user)
            .execute(conn)
            .await?;
        Ok(deleted.rows_affected() > 0)
    }

    async fn plugins(&self, method: HomeRowMethod) -> Result<Vec<PluginId>, sqlx::Error> {
        let users = method == HomeRowMethod::User;
        let plugins = sqlx::query_scalar!(
            "SELECT DISTINCT plugin_id FROM home_rows WHERE (user_id IS NOT NULL) = ?",
            users
        )
        .fetch_all(self.0.db.reader())
        .await?;
        Ok(plugins.into_iter().map(PluginId::from).collect())
    }
}

impl Rows {
    /// Every stored row `user` sees, in Plugin id order, then each Plugin's shared rows before
    /// `user`'s own, each in the Plugin's order, with its scope.
    pub(super) async fn stored(&self, user: i64) -> Result<Vec<(Scope, Row)>, sqlx::Error> {
        let rows = sqlx::query!(
            "SELECT plugin_id, user_id, key, heading, titles, missing FROM home_rows
             WHERE user_id IS NULL OR user_id = ?
             ORDER BY plugin_id, user_id IS NOT NULL, position",
            user
        )
        .fetch_all(self.0.db.reader())
        .await?;
        Ok((rows.into_iter())
            .map(|row| {
                let row_of = Row {
                    key: row.key,
                    heading: row.heading,
                    titles: from_json(&row.titles),
                    missing: from_json(&row.missing),
                };
                (scope(PluginId::from(row.plugin_id), row.user_id), row_of)
            })
            .collect())
    }

    /// The titles of the Plugin `plugin`'s row `key` that `user` sees in a scope it gives, its
    /// shared one before `user`'s own, and those of them missing from the Server, as a [`Row`]
    /// with no key or heading, an empty one if it has no such row.
    pub(super) async fn stored_row(
        &self,
        user: i64,
        plugin: &PluginId,
        key: &str,
    ) -> Result<Row, sqlx::Error> {
        let plugin_id = plugin.as_str();
        let rows = sqlx::query!(
            "SELECT user_id, titles, missing FROM home_rows
             WHERE plugin_id = ? AND key = ? AND (user_id IS NULL OR user_id = ?)
             ORDER BY user_id IS NOT NULL",
            plugin_id,
            key,
            user
        )
        .fetch_all(self.0.db.reader())
        .await?;
        for row in rows {
            let method = scope(plugin.clone(), row.user_id).method;
            if self.0.refreshes.gives(plugin, method).await {
                return Ok(Row {
                    titles: from_json(&row.titles),
                    missing: from_json(&row.missing),
                    ..Row::default()
                });
            }
        }
        Ok(Row::default())
    }

    /// Every Plugin's shared rows as last stored, in Plugin id order, then in the Plugin's order,
    /// each as a Home layout places it, with its heading.
    pub(super) async fn stored_shared(&self) -> Result<Vec<(HomeRowId, String)>, sqlx::Error> {
        let stored = sqlx::query!(
            "SELECT plugin_id, key, heading FROM home_rows WHERE user_id IS NULL
             ORDER BY plugin_id, position"
        )
        .fetch_all(self.0.db.reader())
        .await?;
        Ok((stored.into_iter())
            .map(|row| {
                let plugin = PluginId::from(row.plugin_id);
                let id = HomeRowId::Shared {
                    plugin,
                    key: row.key,
                };
                (id, row.heading)
            })
            .collect())
    }

    /// The titles missing from the Server that `scope`'s rows stored at their last refresh.
    async fn stored_missing(&self, scope: &Scope) -> Result<Vec<Missing>, sqlx::Error> {
        let (plugin, user) = (scope.plugin.as_str(), scope.user);
        let rows = sqlx::query_scalar!(
            "SELECT missing FROM home_rows WHERE plugin_id = ? AND user_id IS ?",
            plugin,
            user
        )
        .fetch_all(self.0.db.reader())
        .await?;
        Ok((rows.iter()).flat_map(|json| from_json(json)).collect())
    }

    async fn ask_shared(&self, plugin: &PluginId) -> Option<Answer<Given>> {
        let asked = SharedRowsAsked::default();
        (self.0.points)
            .ask(plugin, |source: Arc<dyn HomeRowSource>| {
                let asked = &asked;
                async move { source.shared_rows(asked).await }
            })
            .await
    }

    /// Fills in the titles of `rows`, `scope`'s, that aren't on the Server, as the title source
    /// has them, leaving out those it doesn't. Once the title source fails, it asks no more and
    /// the rest keep what the last refresh looked up; the failure is returned.
    async fn look_up_missing(
        &self,
        scope: &Scope,
        rows: &mut [Row],
    ) -> Result<Option<requests::Error>, Error> {
        let titles: HashSet<Title> = (rows.iter())
            .flat_map(|row| row.titles.iter().cloned())
            .collect();
        let titles: Vec<Title> = titles.into_iter().collect();
        let on_server = self.0.on_server(&titles).await?;
        let mut before: HashMap<Title, Missing> = (self.stored_missing(scope).await?)
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
            Kind::Movie => self.0.requests.listed_movie(&id).await,
            Kind::Show => self.0.requests.listed_show(&id).await,
        };
        match listed {
            Ok(listed) => Ok(Some(Missing::new(title.clone(), listed))),
            Err(requests::Error::NoSuchMovie(_) | requests::Error::NoSuchShow(_)) => Ok(None),
            Err(err) => Err(err),
        }
    }
}

/// The scope of the Plugin `plugin`'s rows for `user`, or its shared rows for `None`.
fn scope(plugin: PluginId, user: Option<i64>) -> Scope {
    let method = match user {
        None => HomeRowMethod::Shared,
        Some(_) => HomeRowMethod::User,
    };
    Scope {
        plugin,
        method,
        user,
    }
}

/// What of `given` the Kernel keeps (ADR 0058, ADR 0091): the first [`MAX_ROWS`] rows whose keys
/// don't repeat, each with its first [`MAX_TITLES`] titles less those it can't read, and its
/// heading cut to [`MAX_HEADING`] characters; and how long they stay fresh, the shortest of their
/// times to live, or [`MIN_TTL`] for none.
fn kept(given: Given) -> (Vec<Row>, Duration) {
    let mut keys = HashSet::new();
    let given_rows = (given.rows.into_iter())
        .filter(|row| keys.insert(row.key.clone()))
        .take(MAX_ROWS);
    let mut fresh_for = None;
    let mut rows = Vec::new();
    for row in given_rows {
        let ttl = Duration::from_secs(u64::from(row.ttl_seconds));
        fresh_for = Some(fresh_for.map_or(ttl, |shortest: Duration| shortest.min(ttl)));
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
    (rows, fresh_for.unwrap_or(MIN_TTL))
}

#[cfg(test)]
#[path = "rows_tests.rs"]
mod tests;
