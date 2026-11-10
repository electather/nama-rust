//! The Server's Plugins as stored: whether each is enabled, and its settings, kept in the
//! database. They come from the config's `[plugins.*]` tables once, on the first start; from then
//! on the database is the only store. [`PluginSettings::start`] enables the Plugins stored as on.

use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use nama_plugin_api::{
    PluginId,
    auth::Authenticator,
    settings::{SettingValue, Settings},
};
use nama_proto::nama::v1::PluginKind;
use thiserror::Error;
use tokio::{
    sync::{Mutex, MutexGuard},
    task::{self, JoinError},
};
use tracing::{info, warn};

use crate::{
    config::{Config, InvalidPluginTable, PluginConfig},
    db::Database,
    plugins::{Listed, Registry, installed, log_not_enabled},
};

/// The `schema_meta` key whose presence records that the config was imported.
const IMPORTED_KEY: &str = "plugin_settings_imported";

/// The Server's Plugins, started with what the database stores for each.
///
/// Cloning one gives another handle to the same Plugins.
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
        let plugins = Self {
            db,
            registry,
            data_dir: config.data_dir.clone(),
            changing: Arc::default(),
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
        Ok(plugins)
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
        for plugin in known
            .into_iter()
            .filter(|plugin| plugin.refused_reason.is_none())
        {
            let id = plugin.id;
            let config = settings
                .get(id.as_str())
                .cloned()
                .unwrap_or_else(|| never_stored(plugin.kind));
            if !config.enabled {
                info!(plugin = %id, "plugin disabled in its settings");
                continue;
            }
            if let Err(err) = self
                .registry
                .enable_by_id(&id, Settings::new(config.settings))
            {
                log_not_enabled(&id, &err);
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
        let Some(plugin) = self
            .registry
            .plugins()
            .into_iter()
            .find(|plugin| plugin.id == *id)
        else {
            return Ok(None);
        };
        Ok(Some(with_stored(plugin, &mut self.stored().await?)))
    }

    /// Takes the lock every change to a Plugin holds until it is saved.
    pub(crate) async fn lock(&self) -> MutexGuard<'_, ()> {
        self.changing.lock().await
    }

    async fn stored(&self) -> Result<BTreeMap<String, PluginConfig>, sqlx::Error> {
        Ok(stored(&self.db).await?.unwrap_or_default())
    }
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
///
/// # Errors
///
/// If the database can't be written.
pub async fn save(
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
///
/// # Errors
///
/// If the database can't be written.
pub async fn set_enabled(db: &Database, plugin_id: &str, enabled: bool) -> Result<(), sqlx::Error> {
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
