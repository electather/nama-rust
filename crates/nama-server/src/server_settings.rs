//! What an admin sets about the Server as a whole, kept in the one `server_settings` row: its
//! URL, which notifications link to the web app with, and the Playback settings: the folder
//! playback writes segment files to, the most Transcodes it runs at once, what they run on, and
//! the default Playback Limit for remote playback.
//!
//! [`Settings`] owns the table (see the crate's `AGENTS.md`).

use std::{
    num::NonZeroU32,
    path::{Path, PathBuf},
    sync::{Arc, PoisonError, RwLock},
};

use nama_plugin_api::settings::{SettingKind, SettingValue};
use sqlx::{Sqlite, Transaction};
use thiserror::Error;

use crate::{
    db::{BoxFuture, Database},
    ffmpeg::{Accel, Hardware},
    playback::PlaybackLimit,
};

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
}

/// One value of every Playback setting.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct PlaybackValues {
    /// The folder segment files go to as the admin set it, or `""` for the default.
    pub(crate) segment_folder: String,
    /// The most Transcodes at once, or `None` for the default, which depends on what they run on
    /// (`playback::plan`).
    pub(crate) max_transcodes: Option<NonZeroU32>,
    pub(crate) hardware: HardwareChoice,
    /// The Playback Limit for a Device outside the local network until its User picks one,
    /// `None` being Original.
    pub(crate) remote_limit: Option<PlaybackLimit>,
}

