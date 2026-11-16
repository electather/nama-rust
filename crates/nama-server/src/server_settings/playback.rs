//! The Playback settings: the values in use, a change to them, and what Transcodes run on.

use std::{
    num::NonZeroU32,
    path::PathBuf,
    sync::{Arc, RwLock},
};

use super::{Values, read};
use crate::{ffmpeg::Accel, playback::PlaybackLimit};

/// How many minutes behind playback a running plan keeps its files while none is set: about 1 GB
/// behind a 4K Remux, which a small tmpfs holds (#671).
const DEFAULT_KEEP_BEHIND_MINUTES: u32 = 5;

/// One value of every Playback setting.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct PlaybackValues {
    /// The folder segment files go to as the admin set it, or `""` for the default.
    pub(crate) segment_folder: String,
    /// The most Transcodes at once, or `None` for the default, which depends on what they run on
    /// (`playback::plan`).
    pub(crate) max_transcodes: Option<NonZeroU32>,
    /// How many minutes behind playback a running plan keeps its files, or `None` for
    /// [`DEFAULT_KEEP_BEHIND_MINUTES`].
    pub(crate) keep_behind_minutes: Option<NonZeroU32>,
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
    /// `None` for the default; 0 is refused.
    pub(crate) keep_behind_minutes: Option<u32>,
    /// A picked GPU's name is ignored: the Server names it.
    pub(crate) hardware: HardwareChoice,
    pub(crate) remote_limit: Option<PlaybackLimit>,
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
