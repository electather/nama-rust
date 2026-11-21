//! The Server's Plugins as stored: whether each is enabled, and its settings, kept in the
//! database. They come from the config's `[plugins.*]` tables once, on the first start; from then
//! on the database is the only store. [`PluginSettings::start`] enables the Plugins stored as on,
//! and every later change to one goes through [`PluginSettings`] (the crate's `AGENTS.md`).

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
};

use nama_plugin_api::{
    PluginId,
    auth::Authenticator,
    automation::Permission,
    settings::{SettingValue, SettingsError},
};
use nama_proto::nama::v1::PluginKind;
use thiserror::Error;
use tokio::{
    sync::{Mutex, MutexGuard, mpsc},
    task::{self, JoinError, JoinHandle},
};
use tokio_util::sync::CancellationToken;
use tracing::{Instrument as _, error, info, warn};

use crate::{
    config::{Config, InvalidPluginTable, PluginConfig},
    db::Database,
    library::Contents,
    plugins::{
        CheckError, EnableError, ExtensionPoints, LastProvider, Listed, ListedJob, Registry,
        forget_undeclared_jobs, list_jobs, log_not_enabled, read_installed,
    },
};

mod check;
mod crashed;
mod had;
mod library_ids;
mod personal;
mod put_back;
mod qualities;
mod running;
mod sent;
mod stored;
mod title_sources;

pub(crate) use personal::Personal;
use sent::{check_fits, with_stored_secrets};
use stored::{
    builtin_grants, import, never_stored, save, save_builtin_grants, set_enabled,
    set_enabled_granting, with_stored,
};
pub use stored::{grants, stored};

/// The Plugin Permissions granted each stored Plugin, by Plugin id.
type Grants = BTreeMap<String, HashSet<Permission>>;

/// The Server's Plugins, started with what the database stores for each.
///
/// Cloning one gives another handle to the same Plugins. A change, once asked for, runs to the end
/// even if its caller stops waiting.
#[derive(Clone, Debug)]
pub struct PluginSettings {
    db: Arc<Database>,
    registry: Registry,
    /// Where the Installed Plugins are, in `plugins/`.
    data_dir: PathBuf,
    /// Held while a Plugin's settings change or it is enabled or disabled, and that is saved, so two
    /// changes at once can't leave it running otherwise than the database says.
    // ponytail: one lock for every Plugin; one per Plugin if Admins ever wait on each other.
    changing: Arc<Mutex<()>>,
    /// What each Plugin was last enabled with (`running.rs`), written under `changing` and by
    /// [`PluginSettings::start`], which enables the stored Plugins before any change can run.
    running: Arc<std::sync::Mutex<HashMap<PluginId, running::Ran>>>,
    /// The task saving each Plugin disabled for crashing as disabled, until
    /// [`PluginSettings::shutdown`].
    saving_crashed: Arc<std::sync::Mutex<Option<JoinHandle<()>>>>,
    /// Tells the task saving crashes to save those still queued and stop.
    stop_saving_crashed: CancellationToken,
    /// Takes the id of each Plugin enabled, startup's included, so Requests can hand it the
    /// Approved Requests waiting for it without this module calling Requests (ADR 0028).
    came_up: mpsc::UnboundedSender<PluginId>,
    /// The other end of `came_up`, until [`PluginSettings::take_came_up`] takes it.
    came_up_reader: Arc<std::sync::Mutex<Option<mpsc::UnboundedReceiver<PluginId>>>>,
}

/// A Plugin the Server has, with what is stored for it.
#[derive(Debug, Clone)]
pub struct StoredPlugin {
    /// The Plugin, as the registry lists it.
    pub plugin: Listed,
    /// What is stored for it, or for one never stored, what it starts as.
    pub stored: PluginConfig,
    /// The Plugin Permissions it holds, kept beside its settings: those an Admin granted an
    /// Installed Plugin (ADR 0049), or all a Built-in one requests (ADR 0079).
    pub granted: HashSet<Permission>,
}

