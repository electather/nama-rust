//! The lifecycle of an answer a Plugin gives ahead of time for a scope (ADR 0058, ADR 0072): put
//! off before the Plugin is asked, refreshed in the background one at a time per scope, replaced
//! in one write only while its put-off is still stored, and deleted once the Plugin stops giving
//! it. Home rows and Featured suggestions are its two [`Adapter`]s.

use std::{
    collections::{BTreeSet, HashSet},
    sync::{Arc, Mutex},
    time::Duration,
};

use nama_plugin_api::{
    PluginId,
    home_row::{HomeRowMethod, HomeRowSource},
};
use sqlx::SqliteConnection;
use tracing::warn;

use super::lock;
use crate::{
    db::{self, Database},
    plugins::ExtensionPoints,
};

/// The shortest time to live, which also spaces out refreshes of a Plugin giving nothing or
/// failing.
pub(super) const MIN_TTL: Duration = Duration::from_mins(15);
const MAX_TTL: Duration = Duration::from_hours(7 * 24);

/// Whose answer of a Plugin's, by one method: one User's, or no User's for shared rows.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct Scope {
    pub(super) plugin: PluginId,
    pub(super) method: HomeRowMethod,
    pub(super) user: Option<i64>,
}

/// An answer a Plugin gives ahead of time: what is its own of asking for it and storing it, in
/// its own table. [`Refreshes`] keeps it fresh.
pub(super) trait Adapter: Clone + Send + Sync + 'static {
    /// What of an answer is stored.
    type Kept: Send + 'static;
    /// What the log calls the answers, as in "the Plugin's Home rows".
    const NOUN: &'static str;

    /// The methods giving its answers.
    fn methods(&self) -> &'static [HomeRowMethod];

    /// Asks the Plugin for `scope`'s answer, and returns what of it is kept with how long it
    /// stays fresh, which [`Refreshes`] clamps. `None`, having logged why, for nothing to store.
    fn ask(&self, scope: &Scope) -> impl Future<Output = Option<(Self::Kept, Duration)>> + Send;

    /// Replaces `scope`'s stored answer with `kept`, inside the write renewing its put-off.
    fn store(
        &self,
        conn: &mut SqliteConnection,
        scope: &Scope,
        kept: Self::Kept,
    ) -> impl Future<Output = Result<(), sqlx::Error>> + Send;

    /// Deletes the Plugin `plugin`'s answers by `method`, every User's for one per User.
    fn delete(
        &self,
        conn: &mut SqliteConnection,
        plugin: &str,
        method: HomeRowMethod,
    ) -> impl Future<Output = Result<(), sqlx::Error>> + Send;

    /// The Plugins it stores answers by `method` of.
    fn plugins(
        &self,
        method: HomeRowMethod,
    ) -> impl Future<Output = Result<Vec<PluginId>, sqlx::Error>> + Send;
}

/// Keeps the answers of each [`Adapter`] fresh, alone reading and writing their put-offs in
/// `home_row_refreshes` and `featured_refreshes`.
///
/// Cloning gives another handle to the same refreshes.
#[derive(Debug, Clone)]
pub(super) struct Refreshes {
    db: Arc<Database>,
    points: ExtensionPoints,
    /// The scopes being refreshed, so one refresh per scope runs at a time.
    running: Arc<Mutex<HashSet<Scope>>>,
}

impl Refreshes {
    pub(super) fn new(db: Arc<Database>, points: ExtensionPoints) -> Self {
        Self {
            db,
            points,
            running: Arc::default(),
        }
    }

    /// Refreshes, in the background, each scope of `adapter`'s that `reader` sees, of each
    /// enabled Home row source, that is stale or was never refreshed.
    pub(super) async fn refresh_stale<A: Adapter>(
        &self,
        adapter: &A,
        reader: i64,
    ) -> Result<(), sqlx::Error> {
        let serving = self.points.providers::<dyn HomeRowSource>();
        for &method in adapter.methods() {
            let user = (method != HomeRowMethod::Shared).then_some(reader);
            let fresh = self.fresh(method, user).await?;
            for plugin in serving.iter().filter(|plugin| !fresh.contains(*plugin)) {
                let scope = Scope {
                    plugin: plugin.clone(),
                    method,
                    user,
                };
                self.refresh(adapter, scope).await;
            }
        }
        Ok(())
    }

