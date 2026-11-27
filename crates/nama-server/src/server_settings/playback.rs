//! The Playback settings: the values in use, a change to them, what Transcodes run on, and the
//! Playback Limits.

use std::{
    num::NonZeroU32,
    path::PathBuf,
    sync::{Arc, RwLock},
};

use super::{Invalid, Settings, Values, read};
use crate::ffmpeg::{Accel, Hardware};

/// How many minutes behind playback a running plan keeps its files while none is set: about 1 GB
/// behind a 4K Remux, which a small tmpfs holds (#671).
const DEFAULT_KEEP_BEHIND_MINUTES: u32 = 5;

/// How many MiB the subtitle cache takes while no limit is set: image Tracks are tens of MB a
/// Blu-ray rip, and each one deleted is a whole-file read to make again (#2495).
const DEFAULT_SUBTITLE_CACHE_MIB: u32 = 5120;

/// A Playback Limit below Original: the picture a Transcode fits the video in, and the bitrate it
/// sends it at, which Users never see.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PlaybackLimit {
    pub(crate) name: &'static str,
    pub(crate) width: i64,
    pub(crate) height: i64,
    pub(crate) bits_per_second: u32,
}

impl PlaybackLimit {
    pub(crate) const P1080: Self = Self::new("1080p", 1920, 1080, 8_000_000);
    pub(crate) const P720: Self = Self::new("720p", 1280, 720, 4_000_000);
    // 854 is 480 at 16:9, made even.
    pub(crate) const P480: Self = Self::new("480p", 854, 480, 1_500_000);

    pub(crate) const ALL: [Self; 3] = [Self::P1080, Self::P720, Self::P480];

    /// What it's called, such as "720p", which is also how it's stored.
    pub(crate) const fn name(self) -> &'static str {
        self.name
    }

    /// The limit called `name`, if there is one.
    pub(crate) fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|limit| limit.name == name)
    }

    const fn new(name: &'static str, width: i64, height: i64, bits_per_second: u32) -> Self {
        Self {
            name,
            width,
            height,
            bits_per_second,
        }
    }
}

/// One value of every Playback setting.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct PlaybackValues {
    /// The folder segment files go to as the admin set it, or `""` for the default.
    pub(crate) segment_folder: String,
    /// The most Transcodes at once, or `None` for the default, which depends on what they run on
    /// (`playback::Plans::plan`).
    pub(crate) max_transcodes: Option<NonZeroU32>,
    /// How many minutes behind playback a running plan keeps its files, or `None` for
    /// [`DEFAULT_KEEP_BEHIND_MINUTES`].
    pub(crate) keep_behind_minutes: Option<NonZeroU32>,
    pub(crate) hardware: HardwareChoice,
    /// The Playback Limit for a Device outside the local network until its User picks one,
    /// `None` being Original.
    pub(crate) remote_limit: Option<PlaybackLimit>,
    /// The most MiB the subtitle cache takes, or `None` for [`DEFAULT_SUBTITLE_CACHE_MIB`].
    pub(crate) subtitle_cache_mib: Option<NonZeroU32>,
}

/// A change to the Playback settings, replacing all of them, as a client asked for it.
#[derive(Debug, Clone)]
pub(crate) struct PlaybackChange<'a> {
    /// Untrimmed, `""` for the default.
    pub(crate) segment_folder: &'a str,
    /// `None` for the default; 0 is refused.
    pub(crate) max_transcodes: Option<u32>,
    /// `None` for the default; 0 is refused.
    pub(crate) keep_behind_minutes: Option<u32>,
    /// A picked GPU's name is ignored: the Server names it.
    pub(crate) hardware: HardwareChoice,
    pub(crate) remote_limit: Option<PlaybackLimit>,
    /// `None` for the default; 0 is refused.
    pub(crate) subtitle_cache_mib: Option<u32>,
}

/// The Playback settings in use, for planning and running plans. Clones share them.
#[derive(Debug, Clone)]
pub(crate) struct PlaybackSettings {
    pub(super) default_folder: PathBuf,
    pub(super) values: Arc<RwLock<Values>>,
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

    /// How far behind playback a running plan keeps its files now, in milliseconds: the time set,
    /// or else the default.
    pub(crate) fn keep_behind_ms(&self) -> i64 {
        let minutes = (read(&self.values).playback.keep_behind_minutes)
            .map_or(DEFAULT_KEEP_BEHIND_MINUTES, NonZeroU32::get);
        i64::from(minutes) * 60_000
    }

    /// The most bytes the subtitle cache takes now: the limit set, or else the default.
    pub(crate) fn subtitle_cache_bytes(&self) -> u64 {
        let mib = (read(&self.values).playback.subtitle_cache_mib)
            .map_or(DEFAULT_SUBTITLE_CACHE_MIB, NonZeroU32::get);
        u64::from(mib) * 1024 * 1024
    }

    /// The folders an earlier run may have left segment files in that this one can know of: the
    /// one set, and the default.
    pub(crate) fn leftover_paths(&self) -> [PathBuf; 2] {
        [self.path(), self.default_folder.clone()]
    }
}

impl Settings {
    /// The GPU at the PCI address `id`, named as `passed` found it with `accel`, or else as
    /// stored with `accel`, so saving another setting keeps a pick whose GPU is gone.
    pub(super) fn offered_gpu(
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
    pub(super) const fn stored(&self) -> &'static str {
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
    pub(super) fn from_stored(stored: &str, gpu: String, gpu_name: String) -> Self {
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
