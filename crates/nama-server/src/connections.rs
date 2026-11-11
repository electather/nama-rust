//! Plugin Connections (ADR 0021): a User's identity at an outside service, such as their Telegram
//! chat id, kept by the Kernel for the Plugin that acts on it, since Plugins have no storage.
//!
//! A User connects through a link the Plugin's Connector makes. While they haven't finished with
//! it, the Kernel asks the Connector every [`POLL_EVERY`], for up to [`LIFETIME`]. Waiting links
//! live in memory only, so a restart cancels them.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::Duration,
};

use nama_plugin_api::{PluginId, connector::Connector};
use tokio::time::Instant;
use tracing::{error, warn};

use crate::{
    codes,
    db::{Database, now},
    plugins::Registry,
    users::{Permissions, User},
};

/// How long a link waits for its User to finish with it.
const LIFETIME: Duration = Duration::from_mins(10);

/// How often the Kernel asks a Connector whether a User finished with their link.
const POLL_EVERY: Duration = Duration::from_secs(2);

/// Whose link a code is: a User's, to a Plugin.
type Waiter = (i64, PluginId);

/// The Users' Plugin Connections, and the links they are waiting on. Clones share them.
#[derive(Clone)]
pub(crate) struct Connections {
    db: Arc<Database>,
    plugins: Registry,
    /// The one-time code of each link a User is waiting on. A link's poll stops once its code is
    /// gone from here: expired, replaced or disconnected.
    waiting: Arc<Mutex<HashMap<Waiter, String>>>,
}

/// Where a User's Plugin Connection is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum State {
    NotConnected,
    Waiting,
    Connected,
}

/// Why a User couldn't start connecting.
#[derive(Debug, thiserror::Error)]
pub(crate) enum StartError {
    #[error("no enabled Plugin with that id connects Users")]
    NoConnector,
    /// The Connector's reason, with the Plugin's secret settings blanked.
    #[error("the Plugin couldn't make the link: {0}")]
    Failed(String),
    #[error("could not generate a one-time code: {0}")]
    Random(#[from] getrandom::Error),
}

impl Connections {
    pub(crate) fn new(db: Arc<Database>, plugins: Registry) -> Self {
        Self {
            db,
            plugins,
            waiting: Arc::default(),
        }
    }

    /// Each enabled Plugin that connects Users, with where `user`'s connection to it is.
    pub(crate) async fn list(&self, user: i64) -> Result<Vec<(PluginId, State)>, sqlx::Error> {
        // Waiting first: a link that just connected stops waiting only once it is stored, so it
        // is never seen as neither.
        let waiting: Vec<PluginId> = self
            .lock()
            .keys()
            .filter(|(waiter, _)| *waiter == user)
            .map(|(_, plugin)| plugin.clone())
            .collect();
        let connected = sqlx::query_scalar!(
            "SELECT plugin_id FROM plugin_connections WHERE user_id = ?",
            user
        )
        .fetch_all(self.db.reader())
        .await?;
        Ok(self
            .plugins
            .providers::<dyn Connector>()
            .into_iter()
            .map(|plugin| {
                let state = if connected.iter().any(|id| id == plugin.as_str()) {
                    State::Connected
                } else if waiting.contains(&plugin) {
                    State::Waiting
                } else {
                    State::NotConnected
                };
                (plugin, state)
            })
            .collect())
    }

    /// Starts connecting `user` to `plugin`, replacing any link they are waiting on, and returns
    /// the link they open to finish. Polls the Plugin in the background until they do, or for
    /// [`LIFETIME`].
    pub(crate) async fn start(&self, user: i64, plugin: &PluginId) -> Result<String, StartError> {
        let code = codes::random_secret()?;
        let started = self
            .plugins
            .ask(plugin, |connector: Arc<dyn Connector>| {
                let code = &code;
                async move { connector.start_connection(code).await }
            })
            .await
            .ok_or(StartError::NoConnector)?
            .result
            .map_err(|failure| StartError::Failed(failure.message))?;
        let waiter = (user, plugin.clone());
        self.lock().insert(waiter.clone(), code.clone());
        tokio::spawn(self.clone().wait_for(waiter, code, started.pending));
        Ok(started.url)
    }