    /// Refreshes `scope` in the background, in its Plugin's scope, so disabling it cancels the
    /// call. An answer a cancelled refresh still stores stays hidden, as the reads show only what
    /// a Plugin [gives](Self::gives). Does nothing if it doesn't give it, or a refresh of `scope`
    /// is running.
    pub(super) async fn refresh<A: Adapter>(&self, adapter: &A, scope: Scope) {
        if !self.gives(&scope.plugin, scope.method).await {
            return;
        }
        let Some(running) = Running::start(&self.running, scope.clone()) else {
            return;
        };
        let (this, adapter, plugin) = (self.clone(), adapter.clone(), scope.plugin.clone());
        // Not spawned when the Plugin was disabled meanwhile, which drops `running` with it.
        self.points.spawn_in(&plugin, async move {
            this.refresh_now(&adapter, &scope).await;
            drop(running);
        });
    }

    /// Whether the Plugin `plugin` is an enabled Home row source giving answers by `method`.
    pub(super) async fn gives(&self, plugin: &PluginId, method: HomeRowMethod) -> bool {
        let gives =
            (self.points).read_provider(plugin, |source: &dyn HomeRowSource| source.gives(method));
        gives.await.unwrap_or(false)
    }

    /// Deletes every User's answers of `adapter`'s, and their put-offs, by each method a Plugin
    /// stores them for no longer gives, as it was disabled or its version stopped serving that
    /// export, so its next enable, or a version giving them again, starts afresh.
    pub(super) async fn forget<A: Adapter>(&self, adapter: &A) -> Result<(), sqlx::Error> {
        let mut stored = Vec::new();
        for &method in adapter.methods() {
            // ponytail: reads every Plugin with a put-off or answer by `method` on each visit to
            // Home, bounded by Plugins; delete them as a Plugin is disabled if visits grow slow.
            let mut plugins: BTreeSet<PluginId> =
                self.put_off_plugins(method).await?.into_iter().collect();
            plugins.extend(adapter.plugins(method).await?);
            stored.extend(plugins.into_iter().map(|plugin| (plugin, method)));
        }
        let mut gone = Vec::new();
        for (plugin, method) in stored {
            if !self.gives(&plugin, method).await {
                gone.push((plugin, method));
            }
        }
        if gone.is_empty() {
            return Ok(());
        }
        let adapter = adapter.clone();
        self.db
            .write(move |tx| Box::pin(async move { delete(tx, &adapter, &gone).await }))
            .await
    }

    async fn refresh_now<A: Adapter>(&self, adapter: &A, scope: &Scope) {
        // Put off before the Plugin is asked, so one whose service is down isn't called on every
        // visit to home. A User's change to their Personal settings meanwhile deletes the put-off
        // (the triggers on `personal_settings`), as does one to their Watch State for a Plugin
        // reading it (those on `watch_states`), and `replace` then drops this answer, built from
        // what they changed, rather than store it as fresh.
        if let Err(err) = self.put_off(scope).await {
            let (plugin, user, noun) = (&scope.plugin, scope.user, A::NOUN);
            warn!(plugin = %plugin, user, "could not put off refreshing the Plugin's {noun}: {err}");
            return;
        }
        let Some((kept, fresh_for)) = adapter.ask(scope).await else {
            return;
        };
        let fresh_for = fresh_for.clamp(MIN_TTL, MAX_TTL);
        if let Err(err) = self.replace(adapter, scope, kept, fresh_for).await {
            let (plugin, user, noun) = (&scope.plugin, scope.user, A::NOUN);
            warn!(plugin = %plugin, user, "could not store the Plugin's {noun}: {err}");
        }
    }