/// A change to the Playback settings, replacing all of them, as a client asked for it.
#[derive(Debug, Clone)]
pub(crate) struct PlaybackChange<'a> {
    /// Untrimmed, `""` for the default.
    pub(crate) segment_folder: &'a str,
    /// `None` for the default; 0 is refused.
    pub(crate) max_transcodes: Option<u32>,
    /// A picked GPU's name is ignored: the Server names it.
    pub(crate) hardware: HardwareChoice,
    pub(crate) remote_limit: Option<PlaybackLimit>,
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
    /// The Server URL isn't empty or an http or https URL, or has a query or fragment, which a
    /// link added to its end would break.
    #[error("the Server URL must be an http or https URL with no ? or #")]
    NotAUrl,
    /// The segment folder isn't empty or absolute: a relative one would depend on where the
    /// Server was started.
    #[error("the segment folder {folder} isn't an absolute path")]
    FolderNotAbsolute {
        /// As trimmed.
        folder: String,
    },
    /// The Transcode cap is 0, which would refuse every Transcode; a client asks for the default
    /// by leaving it out.
    #[error("the most Transcodes at once must be at least 1")]
    NoTranscodes,
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
            "SELECT url, segment_folder, max_transcodes, hardware_acceleration, hardware_gpu,
                    hardware_gpu_name, remote_playback_limit
             FROM server_settings"
        )
        .fetch_optional(self.db.reader())
        .await?;
        let values = stored.map_or_else(Values::default, |stored| Values {
            url: stored.url,
            playback: PlaybackValues {
                segment_folder: stored.segment_folder,
                max_transcodes: (stored.max_transcodes)
                    .and_then(|max| u32::try_from(max).ok())
                    .and_then(NonZeroU32::new),
                hardware: HardwareChoice::from_stored(
                    &stored.hardware_acceleration,
                    stored.hardware_gpu,
                    stored.hardware_gpu_name,
                ),
                remote_limit: PlaybackLimit::named(&stored.remote_playback_limit),
            },
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
    /// started from now on. A GPU it picks must be one `passed`, the hardware that passed its test
    /// at startup, has with its API, or the one stored with it.
    ///
    /// # Errors
    ///
    /// [`Invalid::FolderNotAbsolute`], [`Invalid::NoTranscodes`] or [`Invalid::GpuNotOffered`],
    /// in that order, or if storing them fails.
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
        let mut hardware = change.hardware;
        if let HardwareChoice::Api(accel, Some(gpu)) = &mut hardware {
            *gpu = self.offered_gpu(*accel, &gpu.id, passed)?;
        }
        let playback = PlaybackValues {
            segment_folder: folder.to_owned(),
            max_transcodes,
            hardware,
            remote_limit: change.remote_limit,
        };
        let stored_folder = playback.segment_folder.clone();
        let max = max_transcodes.map(NonZeroU32::get);
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
                             (id, url, segment_folder, max_transcodes, hardware_acceleration,
                              hardware_gpu, hardware_gpu_name, remote_playback_limit)
                         VALUES (1, '', ?, ?, ?, ?, ?, ?)
                         ON CONFLICT (id) DO UPDATE SET
                             segment_folder = excluded.segment_folder,
                             max_transcodes = excluded.max_transcodes,
                             hardware_acceleration = excluded.hardware_acceleration,
                             hardware_gpu = excluded.hardware_gpu,
                             hardware_gpu_name = excluded.hardware_gpu_name,
                             remote_playback_limit = excluded.remote_playback_limit",
                        stored_folder,
                        max,
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

/// Whether `url` may be the Server URL: empty, or an http or https URL as a URL setting takes,
/// with no query or fragment, since links are added to its end.
fn is_server_url(url: &str) -> bool {
    url.is_empty()
        || (SettingValue::Text(url.to_owned()).is(SettingKind::Url) && !url.contains(['?', '#']))
}

fn read(values: &RwLock<Values>) -> std::sync::RwLockReadGuard<'_, Values> {
    values.read().unwrap_or_else(PoisonError::into_inner)
}

/// The Server URL in use, for the event bus. Clones share it.
#[derive(Debug, Clone, Default)]
pub(crate) struct ServerUrl(Arc<RwLock<Values>>);

impl ServerUrl {
    /// Where the Request `id` is in the web app, or `None` with no URL set.
    pub(crate) fn request_link(&self, id: i64) -> Option<String> {
        let values = read(&self.0);
        let url = &values.url;
        // The URL may name a path the web app is served under, which a link keeps.
        (!url.is_empty()).then(|| format!("{}/requests/{id}", url.trim_end_matches('/')))
    }
}

/// The Playback settings in use, for planning and running plans. Clones share them.
#[derive(Debug, Clone)]
pub(crate) struct PlaybackSettings {
    default_folder: PathBuf,
    values: Arc<RwLock<Values>>,
}

impl PlaybackSettings {
    /// Every Playback setting, from one change.
    pub(crate) fn now(&self) -> PlaybackValues {
        read(&self.values).playback.clone()
    }

    /// Where segment files go now: the folder set, or else the default.
    pub(crate) fn path(&self) -> PathBuf {
        let set = &read(&self.values).playback.segment_folder;
        if set.is_empty() {
            self.default_folder.clone()
        } else {
            set.into()
        }
    }

    /// The folders an earlier run may have left segment files in that this one can know of: the
    /// one set, and the default.
    pub(crate) fn leftover_paths(&self) -> [PathBuf; 2] {
        [self.path(), self.default_folder.clone()]
    }
}

/// What an admin chose for Transcodes to run on.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) enum HardwareChoice {
    /// The first API that passed its test at startup, or software without one.
    #[default]
    Automatic,
    /// Software only.
    Off,
    /// This API on this GPU, if the pair passed its test at startup, or else software. With no
    /// GPU, as stored before GPUs could be picked, the API on the first GPU it passed on.
    Api(Accel, Option<PickedGpu>),
}

/// The GPU an admin picked for Transcodes to run on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PickedGpu {
    /// Its PCI address, which finds it whatever render node it has.
    pub(crate) id: String,
    /// The name it was last found under, which still names it once it's gone.
    pub(crate) name: String,
}

