//! What an admin sets about the Server as a whole, kept in the one `server_settings` row: its
//! URL, which notifications link to the web app with, and the Playback settings: the folder
//! playback writes segment files to, how far behind playback it keeps them, the most Transcodes it
//! runs at once, what they run on, and the default Playback Limit for remote playback; and whether
//! Posters and Backdrops are resized, how much disk the resized copies take, and which store
//! keeps the images; and the default Home layout.
//!
//! [`Settings`] owns the table (see the crate's `AGENTS.md`).

use std::{
    num::NonZeroU32,
    path::{Path, PathBuf},
    sync::{Arc, PoisonError, RwLock},
};

use nama_plugin_api::{
    PluginId,
    settings::{SettingKind, SettingValue},
};
use sqlx::{Sqlite, SqliteConnection, Transaction};
use thiserror::Error;
use url::Url;

use crate::{
    db::{BoxFuture, Database},
    ffmpeg::{Accel, Hardware},
};

mod home_layout;
mod playback;

pub(crate) use home_layout::{HomeLayout, HomeRowId, PlacedRow};
pub(crate) use playback::{
    HardwareChoice, PickedGpu, PlaybackChange, PlaybackLimit, PlaybackSettings, PlaybackValues,
};

/// How many MiB the resized copies of images take while no limit is set.
const DEFAULT_RESIZED_CACHE_MIB: u32 = 1024;

/// The Server settings: reads them once at start, checks and stores each change, and hands each
/// reader a view of the values in use. Clones share them.
#[derive(Debug, Clone)]
pub(crate) struct Settings {
    db: Arc<Database>,
    /// Where segment files go while no folder is set.
    default_folder: PathBuf,
    values: Arc<RwLock<Values>>,
}

/// One value of every Server setting.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Values {
    /// The address people reach the Server at, or `""` for none.
    pub(crate) url: String,
    pub(crate) playback: PlaybackValues,
    pub(crate) images: ImageValues,
    /// The Plugin keeping the image store's originals, or `None` for local disk.
    pub(crate) image_store: Option<PluginId>,
    pub(crate) default_home_layout: HomeLayout,
}

/// One value of every image setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ImageValues {
    /// Whether an image asked for at a width is sent resized to it.
    pub(crate) resize: bool,
    /// The most MiB the resized copies take, or `None` for [`DEFAULT_RESIZED_CACHE_MIB`].
    pub(crate) resized_cache_mib: Option<NonZeroU32>,
}

impl Default for ImageValues {
    fn default() -> Self {
        Self {
            resize: true,
            resized_cache_mib: None,
        }
    }
}

/// Why a change wasn't stored.
#[derive(Debug, Error)]
pub(crate) enum ChangeError {
    /// A value broke a rule: the first in field order. Nothing changed.
    #[error(transparent)]
    Invalid(#[from] Invalid),
    /// Storing it failed.
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

/// Which rule a proposed value broke.
#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum Invalid {
    /// The Server URL isn't empty or an http or https URL. Or it has a user name or password,
    /// which links would leak, or a query or fragment, which a link added to its end would break.
    #[error("the Server URL must be an http or https URL with no user name, password, ? or #")]
    NotAUrl,
    /// The segment folder isn't empty or absolute: a relative one would depend on where the
    /// Server was started.
    #[error("the segment folder {folder} isn't an absolute path")]
    FolderNotAbsolute {
        /// As trimmed.
        folder: String,
    },
    /// The segment folder can't be created, or a file can't be written in it, so every plan would
    /// fail to start.
    #[error("the segment folder {folder} can't be created or written to")]
    FolderNotWritable {
        /// As trimmed.
        folder: String,
    },
    /// The Transcode cap is 0, which would refuse every Transcode; a client asks for the default
    /// by leaving it out.
    #[error("the most Transcodes at once must be at least 1")]
    NoTranscodes,
    /// The time kept for skipping back is 0, which would delete what's just been played; a client
    /// asks for the default by leaving it out.
    #[error("the time kept for skipping back must be at least 1 minute")]
    NothingKeptBehind,
    /// The resized copies' limit is 0, which would delete each copy as it's made; a client asks
    /// for the default by leaving it out.
    #[error("the resized copies must be allowed at least 1 MiB")]
    NoResizedCache,
    /// The GPU picked isn't one its API passed its test on at startup, nor the one stored with
    /// it, or was sent with Automatic or Off, which take none.
    #[error("GPU {id} isn't one its hardware acceleration passed its test on")]
    GpuNotOffered {
        /// Its PCI address, as asked.
        id: String,
    },
}

impl Settings {
    /// The defaults, stored in `db`, with segment files going to `default_folder` while no folder
    /// is set. [`Settings::load`] takes what `db` stores.
    pub(crate) fn new(db: Arc<Database>, default_folder: PathBuf) -> Self {
        Self {
            db,
            default_folder,
            values: Arc::default(),
        }
    }