    /// Deletes `user`'s Plugin Connection to `plugin`, and stops waiting on their link to it.
    pub(crate) async fn delete(&self, user: i64, plugin: PluginId) -> Result<(), sqlx::Error> {
        let waiting = Arc::clone(&self.waiting);
        self.db
            .write(move |tx| {
                Box::pin(async move {
                    // On the one writer, so a link connecting meanwhile can't store it after this.
                    lock(&waiting).remove(&(user, plugin.clone()));
                    let plugin = plugin.as_str();
                    sqlx::query!(
                        "DELETE FROM plugin_connections WHERE user_id = ? AND plugin_id = ?",
                        user,
                        plugin
                    )
                    .execute(&mut **tx)
                    .await?;
                    Ok(())
                })
            })
            .await
    }

    /// Polls for the link `code` until its User finishes with it, then stores their Plugin
    /// Connection, or gives up after [`LIFETIME`].
    async fn wait_for(self, waiter: Waiter, code: String, pending: String) {
        if let Some(connection) = self.poll(&waiter, &code, &pending).await
            && let Err(err) = self.store(&waiter, &code, connection).await
        {
            error!(plugin = %waiter.1, "could not store a Plugin Connection: {err}");
        }
        // Under one lock, so a link started again meanwhile keeps its own code.
        let mut waiting = self.lock();
        if waiting.get(&waiter) == Some(&code) {
            waiting.remove(&waiter);
        }
    }

    /// The User's identity at the service once they finished with the link `code`, or `None` if
    /// it expired or stopped waiting first, or its Plugin went.
    async fn poll(&self, waiter: &Waiter, code: &str, pending: &str) -> Option<String> {
        let (_, plugin) = waiter;
        // A deadline checked between polls, not a timeout around them: a timeout would drop a
        // poll in flight, which is Plugin code, outside `contain` (`crates/nama-server/AGENTS.md`,
        // "Plugin code runs contained").
        let expires = Instant::now() + LIFETIME;
        loop {
            tokio::time::sleep(POLL_EVERY).await;
            if Instant::now() >= expires || !self.is_waiting(waiter, code) {
                return None;
            }
            match self.poll_once(plugin, pending).await? {
                Ok(Some(connection)) => return Some(connection),
                Ok(None) => {}
                Err(reason) => warn!(%plugin, reason, "a Connector couldn't say who connected"),
            }
        }
    }

    /// What `plugin`'s Connector says about the link `pending` came from, or `None` if the Plugin
    /// went.
    async fn poll_once(
        &self,
        plugin: &PluginId,
        pending: &str,
    ) -> Option<Result<Option<String>, String>> {
        let polled = self
            .plugins
            .ask(plugin, |connector: Arc<dyn Connector>| async move {
                connector.poll_connection(pending).await
            })
            .await?;
        Some(polled.result.map_err(|failure| failure.message))
    }

    /// Stores `connection` as the Plugin Connection of the User `waiter` names, unless their link
    /// `code` stopped waiting meanwhile, such as when they disconnected.
    async fn store(
        &self,
        waiter: &Waiter,
        code: &str,
        connection: String,
    ) -> Result<(), sqlx::Error> {
        let this = self.clone();
        let (waiter, code) = (waiter.clone(), code.to_owned());
        self.db
            .write(move |tx| {
                Box::pin(async move {
                    // On the one writer, so a disconnect stored before this wins.
                    let still_waiting = this.is_waiting(&waiter, &code);
                    let (user, plugin, now) = (waiter.0, waiter.1.as_str(), now());
                    sqlx::query!(
                        "INSERT INTO plugin_connections (user_id, plugin_id, connection, created_at)
                         SELECT ?, ?, ?, ? WHERE ?
                         ON CONFLICT (user_id, plugin_id) DO UPDATE
                         SET connection = excluded.connection, created_at = excluded.created_at",
                        user,
                        plugin,
                        connection,
                        now,
                        still_waiting
                    )
                    .execute(&mut **tx)
                    .await?;
                    Ok(())
                })
            })
            .await
    }

