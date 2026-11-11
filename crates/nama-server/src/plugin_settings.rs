//! The Server's Plugins as stored: whether each is enabled, and its settings, kept in the
//! database. They come from the config's `[plugins.*]` tables once, on the first start; from then
//! on the database is the only store. [`PluginSettings::start`] enables the Plugins stored as on,
//! and every later change to one goes through [`PluginSettings`] (the crate's `AGENTS.md`).

use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use nama_plugin_api::{
    Manifest, PluginId,
    auth::Authenticator,
    settings::{Choices, SettingKind, SettingValue, Settings, SettingsError, SettingsSchema},
};
use nama_proto::nama::v1::{PluginKind, Quality};
use thiserror::Error;
use tokio::{
    sync::{Mutex, MutexGuard, mpsc},
    task::{self, JoinError, JoinHandle},
};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use crate::{
    config::{Config, InvalidPluginTable, PluginConfig},
    db::Database,
    plugins::{
        CheckError, EnableError, LastProvider, Listed, Registry, installed, log_not_enabled,
    },
    qualities,
};

/// The `schema_meta` key whose presence records that the config was imported.
const IMPORTED_KEY: &str = "plugin_settings_imported";

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
    /// The settings don't fit the Plugin's schema.
    #[error("the settings don't fit the plugin: {0}")]
    InvalidSettings(SettingsError),
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
    /// The stored settings couldn't be read.
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
    /// the Plugins have started.
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
        // Unbounded: startup and `enable_again` send from sync code, and a dropped id would leave
        // its Fulfiller's Requests waiting until it is enabled again.
        let (came_up, came_up_reader) = mpsc::unbounded_channel();
        let plugins = Self {
            db,
            registry,
            data_dir: config.data_dir.clone(),
            changing: Arc::default(),
            saving_crashed: Arc::default(),
            stop_saving_crashed: CancellationToken::new(),
            came_up,
            came_up_reader: Arc::new(std::sync::Mutex::new(Some(came_up_reader))),
        };
        let starting = plugins.clone();
        // Blocking: reading an Installed Plugin compiles it, and so does enabling it.
        let settings = task::spawn_blocking(move || {
            starting.registry.load_installed(&starting.data_dir);
            starting.enable_stored(&settings);
            settings
        })
        .await
        .map_err(StartError::Panicked)?;
        let has_authenticator = plugins
            .registry
            .call_provider(|_: Arc<dyn Authenticator>| async {})
            .await
            .is_some();
        if !has_authenticator {
            return Err(StartError::NoAuthenticator);
        }
        if importing {
            import(&plugins.db, &settings)
                .await
                .map_err(StartError::Import)?;
            info!(imported = ?in_config, "imported plugin settings from the config");
        }
        let crashed = plugins.registry.crashed();
        let saving_crashed = task::spawn(plugins.clone().save_crashed(crashed));
        *lock_ignoring_poison(&plugins.saving_crashed) = Some(saving_crashed);
        Ok(plugins)
    }

    /// Stops saving the Plugins disabled for crashing, once those that crashed before it are
    /// saved. One that crashes after those are saved may run again after a restart.
    pub async fn shutdown(&self) {
        let Some(saving_crashed) = lock_ignoring_poison(&self.saving_crashed).take() else {
            return;
        };
        self.stop_saving_crashed.cancel();
        // The task is never aborted, so it only fails if it panicked.
        if let Err(err) = saving_crashed.await {
            error!("saving the Plugins disabled for crashing panicked: {err}");
        }
    }

    /// Saves each Plugin the registry disables for crashing as disabled, as an admin's disable is,
    /// so it stays disabled across a restart until an admin enables it again. Runs until
    /// [`PluginSettings::shutdown`] and the crashes queued are saved, or until the registry hands a
    /// later caller of [`Registry::crashed`] the crashes instead.
    async fn save_crashed(self, mut crashed: mpsc::UnboundedReceiver<PluginId>) {
        loop {
            tokio::select! {
                // Stopping first, so a stop always saves the queue through the one drain below.
                biased;
                () = self.stop_saving_crashed.cancelled() => break,
                id = crashed.recv() => match id {
                    Some(id) => self.save_crashed_plugin(&id).await,
                    None => return,
                },
            }
        }
        while let Ok(id) = crashed.try_recv() {
            self.save_crashed_plugin(&id).await;
        }
    }

    async fn save_crashed_plugin(&self, id: &PluginId) {
        let _changing = self.lock().await;
        // An admin may have enabled it again since.
        if self.registry.enabled().contains(id) {
            return;
        }
        if let Err(err) = set_enabled(&self.db, id.as_str(), false).await {
            error!(plugin = %id, "could not save that the crashed plugin is disabled: {err}");
        }
    }

    /// Enables each Plugin the registry knows that `settings` store as enabled, or that starts
    /// enabled if they store nothing for it.
    fn enable_stored(&self, settings: &BTreeMap<String, PluginConfig>) {
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
            match self
                .registry
                .enable_by_id(&id, Settings::new(config.settings))
            {
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

    /// The registry the Plugins are enabled in, which the Kernel calls them through.
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
        let found = task::spawn_blocking(move || installed::read(&data_dir)).await?;
        // Under `changing`, so a Plugin being enabled can't lose its listing halfway.
        let changing = self.lock().await;
        self.registry.list_installed(found);
        drop(changing);
        let mut stored = self.stored().await?;
        Ok(self
            .registry
            .plugins()
            .into_iter()
            .map(|plugin| with_stored(plugin, &mut stored))
            .collect())
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

    /// Saves `sent` as the Plugin `id`'s settings, a secret left out keeping its stored value, then
    /// applies them if it is stored as enabled, so a restart retries settings that fail to enable
    /// it (ADR 0028). Returns the Plugin as it is now, enabled only if running, with what is
    /// stored for it.
    ///
    /// # Errors
    ///
    /// If no Plugin has the id or the Server refused it, the settings don't fit its schema, its
    /// settings check fails or doesn't offer a choice setting's value, or the database can't be
    /// read or written. A refused change saves nothing.
    pub async fn update(
        &self,
        id: &PluginId,
        sent: BTreeMap<String, SettingValue>,
    ) -> Result<StoredPlugin, ChangeError> {
        let (this, id) = (self.clone(), id.clone());
        detached(async move {
            let _changing = this.lock().await;
            let (manifest, StoredPlugin { plugin, stored }) = this.listed(&id).await?;
            let settings = with_stored_secrets(&manifest.settings, sent, &stored.settings);
            Settings::new(settings.clone())
                .resolve(&manifest.settings)
                .map_err(ChangeError::InvalidSettings)?;
            // ponytail: checked under `changing`, so a slow outside service holds up other Plugin
            // changes for up to SETTINGS_CHECK_TIMEOUT; check before locking if that bites.
            this.check_choices(&manifest, &settings).await?;
            save(&this.db, id.as_str(), stored.enabled, &settings).await?;
            if stored.enabled {
                this.enable_again(&id, &settings);
            }
            let enabled = this.registry.enabled().contains(&id);
            Ok(StoredPlugin {
                plugin: Listed { enabled, ..plugin },
                stored: PluginConfig {
                    enabled: stored.enabled,
                    settings,
                },
            })
        })
        .await
    }

    /// Enables the Plugin `id` with its stored settings, then saves it as enabled, so a refused
    /// enable saves nothing (ADR 0028). Enabling one already running saves it again, which is how
    /// a retry fixes a save that failed.
    ///
    /// # Errors
    ///
    /// If no Plugin has the id or the Server refused it, it can't be enabled, or the database can't
    /// be read or written. Should the save fail, the Plugin stays enabled until a restart.
    pub async fn enable(&self, id: &PluginId) -> Result<(), ChangeError> {
        let (this, id) = (self.clone(), id.clone());
        detached(async move {
            let _changing = this.lock().await;
            let (_, StoredPlugin { stored, .. }) = this.listed(&id).await?;
            match this
                .registry
                .enable_by_id(&id, Settings::new(stored.settings))
            {
                Ok(()) | Err(EnableError::AlreadyEnabled(_)) => {}
                Err(err) => return Err(ChangeError::Enable(err)),
            }
            set_enabled(&this.db, id.as_str(), true).await?;
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
            this.registry.disable(&id);
            Ok(())
        })
        .await
    }

    /// Enables the Plugin `id` again with `settings`, even if it isn't running, such as TMDB left
    /// disabled without its key. Disabled first, so nothing it registered with its old settings
    /// stays; a failure is logged, since its settings are saved and a restart retries them.
    fn enable_again(&self, id: &PluginId, settings: &BTreeMap<String, SettingValue>) {
        self.registry.disable(id);
        match self
            .registry
            .enable_by_id(id, Settings::new(settings.clone()))
        {
            Ok(()) => self.queue_came_up(id),
            Err(err) => {
                error!(plugin = %id, "could not enable the plugin with its new settings: {err}");
            }
        }
    }

    /// Runs the Plugin `id`'s settings check on `sent`, a secret left out keeping its stored value,
    /// and returns the choices it offers. Saves nothing.
    ///
    /// # Errors
    ///
    /// If no Plugin has the id or the Server refused it, the settings don't fit, the check fails
    /// or times out, or the database can't be read.
    pub async fn check(
        &self,
        id: &PluginId,
        sent: BTreeMap<String, SettingValue>,
    ) -> Result<Choices, ChangeError> {
        let (manifest, StoredPlugin { stored, .. }) = self.listed(id).await?;
        let settings = with_stored_secrets(&manifest.settings, sent, &stored.settings);
        self.run_check(id, settings, None).await
    }

    /// Runs the Plugin `id`'s settings check on its stored settings, and returns the choices it
    /// offers.
    async fn check_stored(&self, id: &PluginId) -> Result<Choices, ChangeError> {
        let (_, StoredPlugin { stored, .. }) = self.listed(id).await?;
        self.run_check(id, stored.settings, None).await
    }

    /// The Plugin `id`'s Qualities, in the order the admin listed them.
    pub(crate) async fn qualities(&self, id: &PluginId) -> Result<Vec<Quality>, ChangeError> {
        self.listed(id).await?;
        let mut conn = self.db.reader().acquire().await?;
        Ok(qualities::list(&mut conn, id).await?)
    }

    /// Replaces the Plugin `id`'s Qualities with `listed` and returns them as saved, or why
    /// [`qualities::check`] refused them against the profiles its stored settings' check offers.
    pub(crate) async fn replace_qualities(
        &self,
        id: &PluginId,
        listed: Vec<Quality>,
    ) -> Result<Result<Vec<Quality>, qualities::Invalid>, ChangeError> {
        // Only a list with Qualities asks the settings check, so a Fulfiller that's down can still
        // have its list cleared.
        let offered = if listed.is_empty() {
            self.listed(id).await?;
            Choices::new()
        } else {
            self.check_stored(id).await?
        };
        if let Err(invalid) = qualities::check(&listed, &offered) {
            return Ok(Err(invalid));
        }
        Ok(Ok(qualities::replace(&self.db, id.clone(), listed).await?))
    }

    /// Runs the Plugin `id`'s settings check on `settings`, naming the choice `setting` a failed
    /// check refuses, if any.
    async fn run_check(
        &self,
        id: &PluginId,
        settings: BTreeMap<String, SettingValue>,
        setting: Option<&str>,
    ) -> Result<Choices, ChangeError> {
        self.registry
            .check_settings(id, Settings::new(settings))
            .await
            .map_err(|error| ChangeError::Check {
                plugin: id.clone(),
                error,
                setting: setting.map(str::to_owned),
            })
    }

    /// The Plugin `id`'s Manifest, and the Plugin with what is stored for it, unless no Plugin
    /// has the id or the Server refused it.
    async fn listed(&self, id: &PluginId) -> Result<(Manifest, StoredPlugin), ChangeError> {
        let found = self
            .get(id)
            .await?
            .ok_or_else(|| ChangeError::Unknown(id.clone()))?;
        match &found.plugin.manifest {
            Ok(manifest) => Ok((manifest.clone(), found)),
            Err(reason) => Err(ChangeError::Refused {
                plugin: id.clone(),
                reason: reason.clone(),
            }),
        }
    }

    /// Refuses `settings` for the Plugin `manifest` describes unless its settings check passes on
    /// them and offers each choice setting's value. The check runs only when some choice setting
    /// has a value.
    async fn check_choices(
        &self,
        manifest: &Manifest,
        settings: &BTreeMap<String, SettingValue>,
    ) -> Result<(), ChangeError> {
        let chosen: Vec<_> = manifest
            .settings
            .fields
            .iter()
            .filter(|field| field.kind == SettingKind::Choice)
            .filter_map(|field| match settings.get(&field.key) {
                Some(SettingValue::Text(value)) => Some((field.key.as_str(), value)),
                _ => None,
            })
            .collect();
        let Some(&(first, _)) = chosen.first() else {
            return Ok(());
        };
        let offered = self
            .run_check(&manifest.id, settings.clone(), Some(first))
            .await?;
        for (key, value) in chosen {
            let is_offered = offered
                .get(key)
                .is_some_and(|choices| choices.iter().any(|choice| choice.value == *value));
            if !is_offered {
                return Err(ChangeError::NotOffered {
                    plugin: manifest.id.clone(),
                    key: key.to_owned(),
                    value: value.clone(),
                });
            }
        }
        Ok(())
    }

    /// Takes the lock every change to a Plugin holds until it is saved.
    async fn lock(&self) -> MutexGuard<'_, ()> {
        self.changing.lock().await
    }

    async fn stored(&self) -> Result<BTreeMap<String, PluginConfig>, sqlx::Error> {
        Ok(stored(&self.db).await?.unwrap_or_default())
    }
}