    /// Takes the stored settings as the ones in use. A stored value this Server can't read takes
    /// its default.
    ///
    /// # Errors
    ///
    /// If reading fails, leaving the settings as they were.
    pub(crate) async fn load(&self) -> Result<(), sqlx::Error> {
        let stored = sqlx::query!(
            "SELECT url, segment_folder, max_transcodes, keep_behind_minutes,
                    hardware_acceleration, hardware_gpu, hardware_gpu_name, remote_playback_limit,
                    resize_images, resized_cache_mib, image_store_plugin, default_home_layout
             FROM server_settings"
        )
        .fetch_optional(self.db.reader())
        .await?;
        let values = stored.map_or_else(Values::default, |stored| Values {
            url: stored.url,
            playback: PlaybackValues {
                segment_folder: stored.segment_folder,
                max_transcodes: stored.max_transcodes.and_then(non_zero),
                keep_behind_minutes: stored.keep_behind_minutes.and_then(non_zero),
                hardware: HardwareChoice::from_stored(
                    &stored.hardware_acceleration,
                    stored.hardware_gpu,
                    stored.hardware_gpu_name,
                ),
                remote_limit: PlaybackLimit::named(&stored.remote_playback_limit),
            },
            images: ImageValues {
                resize: stored.resize_images != 0,
                resized_cache_mib: stored.resized_cache_mib.and_then(non_zero),
            },
            image_store: plugin_named(stored.image_store_plugin),
            default_home_layout: (stored.default_home_layout)
                .and_then(|json| serde_json::from_str(&json).ok())
                .unwrap_or_default(),
        });
        *self.values.write().unwrap_or_else(PoisonError::into_inner) = values;
        Ok(())
    }

    /// The settings in use.
    pub(crate) fn now(&self) -> Values {
        read(&self.values).clone()
    }

    /// The Server URL, for building links.
    pub(crate) fn server_url(&self) -> ServerUrl {
        ServerUrl(Arc::clone(&self.values))
    }

    /// The Playback settings, for planning and running plans.
    pub(crate) fn playback(&self) -> PlaybackSettings {
        PlaybackSettings {
            default_folder: self.default_folder.clone(),
            values: Arc::clone(&self.values),
        }
    }

    /// The image settings, for serving images.
    pub(crate) fn images(&self) -> ImageSettings {
        ImageSettings(Arc::clone(&self.values))
    }

    /// Stores `url`, trimmed, as the Server URL and uses it from now on.
    ///
    /// # Errors
    ///
    /// [`Invalid::NotAUrl`], or if storing it fails.
    pub(crate) async fn set_url(&self, url: &str) -> Result<String, ChangeError> {
        let url = url.trim().to_owned();
        if !is_server_url(&url) {
            return Err(Invalid::NotAUrl.into());
        }
        let stored = url.clone();
        let in_use = url.clone();
        self.store(
            move |tx| {
                Box::pin(async move {
                    sqlx::query!(
                        "INSERT INTO server_settings (id, url) VALUES (1, ?)
                         ON CONFLICT (id) DO UPDATE SET url = excluded.url",
                        stored
                    )
                    .execute(&mut **tx)
                    .await?;
                    Ok(())
                })
            },
            move |values| values.url = in_use,
        )
        .await?;
        Ok(url)
    }