/// Why the Server couldn't start its Plugins.
#[derive(Debug, Error)]
pub enum StartError {
    /// The stored settings couldn't be read.
    #[error("could not read the plugin settings: {0}")]
    Read(sqlx::Error),
    /// On the first start, a `[plugins.*]` table in the config isn't a Plugin's config.
    #[error("could not load config: {0}")]
    Config(#[from] InvalidPluginTable),
    /// Reading or enabling the Plugins panicked outside the Plugin code the registry contains.
    #[error("starting the plugins failed: {0}")]
    Panicked(JoinError),
    /// No enabled Plugin provides an [`Authenticator`], so nobody could sign in.
    #[error(
        "no Authenticator is enabled, so nobody could sign in: enable the local-auth plugin \
         (remove `enabled = false` from [plugins.local-auth] in the config), or another plugin \
         that provides one"
    )]
    NoAuthenticator,
    /// On the first start, the config's tables couldn't be imported.
    #[error("could not import the plugin settings: {0}")]
    Import(sqlx::Error),
    /// The Plugin Permissions granted the Built-in Plugins couldn't be saved.
    #[error("could not save the permissions granted the built-in plugins: {0}")]
    Grant(sqlx::Error),
}

/// Why a Plugin couldn't be changed or its settings checked.
#[derive(Debug, Error)]
pub enum ChangeError {
    /// No Plugin has the id.
    #[error("there is no plugin {0}")]
    Unknown(PluginId),
    /// The Server refused the Plugin, for the reason given.
    #[error("the Server refused plugin {plugin}: {reason}")]
    Refused {
        /// The Plugin.
        plugin: PluginId,
        /// Why the Server refused it.
        reason: String,
    },
    /// A Plugin Permission granted that the Plugin's Manifest doesn't request.
    #[error("{plugin} doesn't ask for the Plugin Permission {}", permission_name(*.permission))]
    NotRequested {
        /// The Plugin.
        plugin: PluginId,
        /// The Plugin Permission.
        permission: Permission,
    },
    /// The settings don't fit the Plugin's schema.
    #[error("the settings don't fit the plugin: {0}")]
    InvalidSettings(SettingsError),
    /// No enabled Plugin with the id declares Personal settings.
    #[error("no enabled Plugin {0} has Personal settings")]
    NoPersonalSettings(PluginId),
    /// A Libraries setting names an id no Library has.
    #[error("the setting {key} names no Library {id}")]
    UnknownLibrary {
        /// The Libraries setting.
        key: String,
        /// The first of its ids no Library has.
        id: i64,
    },
    /// A choice setting's value isn't one the Plugin's settings check offers.
    #[error("{value} isn't one of the choices for the setting {key}")]
    NotOffered {
        /// The Plugin.
        plugin: PluginId,
        /// The choice setting.
        key: String,
        /// Its value.
        value: String,
    },
    /// The Plugin's settings check failed or timed out.
    #[error("{error}")]
    Check {
        /// The Plugin.
        plugin: PluginId,
        /// Why it failed.
        error: CheckError,
        /// The choice setting a save was refused for, if any.
        setting: Option<String>,
    },
    /// The Plugin couldn't be enabled.
    #[error(transparent)]
    Enable(EnableError),
    /// Disabling the Plugin would leave no Authenticator, so nobody could sign in.
    #[error(transparent)]
    LastAuthenticator(LastProvider),
    /// The stored settings couldn't be read or written.
    #[error("could not read or save the plugin settings: {0}")]
    Database(#[from] sqlx::Error),
}

/// Why the Plugins couldn't be listed.
#[derive(Debug, Error)]
pub enum ListError {
    /// The stored settings, or a User's Personal settings, couldn't be read.
    #[error("could not read the plugin settings: {0}")]
    Read(#[from] sqlx::Error),
    /// Reading the Installed Plugins panicked.
    #[error("reading the Installed Plugins failed: {0}")]
    ReadInstalled(#[from] JoinError),
}

impl PluginSettings {
    /// Makes the Installed Plugins in `config`'s data directory known beside those `registry`
    /// knows, and enables each one stored as enabled, with its stored settings. On the first start
    /// nothing is stored yet, so it uses `config`'s `[plugins.*]` tables, and imports them once
    /// the Plugins have started. Each Built-in Plugin is granted the Plugin Permissions it requests,
    /// and saved with them in place of those stored, since it is compiled in (ADR 0079).
    ///
    /// A Plugin that fails to enable, such as one missing a required setting, is logged and left
    /// off.
    ///
    /// # Errors
    ///
    /// If the settings can't be read or imported, a `[plugins.*]` table isn't a Plugin's config,
    /// or no Plugin provides an [`Authenticator`], since then nobody could sign in. A refused
    /// start imports nothing, so a config the Server refuses can still be fixed in the file.
    pub async fn start(
        db: Arc<Database>,
        registry: Registry,
        config: &Config,
    ) -> Result<Self, StartError> {
        let stored = stored(&db).await.map_err(StartError::Read)?;
        let mut grants = grants(&db).await.map_err(StartError::Read)?;
        let builtin_grants = builtin_grants(&registry, &grants);
        grants.extend(builtin_grants.clone());
        let in_config: Vec<&str> = config.plugins.ids().collect();
        let importing = stored.is_none();
        let settings = match stored {
            Some(stored) => {
                if !in_config.is_empty() {
                    warn!(
                        ignored = ?in_config,
                        "ignoring the [plugins.*] tables in the config: plugin settings live in \
                         the database since they were imported"
                    );
                }
                stored
            }
            None => config.plugins.parse()?,
        };
        registry
            .record_job_runs_in(Arc::clone(&db))
            .await
            .map_err(StartError::Read)?;
        let plugins = Self::new(db, registry, config.data_dir.clone());
        let starting = plugins.clone();
        // Blocking: reading an Installed Plugin compiles it, and so does enabling it.
        let settings = task::spawn_blocking(move || {
            starting.registry.load_installed(&starting.data_dir);
            starting.enable_stored(&settings, &grants);
            settings
        })
        .await
        .map_err(StartError::Panicked)?;
        forget_undeclared_jobs(&plugins.db, &plugins.registry.plugins()).await;
        if plugins.registry.providers::<dyn Authenticator>().is_empty() {
            return Err(StartError::NoAuthenticator);
        }
        if importing {
            import(&plugins.db, &settings)
                .await
                .map_err(StartError::Import)?;
            info!(imported = ?in_config, "imported plugin settings from the config");
        }
        // After the import, which would otherwise find these Plugins' rows already stored.
        save_builtin_grants(&plugins.db, &builtin_grants)
            .await
            .map_err(StartError::Grant)?;
        plugins.note_had().await;
        let crashed = plugins.registry.crashed();
        let saving_crashed = task::spawn(plugins.clone().save_crashed(crashed));
        *lock_ignoring_poison(&plugins.saving_crashed) = Some(saving_crashed);
        Ok(plugins)
    }

    /// Plugin settings over the Plugins `registry` knows, none enabled from what is stored, nor
    /// any Installed Plugin read. Its data directory is the working directory, which
    /// [`PluginSettings::list`] would read Installed Plugins from.
    #[cfg(test)]
    pub(crate) fn for_tests(db: Arc<Database>, registry: Registry) -> Self {
        Self::new(db, registry, PathBuf::new())
    }

    fn new(db: Arc<Database>, registry: Registry, data_dir: PathBuf) -> Self {
        // Unbounded: startup and `enable_again` send from sync code, and a dropped id would leave
        // its Fulfiller's Requests waiting until it is enabled again.
        let (came_up, came_up_reader) = mpsc::unbounded_channel();
        Self {
            db,
            registry,
            data_dir,
            changing: Arc::default(),
            running: Arc::default(),
            saving_crashed: Arc::default(),
            stop_saving_crashed: CancellationToken::new(),
            came_up,
            came_up_reader: Arc::new(std::sync::Mutex::new(Some(came_up_reader))),
        }
    }

    /// Stops saving the Plugins disabled for crashing, once those that crashed before it are
    /// saved, then cancels every enabled Plugin's scope and waits for its tasks (ADR 0047). One
    /// that crashes after those are saved may run again after a restart.
    pub async fn shutdown(&self) {
        let Some(saving_crashed) = lock_ignoring_poison(&self.saving_crashed).take() else {
            return;
        };
        self.stop_saving_crashed.cancel();
        self.note_had().await;
        // The task is never aborted, so it only fails if it panicked.
        if let Err(err) = saving_crashed.await {
            error!("saving the Plugins disabled for crashing panicked: {err}");
        }
        self.registry.shutdown().await;
    }

    /// Enables each Plugin the registry knows that `settings` store as enabled, or that starts
    /// enabled if they store nothing for it.
    fn enable_stored(&self, settings: &BTreeMap<String, PluginConfig>, grants: &Grants) {
        let mut known = self.registry.plugins();
        for id in settings.keys() {
            if !known.iter().any(|plugin| plugin.id.as_str() == id) {
                warn!(plugin = %id, "settings name a plugin this Server doesn't have");
            }
        }
        // Built-in first: the first Plugin enabled for a kind of Fulfiller keeps it.
        known.sort_by_key(|plugin| plugin.kind != PluginKind::PLUGIN_KIND_BUILT_IN);
        for plugin in known.into_iter().filter(|plugin| plugin.manifest.is_ok()) {
            let id = plugin.id;
            let config = settings
                .get(id.as_str())
                .cloned()
                .unwrap_or_else(|| never_stored(plugin.kind));
            if !config.enabled {
                info!(plugin = %id, "plugin disabled in its settings");
                continue;
            }
            let granted = grants.get(id.as_str()).cloned().unwrap_or_default();
            match self.enable_with(&id, config.settings, granted) {
                Ok(()) => self.queue_came_up(&id),
                Err(err) => log_not_enabled(&id, &err),
            }
        }
        let enabled: Vec<String> = self
            .registry
            .enabled()
            .iter()
            .map(ToString::to_string)
            .collect();
        info!(?enabled, "enabled plugins");
    }

    /// The id of each Plugin enabled since [`PluginSettings::start`], startup's included, in order,
    /// and of each one enabled from now on. Only the first call gets them; a later one gets an
    /// empty, closed queue.
    pub(crate) fn take_came_up(&self) -> mpsc::UnboundedReceiver<PluginId> {
        lock_ignoring_poison(&self.came_up_reader)
            .take()
            .unwrap_or_else(|| mpsc::unbounded_channel().1)
    }

    /// Queues the Plugin `id`, just enabled, for [`PluginSettings::take_came_up`]'s reader.
    fn queue_came_up(&self, id: &PluginId) {
        // Fails only once the reader stopped, when the Server no longer serves.
        let _: Result<(), _> = self.came_up.send(id.clone());
    }

    /// How the rest of the Kernel calls the Plugins, which can't change them.
    #[must_use]
    pub fn extension_points(&self) -> ExtensionPoints {
        self.registry.extension_points()
    }

    /// The registry the Plugins are enabled in, for tests to check which are.
    #[cfg(feature = "testing")]
    #[must_use]
    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// Every Plugin the Server has, enabled or not, sorted by id, each with what is stored for it.
    /// Reads the Installed Plugins again first, so one an admin dropped in is listed.
    ///
    /// # Errors
    ///
    /// If the Installed Plugins or the stored settings can't be read.
    pub async fn list(&self) -> Result<Vec<StoredPlugin>, ListError> {
        let data_dir = self.data_dir.clone();
        // Compiled before taking `changing`, so a slow Plugin doesn't hold up Plugin changes.
        let found = task::spawn_blocking(move || read_installed(&data_dir)).await?;
        // Before as well as after, so a Plugin this listing drops was had until now.
        self.note_had().await;
        // Under `changing`, so a Plugin being enabled can't lose its listing halfway.
        let changing = self.lock().await;
        self.registry.list_installed(found);
        drop(changing);
        self.note_had().await;
        let mut stored = self.stored().await?;
        let plugins = self.registry.plugins();
        forget_undeclared_jobs(&self.db, &plugins).await;
        Ok(plugins
            .into_iter()
            .map(|plugin| with_stored(plugin, &mut stored))
            .collect())
    }

    /// The Jobs of every enabled Plugin, by Plugin id, each with when it next fires and its newest
    /// Job Run.
    ///
    /// # Errors
    ///
    /// If the Job Runs can't be read.
    pub async fn jobs(&self) -> Result<Vec<ListedJob>, sqlx::Error> {
        list_jobs(self.registry.plugins(), &self.db).await
    }

    /// Whether a Plugin has the id `id`, refused or not, as [`PluginSettings::get`] decides.
    #[must_use]
    pub fn has(&self, id: &PluginId) -> bool {
        self.registry.plugin(id).is_some()
    }

    /// The Plugin `id`, refused or not, with what is stored for it, or `None` if no Plugin has
    /// the id. Doesn't read the Installed Plugins again as [`PluginSettings::list`] does.
    ///
    /// # Errors
    ///
    /// If the stored settings can't be read.
    pub async fn get(&self, id: &PluginId) -> Result<Option<StoredPlugin>, sqlx::Error> {
        let Some(plugin) = self.registry.plugin(id) else {
            return Ok(None);
        };
        Ok(Some(with_stored(plugin, &mut self.stored().await?)))
    }

    /// Saves `sent` as the Plugin `id`'s settings, then applies them if it is stored as enabled, so
    /// a restart retries settings that fail to enable it (ADR 0028). A secret left out keeps its
    /// stored value unless `forgotten` names it, and a repeated Library id is kept once. One that
    /// fails to enable with them is put back as it ran, as [`PluginSettings::enable_granting`]
    /// does. Returns the Plugin as it is now, enabled only if running, with what is stored for it.
    ///
    /// # Errors
    ///
    /// If no Plugin has the id or the Server refused it, the settings don't fit its schema (even
    /// an integer's bounds, and a required setting only if it is stored as enabled) or name a
    /// Library that doesn't exist, its settings check fails or doesn't offer a choice's value, or
    /// the database fails. A refused change saves nothing.
    pub async fn update(
        &self,
        id: &PluginId,
        sent: BTreeMap<String, SettingValue>,
        forgotten: Vec<String>,
    ) -> Result<StoredPlugin, ChangeError> {
        let (this, id) = (self.clone(), id.clone());
        detached(async move {
            let _changing = this.lock().await;
            let (
                manifest,
                StoredPlugin {
                    plugin,
                    mut stored,
                    granted,
                },
            ) = this.listed(&id).await?;
            stored.settings.retain(|key, _| !forgotten.contains(key));
            let mut settings = with_stored_secrets(&manifest.settings, sent, &stored.settings);
            library_ids::dedupe_library_ids(&manifest.settings, &mut settings);
            check_fits(&manifest.settings, &settings, stored.enabled)
                .map_err(ChangeError::InvalidSettings)?;
            this.check_library_ids(&manifest.settings, &settings)
                .await?;
            // ponytail: checked under `changing`, so a slow outside service holds up other Plugin
            // changes for up to SETTINGS_CHECK_TIMEOUT; check before locking if that bites.
            this.check_choices(&manifest, &settings).await?;
            save(&this.db, id.as_str(), stored.enabled, &settings).await?;
            let stored_enabled = if stored.enabled {
                let ran = this.ran_with(&id);
                !this.enable_again(&id, &settings, &granted, ran).await
            } else {
                false
            };
            let enabled = this.registry.enabled().contains(&id);
            Ok(StoredPlugin {
                plugin: Listed { enabled, ..plugin },
                stored: PluginConfig {
                    enabled: stored_enabled,
                    settings,
                },
                granted,
            })
        })
        .await
    }

    /// [`PluginSettings::enable_granting`] with no Plugin Permissions, which a Built-in Plugin
    /// ignores.
    ///
    /// # Errors
    ///
    /// As [`PluginSettings::enable_granting`].
    pub async fn enable(&self, id: &PluginId) -> Result<(), ChangeError> {
        self.enable_granting(id, HashSet::new()).await
    }

    /// Enables the Plugin `id` with its stored settings, granted the Plugin Permissions `granted`
    /// in place of those granted before, then saves it as enabled with them, so a refused enable
    /// saves nothing (ADR 0028, 0049). One already running with other grants is enabled again with
    /// these. A Built-in Plugin ignores `granted` and holds every Plugin Permission it requests
    /// (ADR 0079). Enabling one already running saves it again, which is how a retry fixes a save
    /// that failed.
    ///
    /// # Errors
    ///
    /// If no Plugin has the id or the Server refused it, `granted` names a Permission its Manifest
    /// doesn't request, it can't be enabled, or the database can't be read or written. When
    /// enabling or saving fails, the Plugin is put back as it ran before: with the settings and
    /// grants it ran with, or not at all; should it not enable with those either, it is saved as
    /// disabled, so what runs and what is stored agree whatever fails.
    pub async fn enable_granting(
        &self,
        id: &PluginId,
        granted: HashSet<Permission>,
    ) -> Result<(), ChangeError> {
        let (this, id) = (self.clone(), id.clone());
        detached(async move {
            let _changing = this.lock().await;
            let (manifest, StoredPlugin { plugin, stored, .. }) = this.listed(&id).await?;
            let granted = if plugin.kind == PluginKind::PLUGIN_KIND_BUILT_IN {
                manifest.permissions.iter().copied().collect()
            } else {
                granted
            };
            if let Some(&permission) = granted
                .iter()
                .find(|permission| !manifest.permissions.contains(permission))
            {
                return Err(ChangeError::NotRequested {
                    plugin: id,
                    permission,
                });
            }
            // What it runs with, not what is stored, which a failed settings change makes differ.
            let ran = this.ran_with(&id);
            let unchanged = ran.as_ref().is_some_and(|ran| ran.granted == granted);
            if !unchanged && ran.is_some() {
                // Disabled first, as `enable_again` does, so nothing it registered with its old
                // grants stays.
                this.registry.disable(&id).await;
            }
            match this.enable_with(&id, stored.settings, granted.clone()) {
                Ok(()) | Err(EnableError::AlreadyEnabled(_)) => {}
                // Unchanged, it was never disabled and runs on as it ran.
                Err(err) if unchanged => return Err(ChangeError::Enable(err)),
                Err(err) => {
                    let err = ChangeError::Enable(err);
                    this.put_back(&id, ran, &err).await;
                    return Err(err);
                }
            }
            let saved = set_enabled_granting(&this.db, id.as_str(), &granted).await;
            // Unchanged, it runs as before and as stored, unless an earlier save failed, which
            // this retry then fixes.
            if let Err(err) = &saved
                && !unchanged
            {
                this.put_back(&id, ran, err).await;
            }
            saved?;
            // Queued even if it was running, so enabling it again retries a failed hand-off.
            this.queue_came_up(&id);
            Ok(())
        })
        .await
    }

    /// Saves the Plugin `id` as disabled, then disables it, so should the save fail it keeps
    /// running as stored (ADR 0028).
    ///
    /// # Errors
    ///
    /// If no Plugin has the id or the Server refused it, it may be the last enabled Plugin that
    /// provides an [`Authenticator`], since then nobody could sign in, or the database can't be
    /// read or written. A refused disable saves nothing.
    pub async fn disable(&self, id: &PluginId) -> Result<(), ChangeError> {
        let (this, id) = (self.clone(), id.clone());
        detached(async move {
            let _changing = this.lock().await;
            this.listed(&id).await?;
            if let Some(last) = this.registry.last_provider::<dyn Authenticator>(&id) {
                return Err(ChangeError::LastAuthenticator(last));
            }
            set_enabled(&this.db, id.as_str(), false).await?;
            // ponytail: an Installed Authenticator crashing since the check can leave none
            // running, as a crash can anyway; the next start then refuses until an admin enables
            // one.
            this.registry.disable(&id).await;
            this.forget_similar(&id).await;
            Ok(())
        })
        .await
    }

    /// Deletes the Similar titles the Plugin `id` answered, as disabling it does (ADR 0084), so
    /// enabling it again shows them only as its titles refresh. Pages already leave out a disabled
    /// Plugin's, so a failure is only logged.
    async fn forget_similar(&self, id: &PluginId) {
        let contents = Contents::new(Arc::clone(&self.db));
        if let Err(err) = contents.forget_similar_from(id).await {
            error!(plugin = %id, %err, "couldn't delete a disabled Plugin's Similar titles");
        }
    }

    /// Enables the Plugin `id` again with `settings` and the Plugin Permissions `granted`, even if
    /// it isn't running, such as TMDB left disabled without its key. Disabled first, so nothing it
    /// registered with its old settings stays. A failure is logged, since its settings are saved
    /// and a restart retries them, and the Plugin is put back as it ran, `ran`. Returns whether
    /// putting it back saved it disabled.
    async fn enable_again(
        &self,
        id: &PluginId,
        settings: &BTreeMap<String, SettingValue>,
        granted: &HashSet<Permission>,
        ran: Option<running::Ran>,
    ) -> bool {
        self.registry.disable(id).await;
        match self.enable_with(id, settings.clone(), granted.clone()) {
            Ok(()) => {
                self.queue_came_up(id);
                false
            }
            Err(err) => {
                error!(plugin = %id, "could not enable the plugin with its new settings: {err}");
                self.put_back(id, ran, &err).await
            }
        }
    }

    /// Takes the lock every change to a Plugin holds until it is saved.
    async fn lock(&self) -> MutexGuard<'_, ()> {
        self.changing.lock().await
    }

    async fn stored(&self) -> Result<(BTreeMap<String, PluginConfig>, Grants), sqlx::Error> {
        let settings = stored(&self.db).await?.unwrap_or_default();
        Ok((settings, grants(&self.db).await?))
    }
}

/// Runs `change` in a task of its own that the caller awaits but can't cancel, so a client that
/// gives up can't stop it between saving and applying (ADR 0029).
async fn detached<T: Send + 'static>(
    change: impl Future<Output = Result<T, ChangeError>> + Send + 'static,
) -> Result<T, ChangeError> {
    // The task is never aborted, so it only fails if it panicked.
    task::spawn(change.in_current_span())
        .await
        .unwrap_or_else(|err| {
            error!("changing a plugin panicked: {err}");
            Err(sqlx::Error::WorkerCrashed.into())
        })
}

// No code that could panic runs while it is held, so a poisoned lock's handle is still whole.
fn lock_ignoring_poison<T>(mutex: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// `permission`'s name, such as `PERMISSION_STORE`. The migration's `plugins_reading_watch_states`
/// view finds Plugins granted Read Watch States by it.
fn permission_name(permission: Permission) -> &'static str {
    buffa::Enumeration::proto_name(&permission)
}