/// Runs `change` in a task of its own that the caller awaits but can't cancel, so a client that
/// gives up can't stop it between saving and applying (ADR 0029).
async fn detached<T: Send + 'static>(
    change: impl Future<Output = Result<T, ChangeError>> + Send + 'static,
) -> Result<T, ChangeError> {
    // The task is never aborted, so it only fails if it panicked.
    task::spawn(change).await.unwrap_or_else(|err| {
        error!("changing a plugin panicked: {err}");
        Err(sqlx::Error::WorkerCrashed.into())
    })
}

/// The settings `sent` for a Plugin with `schema`: the ones it names, with a secret left out
/// keeping its `stored` value, so a form can replace a secret without showing it.
fn with_stored_secrets(
    schema: &SettingsSchema,
    mut sent: BTreeMap<String, SettingValue>,
    stored: &BTreeMap<String, SettingValue>,
) -> BTreeMap<String, SettingValue> {
    schema
        .fields
        .iter()
        .filter_map(|field| {
            let kept = || {
                let secret = field.kind == SettingKind::Secret;
                stored.get(&field.key).filter(|_| secret).cloned()
            };
            Some((field.key.clone(), sent.remove(&field.key).or_else(kept)?))
        })
        .collect()
}

// No code that could panic runs while it is held, so a poisoned lock's handle is still whole.
fn lock_ignoring_poison<T>(mutex: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// `plugin` with what `stored` holds for it, taken out of `stored`.
fn with_stored(plugin: Listed, stored: &mut BTreeMap<String, PluginConfig>) -> StoredPlugin {
    let stored = stored
        .remove(plugin.id.as_str())
        .unwrap_or_else(|| never_stored(plugin.kind));
    StoredPlugin { plugin, stored }
}

/// What a Plugin of `kind` with nothing stored starts as: no settings, and enabled only if it is
/// Built-in. An Installed Plugin waits for an admin to enable it.
fn never_stored(kind: PluginKind) -> PluginConfig {
    PluginConfig {
        enabled: kind == PluginKind::PLUGIN_KIND_BUILT_IN,
        settings: BTreeMap::new(),
    }
}

/// Every stored Plugin's settings, by Plugin id, or `None` until the first start imported the
/// config's tables.
///
/// # Errors
///
/// If the database can't be read, or holds settings that aren't a JSON object of setting values.
pub async fn stored(db: &Database) -> Result<Option<BTreeMap<String, PluginConfig>>, sqlx::Error> {
    let imported = sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM schema_meta WHERE key = ?) AS "imported!: bool""#,
        IMPORTED_KEY
    )
    .fetch_one(db.reader())
    .await?;
    if !imported {
        return Ok(None);
    }
    let rows = sqlx::query!(
        r#"SELECT plugin_id, enabled AS "enabled: bool", settings FROM plugin_settings"#
    )
    .fetch_all(db.reader())
    .await?;
    rows.into_iter()
        .map(|row| {
            let settings = serde_json::from_str(&row.settings)
                .map_err(|err| sqlx::Error::Decode(err.into()))?;
            let config = PluginConfig {
                enabled: row.enabled,
                settings,
            };
            Ok((row.plugin_id, config))
        })
        .collect::<Result<_, _>>()
        .map(Some)
}