    /// Stores every Playback setting `change` gives, or none, and uses them for plans made and
    /// started from now on, and the time kept for skipping back for running plans too. A GPU it
    /// picks must be one `passed`, the hardware that passed its test at startup, has with its API,
    /// or the one stored with it.
    ///
    /// # Errors
    ///
    /// [`Invalid::FolderNotAbsolute`], [`Invalid::NoTranscodes`], [`Invalid::NothingKeptBehind`],
    /// [`Invalid::GpuNotOffered`] or [`Invalid::FolderNotWritable`], in that order, or if storing
    /// them fails. A folder given is created before it's stored, even if storing then fails.
    pub(crate) async fn change_playback(
        &self,
        change: PlaybackChange<'_>,
        passed: &[Hardware],
    ) -> Result<PlaybackValues, ChangeError> {
        let folder = change.segment_folder.trim();
        if !(folder.is_empty() || Path::new(folder).is_absolute()) {
            return Err(Invalid::FolderNotAbsolute {
                folder: folder.to_owned(),
            }
            .into());
        }
        let max_transcodes = match change.max_transcodes {
            Some(max) => Some(NonZeroU32::new(max).ok_or(Invalid::NoTranscodes)?),
            None => None,
        };
        let keep_behind_minutes = match change.keep_behind_minutes {
            Some(minutes) => Some(NonZeroU32::new(minutes).ok_or(Invalid::NothingKeptBehind)?),
            None => None,
        };
        let mut hardware = change.hardware;
        if let HardwareChoice::Api(accel, Some(gpu)) = &mut hardware {
            *gpu = self.offered_gpu(*accel, &gpu.id, passed)?;
        }
        if !folder.is_empty() && writable(Path::new(folder)).await.is_err() {
            return Err(Invalid::FolderNotWritable {
                folder: folder.to_owned(),
            }
            .into());
        }
        let playback = PlaybackValues {
            segment_folder: folder.to_owned(),
            max_transcodes,
            keep_behind_minutes,
            hardware,
            remote_limit: change.remote_limit,
        };
        let stored_folder = playback.segment_folder.clone();
        let max = max_transcodes.map(NonZeroU32::get);
        let keep_behind = keep_behind_minutes.map(NonZeroU32::get);
        let api = playback.hardware.stored();
        let (gpu, gpu_name) = playback
            .hardware
            .gpu()
            .map_or_else(Default::default, |gpu| (gpu.id.clone(), gpu.name.clone()));
        let limit = playback.remote_limit.map_or("", PlaybackLimit::name);
        let in_use = playback.clone();
        self.store(
            move |tx| {
                Box::pin(async move {
                    sqlx::query!(
                        "INSERT INTO server_settings
                             (id, url, segment_folder, max_transcodes, keep_behind_minutes,
                              hardware_acceleration, hardware_gpu, hardware_gpu_name,
                              remote_playback_limit)
                         VALUES (1, '', ?, ?, ?, ?, ?, ?, ?)
                         ON CONFLICT (id) DO UPDATE SET
                             segment_folder = excluded.segment_folder,
                             max_transcodes = excluded.max_transcodes,
                             keep_behind_minutes = excluded.keep_behind_minutes,
                             hardware_acceleration = excluded.hardware_acceleration,
                             hardware_gpu = excluded.hardware_gpu,
                             hardware_gpu_name = excluded.hardware_gpu_name,
                             remote_playback_limit = excluded.remote_playback_limit",
                        stored_folder,
                        max,
                        keep_behind,
                        api,
                        gpu,
                        gpu_name,
                        limit
                    )
                    .execute(&mut **tx)
                    .await?;
                    Ok(())
                })
            },
            move |values| values.playback = in_use,
        )
        .await?;
        Ok(playback)
    }

    /// Stores whether images are `resize`d and the most MiB their copies take, `None` for the
    /// default, and uses them for images asked for from now on.
    ///
    /// # Errors
    ///
    /// [`Invalid::NoResizedCache`], or if storing them fails.
    pub(crate) async fn change_images(
        &self,
        resize: bool,
        resized_cache_mib: Option<u32>,
    ) -> Result<ImageValues, ChangeError> {
        let resized_cache_mib = match resized_cache_mib {
            Some(mib) => Some(NonZeroU32::new(mib).ok_or(Invalid::NoResizedCache)?),
            None => None,
        };
        let images = ImageValues {
            resize,
            resized_cache_mib,
        };
        let mib = resized_cache_mib.map(NonZeroU32::get);
        self.store(
            move |tx| {
                Box::pin(async move {
                    sqlx::query!(
                        "INSERT INTO server_settings (id, url, resize_images, resized_cache_mib)
                         VALUES (1, '', ?, ?)
                         ON CONFLICT (id) DO UPDATE SET
                             resize_images = excluded.resize_images,
                             resized_cache_mib = excluded.resized_cache_mib",
                        resize,
                        mib
                    )
                    .execute(&mut **tx)
                    .await?;
                    Ok(())
                })
            },
            move |values| values.images = images,
        )
        .await?;
        Ok(images)
    }

