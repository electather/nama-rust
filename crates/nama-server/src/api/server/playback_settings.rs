//! The Playback settings, and what Transcodes run on, as the API has them.

use std::num::NonZeroU32;

use buffa::EnumValue;
use connectrpc::ConnectError;
use nama_proto::nama::v1::{
    self as proto, HardwareAcceleration as HardwareAccelerationProto, ToneMapping,
};

use super::{settings_change_failed, unknown_enum_value};
use crate::{
    ffmpeg::{Accel, ToneMapper},
    playback::{accel_to_proto, limit_to_proto},
    server_settings::{HardwareChoice, Invalid, PickedGpu, PlaybackValues},
};

/// The Playback settings as the API has them.
pub(super) fn playback_settings_to_proto(settings: PlaybackValues) -> proto::PlaybackSettings {
    proto::PlaybackSettings {
        segment_folder: settings.segment_folder,
        max_transcodes: settings.max_transcodes.map(NonZeroU32::get),
        keep_behind_minutes: settings.keep_behind_minutes.map(NonZeroU32::get),
        remote_playback_limit: limit_to_proto(settings.remote_limit).into(),
        hardware_acceleration: hardware_choice_to_proto(&settings.hardware).into(),
        gpu: (settings.hardware.gpu())
            .map(|gpu| gpu_to_proto(gpu.id.clone(), gpu.name.clone()))
            .into(),
        ..Default::default()
    }
}

/// The choice of what Transcodes run on that `choice` names, on the GPU at the PCI address `gpu`
/// if given, which [`Settings`] names.
///
/// # Errors
///
/// `UNKNOWN_ENUM_VALUE` for a `choice` this Server doesn't know, checked first, and
/// `GPU_NOT_OFFERED` for a GPU with Automatic or Off.
///
/// [`Settings`]: crate::server_settings::Settings
pub(super) fn hardware_choice(
    choice: EnumValue<HardwareAccelerationProto>,
    gpu: Option<&str>,
) -> Result<HardwareChoice, ConnectError> {
    use HardwareAccelerationProto as P;
    let accel = match choice {
        EnumValue::Unknown(_) => {
            return Err(unknown_enum_value(
                "settings.hardware_acceleration",
                "the hardware acceleration must be Automatic, Off, VAAPI, QSV, NVENC or \
                 VideoToolbox",
            ));
        }
        EnumValue::Known(P::HARDWARE_ACCELERATION_VAAPI) => Accel::Vaapi,
        EnumValue::Known(P::HARDWARE_ACCELERATION_QSV) => Accel::Qsv,
        EnumValue::Known(P::HARDWARE_ACCELERATION_NVENC) => Accel::Nvenc,
        EnumValue::Known(P::HARDWARE_ACCELERATION_VIDEOTOOLBOX) => Accel::VideoToolbox,
        _ if let Some(id) = gpu => {
            let invalid = Invalid::GpuNotOffered { id: id.to_owned() };
            return Err(settings_change_failed(invalid.into()));
        }
        EnumValue::Known(P::HARDWARE_ACCELERATION_OFF) => return Ok(HardwareChoice::Off),
        EnumValue::Known(P::HARDWARE_ACCELERATION_UNSPECIFIED) => {
            return Ok(HardwareChoice::Automatic);
        }
    };
    let gpu = gpu.map(|id| PickedGpu {
        id: id.to_owned(),
        name: String::new(),
    });
    Ok(HardwareChoice::Api(accel, gpu))
}

/// `choice`'s API as the API names it.
const fn hardware_choice_to_proto(choice: &HardwareChoice) -> HardwareAccelerationProto {
    match choice {
        HardwareChoice::Automatic => HardwareAccelerationProto::HARDWARE_ACCELERATION_UNSPECIFIED,
        HardwareChoice::Off => HardwareAccelerationProto::HARDWARE_ACCELERATION_OFF,
        HardwareChoice::Api(accel, _) => accel_to_proto(*accel),
    }
}

/// The GPU with the PCI address `id`, called `name`, as the API has it.
pub(super) fn gpu_to_proto(id: String, name: String) -> proto::Gpu {
    proto::Gpu {
        id,
        name,
        ..Default::default()
    }
}

/// What tone-maps HDR as the API names it.
pub(super) const fn tone_mapping_to_proto(tone_mapper: Option<ToneMapper>) -> ToneMapping {
    match tone_mapper {
        None => ToneMapping::TONE_MAPPING_NONE,
        Some(ToneMapper::Hardware) => ToneMapping::TONE_MAPPING_GPU,
        Some(
            ToneMapper::Software | ToneMapper::Tonemapx { .. } | ToneMapper::DolbyVisionToHdr10,
        ) => ToneMapping::TONE_MAPPING_SOFTWARE,
    }
}