impl HardwareChoice {
    /// As the database stores it.
    const fn stored(&self) -> &'static str {
        match self {
            Self::Automatic => "",
            Self::Off => "off",
            Self::Api(Accel::Nvenc, _) => "nvenc",
            Self::Api(Accel::Qsv, _) => "qsv",
            Self::Api(Accel::Vaapi, _) => "vaapi",
            Self::Api(Accel::VideoToolbox, _) => "videotoolbox",
        }
    }

    /// The choice the database stores as `stored`, on the GPU at the PCI address `gpu` last found
    /// as `gpu_name` unless `gpu` is empty, Automatic for one this Server doesn't know.
    fn from_stored(stored: &str, gpu: String, gpu_name: String) -> Self {
        let picked = (!gpu.is_empty()).then_some(PickedGpu {
            id: gpu,
            name: gpu_name,
        });
        match Accel::ALL
            .into_iter()
            .find(|&accel| Self::Api(accel, None).stored() == stored)
        {
            Some(accel) => Self::Api(accel, picked),
            None if stored == Self::Off.stored() => Self::Off,
            None => Self::Automatic,
        }
    }

    /// The GPU picked, if one is.
    pub(crate) const fn gpu(&self) -> Option<&PickedGpu> {
        match self {
            Self::Api(_, gpu) => gpu.as_ref(),
            Self::Automatic | Self::Off => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        num::NonZeroU32,
        path::{Path, PathBuf},
        sync::Arc,
    };

    use super::{
        ChangeError, HardwareChoice, Invalid, PickedGpu, PlaybackChange, PlaybackValues, Settings,
        Values,
    };
    #[cfg(unix)]
    use crate::ffmpeg::{Hardware, fake, find_gpus};
    use crate::{db::Database, ffmpeg::Accel, playback::PlaybackLimit};

    const DEFAULT_FOLDER: &str = "/data/transcodes";

    async fn fresh() -> (tempfile::TempDir, Settings) {
        let data_dir = tempfile::tempdir().unwrap();
        let db = Database::open(data_dir.path()).await.unwrap();
        let settings = Settings::new(Arc::new(db), DEFAULT_FOLDER.into());
        settings.load().await.unwrap();
        (data_dir, settings)
    }

    /// A second `Settings` on the same database, as after a restart, once it loaded.
    async fn restarted(settings: &Settings) -> Settings {
        let restarted = Settings::new(Arc::clone(&settings.db), DEFAULT_FOLDER.into());
        restarted.load().await.unwrap();
        restarted
    }

    fn change(folder: &str) -> PlaybackChange<'_> {
        PlaybackChange {
            segment_folder: folder,
            max_transcodes: None,
            hardware: HardwareChoice::Automatic,
            remote_limit: None,
        }
    }

    fn invalid(result: Result<impl std::fmt::Debug, ChangeError>) -> Invalid {
        match result {
            Err(ChangeError::Invalid(invalid)) => invalid,
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn every_setting_is_its_default_until_one_is_stored() {
        let (_data_dir, settings) = fresh().await;
        let playback = settings.playback();

        assert_eq!(settings.now(), Values::default());
        assert_eq!(settings.server_url().request_link(7), None);
        assert_eq!(playback.now().hardware, HardwareChoice::Automatic);
        assert_eq!(playback.path(), Path::new(DEFAULT_FOLDER));
    }

    #[tokio::test]
    async fn a_stored_url_is_trimmed_used_for_links_and_read_again_after_a_restart() {
        let (_data_dir, settings) = fresh().await;

        let stored = settings
            .set_url(" https://example.com/nama/ ")
            .await
            .unwrap();

        assert_eq!(stored, "https://example.com/nama/");
        assert_eq!(
            settings.server_url().request_link(7).as_deref(),
            Some("https://example.com/nama/requests/7")
        );
        assert_eq!(restarted(&settings).await.now().url, stored);
    }

    #[tokio::test]
    async fn a_url_that_isnt_http_or_has_a_query_or_fragment_is_refused_and_changes_nothing() {
        let (_data_dir, settings) = fresh().await;
        settings.set_url("https://nama.example.com").await.unwrap();

        for not_a_url in [
            "nama.example.com",
            "ftp://nama.example.com",
            "https://nama.example.com/?a=1",
            "https://nama.example.com/#top",
        ] {
            assert_eq!(invalid(settings.set_url(not_a_url).await), Invalid::NotAUrl);
        }
        assert_eq!(settings.now().url, "https://nama.example.com");
    }

    #[tokio::test]
    async fn clearing_the_url_leaves_links_out() {
        let (_data_dir, settings) = fresh().await;
        settings.set_url("https://nama.example.com").await.unwrap();

        settings.set_url("  ").await.unwrap();

        assert_eq!(settings.server_url().request_link(7), None);
        assert_eq!(restarted(&settings).await.now().url, "");
    }

    #[tokio::test]
    async fn stored_playback_settings_are_in_use_and_read_again_after_a_restart() {
        let (_data_dir, settings) = fresh().await;
        let playback = settings.playback();

        let stored = settings
            .change_playback(
                PlaybackChange {
                    segment_folder: " /tmp/nama ",
                    max_transcodes: Some(5),
                    hardware: HardwareChoice::Api(Accel::Vaapi, None),
                    remote_limit: Some(PlaybackLimit::P720),
                },
                &[],
            )
            .await
            .unwrap();

        let expected = PlaybackValues {
            segment_folder: "/tmp/nama".to_owned(),
            max_transcodes: NonZeroU32::new(5),
            hardware: HardwareChoice::Api(Accel::Vaapi, None),
            remote_limit: Some(PlaybackLimit::P720),
        };
        assert_eq!(stored, expected);
        assert_eq!(playback.now(), expected);
        assert_eq!(playback.path(), Path::new("/tmp/nama"));
        assert_eq!(
            playback.leftover_paths(),
            [PathBuf::from("/tmp/nama"), DEFAULT_FOLDER.into()]
        );
        assert_eq!(restarted(&settings).await.playback().now(), expected);
    }

    #[tokio::test]
    async fn playback_settings_go_back_to_their_defaults_when_cleared() {
        let (_data_dir, settings) = fresh().await;
        settings
            .change_playback(
                PlaybackChange {
                    segment_folder: "/tmp/nama",
                    max_transcodes: Some(5),
                    hardware: HardwareChoice::Off,
                    remote_limit: Some(PlaybackLimit::P720),
                },
                &[],
            )
            .await
            .unwrap();

        settings.change_playback(change(""), &[]).await.unwrap();

        let restarted = restarted(&settings).await.playback();
        assert_eq!(restarted.now(), PlaybackValues::default());
        assert_eq!(restarted.path(), Path::new(DEFAULT_FOLDER));
    }

    #[tokio::test]
    async fn a_folder_that_isnt_absolute_is_refused_naming_it_trimmed() {
        let (_data_dir, settings) = fresh().await;

        assert_eq!(
            invalid(settings.change_playback(change(" tmp/nama "), &[]).await),
            Invalid::FolderNotAbsolute {
                folder: "tmp/nama".to_owned()
            }
        );
    }

    #[tokio::test]
    async fn a_cap_of_0_is_refused_and_leaves_every_playback_setting_as_it_was() {
        let (_data_dir, settings) = fresh().await;

        let refused = settings
            .change_playback(
                PlaybackChange {
                    max_transcodes: Some(0),
                    hardware: HardwareChoice::Off,
                    ..change("/tmp/nama")
                },
                &[],
            )
            .await;

        assert_eq!(invalid(refused), Invalid::NoTranscodes);
        assert_eq!(settings.playback().now(), PlaybackValues::default());
        assert_eq!(
            restarted(&settings).await.playback().now(),
            PlaybackValues::default()
        );
    }

    #[tokio::test]
    async fn a_folder_that_isnt_absolute_is_reported_before_a_cap_of_0() {
        let (_data_dir, settings) = fresh().await;

        let refused = settings
            .change_playback(
                PlaybackChange {
                    max_transcodes: Some(0),
                    ..change("tmp")
                },
                &[],
            )
            .await;

        assert_eq!(
            invalid(refused),
            Invalid::FolderNotAbsolute {
                folder: "tmp".to_owned()
            }
        );
    }

    /// VAAPI on a dedicated AMD GPU at `0000:03:00.0`, then on an integrated one at
    /// `0000:18:00.0`, as Automatic orders them.
    #[cfg(unix)]
    fn vaapi_on_two_gpus(root: &Path) -> Vec<Hardware> {
        let (sys, dri) = (root.join("sys"), root.join("dri"));
        fake::gpu(&sys, &dri, "renderD128", "0000:18:00.0", "0x1002");
        fake::pci_function(&sys, "0000:18:00.2", "0x1022");
        fake::gpu(&sys, &dri, "renderD129", "0000:03:00.0", "0x1002");
        find_gpus(&dri, &sys.join("class/drm"))
            .into_iter()
            .map(|gpu| Hardware {
                accel: Accel::Vaapi,
                gpu: Some(gpu),
                tone_maps: false,
            })
            .collect()
    }

    fn picked(accel: Accel, id: &str, name: &str) -> HardwareChoice {
        let gpu = PickedGpu {
            id: id.to_owned(),
            name: name.to_owned(),
        };
        HardwareChoice::Api(accel, Some(gpu))
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_picked_gpu_is_stored_under_the_name_it_was_found_with_and_read_after_a_restart() {
        let (data_dir, settings) = fresh().await;
        let passed = vaapi_on_two_gpus(data_dir.path());

        let stored = settings
            .change_playback(
                PlaybackChange {
                    hardware: picked(Accel::Vaapi, "0000:18:00.0", "a name the client made up"),
                    ..change("")
                },
                &passed,
            )
            .await
            .unwrap();

        let integrated = picked(Accel::Vaapi, "0000:18:00.0", "AMD integrated GPU");
        assert_eq!(stored.hardware, integrated);
        assert_eq!(settings.playback().now().hardware, integrated);
        assert_eq!(
            restarted(&settings).await.playback().now().hardware,
            integrated
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_gpu_its_api_didnt_pass_on_or_one_with_an_api_that_takes_none_is_refused() {
        let (data_dir, settings) = fresh().await;
        let passed = vaapi_on_two_gpus(data_dir.path());

        for (accel, id) in [
            (Accel::Vaapi, "0000:0a:00.0"),
            (Accel::Qsv, "0000:03:00.0"),
            (Accel::Nvenc, "0000:03:00.0"),
            (Accel::VideoToolbox, "0000:03:00.0"),
        ] {
            let refused = settings
                .change_playback(
                    PlaybackChange {
                        hardware: picked(accel, id, "AMD Radeon RX 9070 XT"),
                        ..change("")
                    },
                    &passed,
                )
                .await;

            assert_eq!(
                invalid(refused),
                Invalid::GpuNotOffered { id: id.to_owned() },
                "{accel:?}"
            );
        }
        assert_eq!(settings.playback().now(), PlaybackValues::default());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn after_a_restart_without_its_gpu_a_pick_keeps_its_name_and_survives_another_save() {
        let (data_dir, settings) = fresh().await;
        let pick = |name| PlaybackChange {
            hardware: picked(Accel::Vaapi, "0000:03:00.0", name),
            ..change("")
        };
        let passed = vaapi_on_two_gpus(data_dir.path());
        settings.change_playback(pick(""), &passed).await.unwrap();
        let restarted = restarted(&settings).await;

        let saved_again = restarted
            .change_playback(
                PlaybackChange {
                    max_transcodes: Some(3),
                    ..pick("another name")
                },
                &[],
            )
            .await
            .unwrap();

        let stored = picked(Accel::Vaapi, "0000:03:00.0", "AMD GPU at 0000:03:00.0");
        assert_eq!(saved_again.hardware, stored);
        assert_eq!(saved_again.max_transcodes, NonZeroU32::new(3));
    }

    #[tokio::test]
    async fn changing_one_group_keeps_the_others_stored_values() {
        let (_data_dir, settings) = fresh().await;
        settings.set_url("https://nama.example.com").await.unwrap();
        settings
            .change_playback(change("/tmp/nama"), &[])
            .await
            .unwrap();
        let after_playback = restarted(&settings).await.now();
        settings.set_url("https://other.example.com").await.unwrap();
        let after_url = restarted(&settings).await.now();

        assert_eq!(after_playback.url, "https://nama.example.com");
        assert_eq!(after_url.playback.segment_folder, "/tmp/nama");
    }

    #[tokio::test]
    async fn changing_the_playback_settings_first_stores_no_url() {
        let (_data_dir, settings) = fresh().await;

        settings
            .change_playback(change("/tmp/nama"), &[])
            .await
            .unwrap();

        assert_eq!(restarted(&settings).await.now().url, "");
    }
}