    /// Stores `plugin` as the one keeping the image store's originals, `None` for local disk. Only
    /// a switch of the image store, once it has copied them, calls it.
    ///
    /// # Errors
    ///
    /// If storing it fails.
    pub(crate) async fn set_image_store(&self, plugin: Option<PluginId>) -> sqlx::Result<()> {
        let stored = plugin.as_ref().map_or_default(ToString::to_string);
        self.store(
            move |tx| {
                Box::pin(async move {
                    sqlx::query!(
                        "INSERT INTO server_settings (id, url, image_store_plugin)
                         VALUES (1, '', ?)
                         ON CONFLICT (id) DO UPDATE SET
                             image_store_plugin = excluded.image_store_plugin",
                        stored
                    )
                    .execute(&mut **tx)
                    .await?;
                    Ok(())
                })
            },
            move |values| values.image_store = plugin,
        )
        .await
    }

    /// Stores `layout` as the default Home layout and uses it from now on.
    ///
    /// # Errors
    ///
    /// If storing it fails.
    pub(crate) async fn set_default_home_layout(&self, layout: HomeLayout) -> sqlx::Result<()> {
        let stored = to_json(&layout)?;
        self.store(
            move |tx| Box::pin(store_default_home_layout(tx, stored)),
            move |values| values.default_home_layout = layout,
        )
        .await
    }

    /// Drops the default Home layout's rows of each Plugin the Server no longer `has`, as it was
    /// uninstalled (ADR 0077).
    ///
    /// # Errors
    ///
    /// If storing the layout without them fails.
    pub(crate) async fn forget_uninstalled(
        &self,
        has: impl Fn(&PluginId) -> bool,
    ) -> sqlx::Result<()> {
        let gone: Vec<PluginId> = (read(&self.values).default_home_layout.rows.iter())
            .filter_map(|placed| placed.row.plugin())
            .filter(|plugin| !has(plugin))
            .cloned()
            .collect();
        if gone.is_empty() {
            return Ok(());
        }
        let values = Arc::clone(&self.values);
        let forgotten = gone.clone();
        self.store(
            // Read on the writer, so a layout saved meanwhile loses only these rows.
            move |tx| {
                let mut layout = read(&values).default_home_layout.clone();
                layout.forget(&forgotten);
                Box::pin(async move { store_default_home_layout(tx, to_json(&layout)?).await })
            },
            move |values| values.default_home_layout.forget(&gone),
        )
        .await
    }

    /// The GPU at the PCI address `id`, named as `passed` found it with `accel`, or else as
    /// stored with `accel`, so saving another setting keeps a pick whose GPU is gone.
    fn offered_gpu(
        &self,
        accel: Accel,
        id: &str,
        passed: &[Hardware],
    ) -> Result<PickedGpu, Invalid> {
        let found = (passed.iter())
            .find(|hardware| hardware.is_pair(accel, id))
            .and_then(|hardware| hardware.gpu.as_ref());
        if let Some(gpu) = found {
            return Ok(PickedGpu {
                id: id.to_owned(),
                name: gpu.name(),
            });
        }
        match &read(&self.values).playback.hardware {
            HardwareChoice::Api(stored, Some(gpu)) if *stored == accel && gpu.id == id => {
                Ok(gpu.clone())
            }
            HardwareChoice::Automatic | HardwareChoice::Off | HardwareChoice::Api(..) => {
                Err(Invalid::GpuNotOffered { id: id.to_owned() })
            }
        }
    }

    /// Runs `upsert`, which writes only its own group's columns, then `merge`s the same values
    /// into the ones in use once they're committed.
    async fn store<U>(
        &self,
        upsert: U,
        merge: impl FnOnce(&mut Values) + Send + 'static,
    ) -> Result<(), sqlx::Error>
    where
        U: for<'t> FnOnce(
                &'t mut Transaction<'_, Sqlite>,
            ) -> BoxFuture<'t, Result<(), sqlx::Error>>
            + Send
            + 'static,
    {
        let values = Arc::clone(&self.values);
        // Merged on the writer, so changes take effect in the order they're stored.
        self.db
            .write_then(upsert, move || {
                merge(&mut values.write().unwrap_or_else(PoisonError::into_inner));
            })
            .await
    }
}