/// Stores `plugins`, read from the config's `[plugins.*]` tables, as each Plugin's settings, and
/// records the import so [`stored`] returns them from now on. Fails if the import already ran.
async fn import(
    db: &Database,
    plugins: &BTreeMap<String, PluginConfig>,
) -> Result<(), sqlx::Error> {
    let rows = plugins
        .iter()
        .map(|(id, config)| {
            let settings = serde_json::to_string(&config.settings)?;
            Ok((id.clone(), config.enabled, settings))
        })
        .collect::<Result<Vec<_>, serde_json::Error>>()
        .map_err(|err| sqlx::Error::Encode(err.into()))?;
    db.write(move |tx| {
        Box::pin(async move {
            for (plugin_id, enabled, settings) in rows {
                sqlx::query!(
                    "INSERT INTO plugin_settings (plugin_id, enabled, settings) VALUES (?, ?, ?)",
                    plugin_id,
                    enabled,
                    settings
                )
                .execute(&mut **tx)
                .await?;
            }
            sqlx::query!(
                "INSERT INTO schema_meta (key, value) VALUES (?, 'yes')",
                IMPORTED_KEY
            )
            .execute(&mut **tx)
            .await?;
            Ok(())
        })
    })
    .await
}

/// Stores whether the Plugin `id` is enabled and its `settings`, replacing any stored.
async fn save(
    db: &Database,
    id: &str,
    enabled: bool,
    settings: &BTreeMap<String, SettingValue>,
) -> Result<(), sqlx::Error> {
    let plugin_id = id.to_owned();
    let settings =
        serde_json::to_string(settings).map_err(|err| sqlx::Error::Encode(err.into()))?;
    db.write(move |tx| {
        Box::pin(async move {
            sqlx::query!(
                "INSERT INTO plugin_settings (plugin_id, enabled, settings) VALUES (?, ?, ?)
                 ON CONFLICT (plugin_id) DO UPDATE
                 SET enabled = excluded.enabled, settings = excluded.settings",
                plugin_id,
                enabled,
                settings
            )
            .execute(&mut **tx)
            .await?;
            Ok(())
        })
    })
    .await
}

/// Saves whether the Plugin `plugin_id` is enabled, keeping its stored settings.
async fn set_enabled(db: &Database, plugin_id: &str, enabled: bool) -> Result<(), sqlx::Error> {
    let plugin_id = plugin_id.to_owned();
    db.write(move |tx| {
        Box::pin(async move {
            sqlx::query!(
                "INSERT INTO plugin_settings (plugin_id, enabled, settings) VALUES (?, ?, '{}')
                 ON CONFLICT (plugin_id) DO UPDATE SET enabled = excluded.enabled",
                plugin_id,
                enabled
            )
            .execute(&mut **tx)
            .await?;
            Ok(())
        })
    })
    .await
}
