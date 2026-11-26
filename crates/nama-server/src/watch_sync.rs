//! Watch sync's imports: every [`IMPORT_EVERY`], each enabled Watch sync Plugin is asked, for each
//! User who set any of its Personal settings, what changed in their Watch State on its tracker
//! since its last answer for them. Its answer is taken through Browse where newer than the
//! Server's own (ADR 0008), which tells no Plugin, so nothing it brings is scrobbled back. Telling
//! the Plugins of the Server's own changes is the Watch State owner's (`src/library/watch_sync.rs`).

use std::{sync::Arc, time::Duration};

use nama_plugin_api::{PluginError, PluginId, contract::ImportAsked, watch_sync::WatchSync};
use tokio::task::JoinHandle;
use tracing::{error, warn};

use crate::{
    browse::Browse,
    db::Database,
    library::{import_cursor, imported_watch_state, keep_import_cursor},
    plugins::{ExtensionPoints, without_secrets},
    users::{personal_settings_recipients, renew_personal_secrets},
};

/// How often each Plugin is asked, the first time as the Server starts.
const IMPORT_EVERY: Duration = Duration::from_mins(15);

/// Imports each Watch sync Plugin's changes to its Users' Watch State.
#[derive(Debug, Clone)]
pub(crate) struct WatchSyncImports {
    db: Arc<Database>,
    points: ExtensionPoints,
    browse: Browse,
}

impl WatchSyncImports {
    /// Imports into what `db` holds through the Plugins of `points`.
    pub(crate) fn new(db: Arc<Database>, points: ExtensionPoints) -> Self {
        Self {
            browse: Browse::new(Arc::clone(&db), points.clone()),
            db,
            points,
        }
    }

    /// Starts importing every [`IMPORT_EVERY`], until the task is aborted. Each Plugin's
    /// imports run in its scope, so disabling it or shutting down stops them.
    pub(crate) fn start(&self) -> JoinHandle<()> {
        tokio::spawn(self.clone().import_when_due())
    }

    #[expect(clippy::infinite_loop, reason = "it runs until its task is aborted")]
    async fn import_when_due(self) {
        // From the start, so a Server restarted more often than `IMPORT_EVERY` still imports.
        let mut ticks = tokio::time::interval(IMPORT_EVERY);
        loop {
            ticks.tick().await;
            // ponytail: a Plugin whose Users take over IMPORT_EVERY to ask gets a second round
            // alongside the first; skip a Plugin still importing once that happens.
            for plugin in self.points.providers::<dyn WatchSync>() {
                let importing = self.clone().import_each_user(plugin.clone());
                self.points.spawn_in(&plugin, importing);
            }
        }
    }

    /// Imports the changes of each User who set any of the Plugin `plugin`'s Personal settings,
    /// one at a time.
    async fn import_each_user(self, plugin: PluginId) {
        let recipients = match personal_settings_recipients(&self.db, &plugin).await {
            Ok(recipients) => recipients,
            Err(err) => {
                error!(plugin = %plugin, "could not read whose Watch State to import: {err}");
                return;
            }
        };
        for (user, personal_settings, secrets) in recipients {
            let asked = ImportAsked {
                personal_settings: personal_settings.into_iter().collect(),
                user_id: user.id,
                ..ImportAsked::default()
            };
            self.import(&plugin, asked, &secrets).await;
        }
    }

    /// Asks the Plugin `plugin` `asked`, from the User's cursor, and takes its answer, keeping the
    /// tokens it renewed, then its cursor once taken. A failure is logged, and the next import
    /// asks from the same cursor.
    async fn import(&self, plugin: &PluginId, asked: ImportAsked, secrets: &[String]) {
        let user = asked.user_id;
        let since = match import_cursor(&self.db, user, plugin).await {
            Ok(since) => since,
            Err(err) => {
                error!(plugin = %plugin, user, "could not read where to import from: {err}");
                return;
            }
        };
        let asked = ImportAsked { since, ..asked };
        let answer = (self.points)
            .ask(plugin, |sync: Arc<dyn WatchSync>| {
                let asked = &asked;
                async move {
                    (sync.import(asked).await)
                        .map_err(|err| PluginError::from(without_secrets(err.to_string(), secrets)))
                }
            })
            .await;
        let imported = match answer.map(|answer| answer.result) {
            Some(Ok(imported)) => imported,
            Some(Err(failure)) => {
                let error = failure.message;
                warn!(plugin = %plugin, user, error, "importing a User's Watch State failed");
                return;
            }
            // Disabled since, which cancels this task too.
            None => return,
        };
        // Before taking the changes, which may fail: the Plugin already spent the old tokens.
        let renewed = renew_personal_secrets(&self.db, user, plugin, imported.personal_settings);
        if let Err(err) = renewed.await {
            error!(plugin = %plugin, user, "keeping the tokens an import renewed failed: {err}");
        }
        let changes = imported.changes.iter();
        let changes = changes.filter_map(imported_watch_state).collect();
        if let Err(err) = self.browse.take_pulled(user, changes).await {
            error!(plugin = %plugin, user, "taking a User's imported Watch State failed: {err}");
            return;
        }
        if let Err(err) = keep_import_cursor(&self.db, user, plugin, imported.cursor).await {
            error!(plugin = %plugin, user, "keeping where the next import starts failed: {err}");
        }
    }
}

#[cfg(test)]
#[path = "watch_sync_tests.rs"]
mod tests;