/// Stores `layout`, the default Home layout as JSON.
async fn store_default_home_layout(
    tx: &mut Transaction<'_, Sqlite>,
    layout: String,
) -> sqlx::Result<()> {
    sqlx::query!(
        "INSERT INTO server_settings (id, url, default_home_layout) VALUES (1, '', ?)
         ON CONFLICT (id) DO UPDATE SET default_home_layout = excluded.default_home_layout",
        layout
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn to_json(value: &impl serde::Serialize) -> sqlx::Result<String> {
    serde_json::to_string(value).map_err(|err| sqlx::Error::Encode(err.into()))
}

/// Whether `url` may be the Server URL: empty, or an http or https URL as a URL setting takes,
/// with no query or fragment, since links are added to its end, and no user name or password,
/// which those links would carry to every recipient.
fn is_server_url(url: &str) -> bool {
    url.is_empty()
        || (SettingValue::Text(url.to_owned()).is(SettingKind::Url)
            && !url.contains(['?', '#'])
            && Url::parse(url)
                .is_ok_and(|parsed| parsed.username().is_empty() && parsed.password().is_none()))
}

/// The Plugin keeping the image store's originals in the database `conn` is open on, such as a
/// backup's copy, or `None` for local disk.
///
/// # Errors
///
/// If reading fails.
pub(crate) async fn stored_image_store(
    conn: &mut SqliteConnection,
) -> sqlx::Result<Option<PluginId>> {
    let stored = sqlx::query_scalar!("SELECT image_store_plugin FROM server_settings")
        .fetch_optional(conn)
        .await?;
    Ok(stored.and_then(plugin_named))
}

/// The Plugin `stored` names, or `None` for an empty one.
fn plugin_named(stored: String) -> Option<PluginId> {
    (!stored.is_empty()).then(|| PluginId::from(stored))
}

/// Creates `folder`, then writes and deletes a file in it, as a plan starting there would.
async fn writable(folder: &Path) -> std::io::Result<()> {
    // Not `nama-plan-…`: startup deletes those as a stopped plan's leftovers.
    // ponytail: two saves of one folder at once can delete each other's file and be refused; name
    // it uniquely if Admins ever hit that.
    let test_file = folder.join(".nama-write-test");
    tokio::fs::create_dir_all(folder).await?;
    tokio::fs::write(&test_file, b"").await?;
    tokio::fs::remove_file(&test_file).await
}

/// A stored count this Server can read, `None` for one out of range.
fn non_zero(stored: i64) -> Option<NonZeroU32> {
    u32::try_from(stored).ok().and_then(NonZeroU32::new)
}

pub(super) fn read(values: &RwLock<Values>) -> std::sync::RwLockReadGuard<'_, Values> {
    values.read().unwrap_or_else(PoisonError::into_inner)
}

/// The Server URL in use, for the event bus and Invites. Clones share it.
#[derive(Debug, Clone, Default)]
pub(crate) struct ServerUrl(Arc<RwLock<Values>>);

impl ServerUrl {
    /// Where the Request `id` is in the web app, or `None` with no URL set.
    pub(crate) fn request_link(&self, id: i64) -> Option<String> {
        self.link(&format!("requests/{id}"))
    }

    /// Where `path` is in the web app, or `None` with no URL set.
    pub(crate) fn link(&self, path: &str) -> Option<String> {
        let values = read(&self.0);
        let url = &values.url;
        // The URL may name a path the web app is served under, which a link keeps.
        (!url.is_empty()).then(|| format!("{}/{path}", url.trim_end_matches('/')))
    }
}

/// The image settings in use, for serving images. Clones share them.
#[derive(Debug, Clone)]
pub(crate) struct ImageSettings(Arc<RwLock<Values>>);

impl ImageSettings {
    /// The most bytes the resized copies may take while resizing is on, `None` while it's off.
    pub(crate) fn resizing(&self) -> Option<u64> {
        let images = &read(&self.0).images;
        let mib = images
            .resized_cache_mib
            .map_or(DEFAULT_RESIZED_CACHE_MIB, NonZeroU32::get);
        images.resize.then_some(u64::from(mib) * 1024 * 1024)
    }
}

#[cfg(test)]
#[path = "server_settings/server_settings_tests.rs"]
mod tests;