    /// The Plugins whose answers by `method` for `user` are fresh.
    async fn fresh(
        &self,
        method: HomeRowMethod,
        user: Option<i64>,
    ) -> Result<HashSet<PluginId>, sqlx::Error> {
        let now = db::now();
        let plugins =
            match method {
                HomeRowMethod::Featured => sqlx::query_scalar!(
                    "SELECT plugin_id FROM featured_refreshes WHERE stale_at > ? AND user_id = ?",
                    now,
                    user
                )
                .fetch_all(self.db.reader())
                .await?,
                HomeRowMethod::Shared | HomeRowMethod::User => sqlx::query_scalar!(
                    "SELECT plugin_id FROM home_row_refreshes WHERE stale_at > ? AND user_id IS ?",
                    now,
                    user
                )
                .fetch_all(self.db.reader())
                .await?,
            };
        Ok(plugins.into_iter().map(PluginId::from).collect())
    }

    /// The Plugins with a put-off by `method`, any User's.
    async fn put_off_plugins(&self, method: HomeRowMethod) -> Result<Vec<PluginId>, sqlx::Error> {
        let plugins = match method {
            HomeRowMethod::Featured => {
                sqlx::query_scalar!("SELECT DISTINCT plugin_id FROM featured_refreshes")
                    .fetch_all(self.db.reader())
                    .await?
            }
            HomeRowMethod::Shared | HomeRowMethod::User => {
                let users = method == HomeRowMethod::User;
                sqlx::query_scalar!(
                    "SELECT DISTINCT plugin_id FROM home_row_refreshes
                     WHERE (user_id IS NOT NULL) = ?",
                    users
                )
                .fetch_all(self.db.reader())
                .await?
            }
        };
        Ok(plugins.into_iter().map(PluginId::from).collect())
    }

    /// Keeps `scope`'s answer as it is, none before its first refresh, and refreshes it no
    /// sooner than [`MIN_TTL`] from now.
    async fn put_off(&self, scope: &Scope) -> Result<(), sqlx::Error> {
        let (plugin, method, user) = (scope.plugin.as_str().to_owned(), scope.method, scope.user);
        let stale_at = stale_after(MIN_TTL);
        self.db
            .write(move |tx| {
                Box::pin(async move {
                    match method {
                        HomeRowMethod::Featured => sqlx::query!(
                            "INSERT INTO featured_refreshes (plugin_id, user_id, stale_at)
                             VALUES (?, ?, ?)
                             ON CONFLICT (plugin_id, user_id) DO UPDATE SET stale_at = excluded.stale_at",
                            plugin,
                            user,
                            stale_at
                        )
                        .execute(&mut **tx)
                        .await?,
                        HomeRowMethod::Shared | HomeRowMethod::User => sqlx::query!(
                            "INSERT INTO home_row_refreshes (plugin_id, user_id, stale_at)
                             VALUES (?, ?, ?)
                             ON CONFLICT (plugin_id, coalesce(user_id, 0))
                             DO UPDATE SET stale_at = excluded.stale_at",
                            plugin,
                            user,
                            stale_at
                        )
                        .execute(&mut **tx)
                        .await?,
                    };
                    Ok(())
                })
            })
            .await
    }

    /// Replaces `scope`'s answer with `kept`, fresh for `fresh_for`, in one write. Stores nothing
    /// once the put-off its refresh started with is gone, as the Plugin was forgotten or the User
    /// changed their Personal settings, or their Watch State for a Plugin reading it, meanwhile.
    async fn replace<A: Adapter>(
        &self,
        adapter: &A,
        scope: &Scope,
        kept: A::Kept,
        fresh_for: Duration,
    ) -> Result<(), sqlx::Error> {
        let (adapter, scope) = (adapter.clone(), scope.clone());
        let stale_at = stale_after(fresh_for);
        self.db
            .write(move |tx| {
                Box::pin(
                    async move { replace_if_put_off(tx, &adapter, &scope, kept, stale_at).await },
                )
            })
            .await
    }
}