    /// Whether `waiter` is still waiting on the link `code`.
    fn is_waiting(&self, waiter: &Waiter, code: &str) -> bool {
        self.lock()
            .get(waiter)
            .is_some_and(|waiting| waiting == code)
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<Waiter, String>> {
        lock(&self.waiting)
    }
}

/// Each User with a Plugin Connection to `plugin`, with it.
pub(crate) async fn connected(
    db: &Database,
    plugin: &PluginId,
) -> Result<Vec<(User, String)>, sqlx::Error> {
    let plugin = plugin.as_str();
    let rows = sqlx::query!(
        r#"SELECT users.id AS "id!", users.display_username AS name,
                  users.permissions AS "permissions: Permissions", users.created_at,
                  plugin_connections.connection
           FROM plugin_connections JOIN users ON users.id = plugin_connections.user_id
           WHERE plugin_connections.plugin_id = ?"#,
        plugin
    )
    .fetch_all(db.reader())
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            let user = User {
                id: row.id,
                name: row.name,
                permissions: row.permissions,
                created_at: row.created_at,
            };
            (user, row.connection)
        })
        .collect())
}

/// Deletes `user`'s Plugin Connection to `plugin`, as when its Notifier found them gone, unless it
/// is no longer `connection`: one made again since is theirs.
pub(crate) async fn forget(
    db: &Database,
    user: i64,
    plugin: &PluginId,
    connection: String,
) -> Result<(), sqlx::Error> {
    let plugin = plugin.as_str().to_owned();
    db.write(move |tx| {
        Box::pin(async move {
            sqlx::query!(
                "DELETE FROM plugin_connections
                 WHERE user_id = ? AND plugin_id = ? AND connection = ?",
                user,
                plugin,
                connection
            )
            .execute(&mut **tx)
            .await?;
            Ok(())
        })
    })
    .await
}

fn lock(waiting: &Mutex<HashMap<Waiter, String>>) -> MutexGuard<'_, HashMap<Waiter, String>> {
    waiting.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use nama_plugin_api::{
        BoxFuture, Manifest, Plugin, PluginError, PointKey, Priority, Registrar,
        connector::ConnectionStarted,
    };

    use super::*;

    /// A Plugin whose Connector counts its polls, and nobody ever finishes with its links.
    struct NeverConnected(Arc<AtomicUsize>);

    impl Plugin for NeverConnected {
        fn manifest(&self) -> Manifest {
            Manifest {
                version: "1.0.0".to_owned(),
                extension_points: vec![PointKey::provide::<dyn Connector>()],
                ..Manifest::new(PluginId::from_static("chat"))
            }
        }

        fn register(&self, registrar: &mut Registrar) {
            let connector: Arc<dyn Connector> = Arc::new(Self(Arc::clone(&self.0)));
            registrar.provide(Priority::DEFAULT, connector);
        }
    }

    impl Connector for NeverConnected {
        fn start_connection<'a>(
            &'a self,
            code: &'a str,
        ) -> BoxFuture<'a, Result<ConnectionStarted, PluginError>> {
            Box::pin(async move {
                Ok(ConnectionStarted {
                    url: format!("https://chat.example.com/{code}"),
                    pending: code.to_owned(),
                    ..ConnectionStarted::default()
                })
            })
        }

        fn poll_connection<'a>(
            &'a self,
            _pending: &'a str,
        ) -> BoxFuture<'a, Result<Option<String>, PluginError>> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Box::pin(async { Ok(None) })
        }
    }

    #[tokio::test]
    async fn a_link_nobody_finishes_within_ten_minutes_expires_and_is_polled_no_more() {
        let data_dir = tempfile::tempdir().unwrap();
        let db = Arc::new(Database::open(data_dir.path()).await.unwrap());
        let polls = Arc::new(AtomicUsize::new(0));
        let plugins = Registry::new();
        plugins
            .enable(Arc::new(NeverConnected(Arc::clone(&polls))))
            .unwrap();
        let connections = Connections::new(db, plugins);
        let chat = PluginId::from_static("chat");
        // After opening the database, whose pool times out by the clock.
        tokio::time::pause();

        connections.start(1, &chat).await.unwrap();
        tokio::time::sleep(Duration::from_mins(9)).await;
        let polled_while_waiting = polls.load(Ordering::Relaxed);
        tokio::time::sleep(Duration::from_secs(61)).await;
        let polled_by_expiry = polls.load(Ordering::Relaxed);
        tokio::time::sleep(Duration::from_secs(60)).await;
        let polled_after = polls.load(Ordering::Relaxed);
        tokio::time::resume();

        assert!(polled_while_waiting > 0, "the link was never polled");
        assert_eq!(polled_after, polled_by_expiry, "polled after it expired");
        assert_eq!(
            connections.list(1).await.unwrap(),
            [(chat, State::NotConnected)]
        );
    }
}