/// [`Refreshes::replace`]'s write: moves `scope`'s put-off to `stale_at` and, only if it was still
/// stored, has `adapter` store `kept`.
async fn replace_if_put_off<A: Adapter>(
    conn: &mut SqliteConnection,
    adapter: &A,
    scope: &Scope,
    kept: A::Kept,
    stale_at: i64,
) -> Result<(), sqlx::Error> {
    let plugin = scope.plugin.as_str();
    let renewed =
        match scope.method {
            HomeRowMethod::Featured => sqlx::query!(
                "UPDATE featured_refreshes SET stale_at = ? WHERE plugin_id = ? AND user_id = ?",
                stale_at,
                plugin,
                scope.user
            )
            .execute(&mut *conn)
            .await?,
            HomeRowMethod::Shared | HomeRowMethod::User => sqlx::query!(
                "UPDATE home_row_refreshes SET stale_at = ? WHERE plugin_id = ? AND user_id IS ?",
                stale_at,
                plugin,
                scope.user
            )
            .execute(&mut *conn)
            .await?,
        };
    if renewed.rows_affected() == 0 {
        return Ok(());
    }
    adapter.store(conn, scope, kept).await
}

/// [`Refreshes::forget`]'s write: deletes the put-offs and `adapter`'s answers that `gone` names,
/// each by a Plugin and the method it no longer gives.
async fn delete<A: Adapter>(
    conn: &mut SqliteConnection,
    adapter: &A,
    gone: &[(PluginId, HomeRowMethod)],
) -> Result<(), sqlx::Error> {
    for (plugin, method) in gone {
        delete_put_offs(conn, plugin.as_str(), *method).await?;
        adapter.delete(conn, plugin.as_str(), *method).await?;
    }
    Ok(())
}

/// Deletes the Plugin `plugin`'s put-offs by `method`, every User's for one per User.
async fn delete_put_offs(
    conn: &mut SqliteConnection,
    plugin: &str,
    method: HomeRowMethod,
) -> Result<(), sqlx::Error> {
    match method {
        HomeRowMethod::Featured => {
            sqlx::query!("DELETE FROM featured_refreshes WHERE plugin_id = ?", plugin)
                .execute(conn)
                .await?
        }
        HomeRowMethod::Shared | HomeRowMethod::User => {
            let users = method == HomeRowMethod::User;
            sqlx::query!(
                "DELETE FROM home_row_refreshes WHERE plugin_id = ? AND (user_id IS NOT NULL) = ?",
                plugin,
                users
            )
            .execute(conn)
            .await?
        }
    };
    Ok(())
}

/// When something fresh for `fresh_for` from now goes stale, in seconds since the Unix epoch.
fn stale_after(fresh_for: Duration) -> i64 {
    db::now().saturating_add(i64::try_from(fresh_for.as_secs()).unwrap_or(0))
}

/// A scope's place in [`Refreshes::running`], given up when dropped, even by a refresh cancelled
/// as its Plugin is disabled.
struct Running {
    set: Arc<Mutex<HashSet<Scope>>>,
    scope: Scope,
}

impl Running {
    /// Takes `scope`'s place in `set`, or `None` if a refresh holds it.
    fn start(set: &Arc<Mutex<HashSet<Scope>>>, scope: Scope) -> Option<Self> {
        let taken = lock(set).insert(scope.clone());
        taken.then(|| Self {
            set: Arc::clone(set),
            scope,
        })
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        lock(&self.set).remove(&self.scope);
    }
}

#[cfg(test)]
impl Refreshes {
    /// Waits until no refresh is running.
    pub(super) async fn settled(&self) {
        let started = std::time::Instant::now();
        while !lock(&self.running).is_empty() {
            assert!(
                started.elapsed() < Duration::from_secs(10),
                "still refreshing"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
}

#[cfg(test)]
#[path = "refreshes_tests.rs"]
mod tests;
