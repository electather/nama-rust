//! Facts about this Server, its settings, and the ffmpeg it found: `nama.v1.ServerService`.

use std::{future, num::NonZeroU32, sync::Arc};

use buffa::EnumValue;
use connectrpc::{
    ConnectError, Encodable, RequestContext, Response, ServiceRequest, ServiceResult,
};
use nama_proto::nama::v1::{
    self as proto, FoundHardware, FoundProgram, GetPlaybackSettingsRequest,
    GetPlaybackSettingsResponse, GetPlaybackStatusRequest, GetPlaybackStatusResponse,
    GetServerInfoRequest, GetServerInfoResponse, GetServerSettingsRequest,
    GetServerSettingsResponse, HardwareAcceleration as HardwareAccelerationProto, RefusalReason,
    ServerService, ServerSettings, UpdatePlaybackSettingsRequest, UpdatePlaybackSettingsResponse,
    UpdateServerSettingsRequest, UpdateServerSettingsResponse,
};

use crate::{
    ffmpeg::{self, Accel, Ffmpeg},
    playback::running::RunningPlans,
    server_settings::{self, HardwareChoice, Invalid, PlaybackChange, PlaybackValues},
    users::Users,
};

use super::{
    database_failed,
    playback::{limit_from_proto, limit_to_proto},
    refuse,
};

/// `nama.v1.ServerService`: facts about this Server, its settings, and the ffmpeg it found.
pub(super) struct ServerServiceHandler {
    pub(super) name: String,
    pub(super) users: Users,
    pub(super) settings: server_settings::Settings,
    /// For the hardware found.
    pub(super) running: RunningPlans,
    pub(super) ffmpeg: Arc<Ffmpeg>,
}

#[expect(
    refining_impl_trait,
    reason = "plain `async fn` returning the owned response is the form the generated trait \
              documents; the refined signature isn't visible outside this crate"
)]
impl ServerService for ServerServiceHandler {
    async fn get_server_info(
        &self,
        _ctx: RequestContext,
        _request: ServiceRequest<'_, GetServerInfoRequest>,
    ) -> ServiceResult<GetServerInfoResponse> {
        let setup_required = !self.users.any().await.map_err(database_failed)?;
        Response::ok(GetServerInfoResponse {
            name: self.name.clone(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            setup_required,
            ..Default::default()
        })
    }

    fn get_server_settings<'a>(
        &'a self,
        _ctx: RequestContext,
        _request: ServiceRequest<'_, GetServerSettingsRequest>,
    ) -> impl Future<
        Output = ServiceResult<impl Encodable<GetServerSettingsResponse> + Send + use<'a>>,
    > + Send {
        future::ready(Response::ok(GetServerSettingsResponse {
            settings: server_settings_to_proto(self.settings.now().url).into(),
            ..Default::default()
        }))
    }

    async fn update_server_settings(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, UpdateServerSettingsRequest>,
    ) -> ServiceResult<UpdateServerSettingsResponse> {
        let url =
            (self.settings.set_url(request.settings.url).await).map_err(settings_change_failed)?;
        Response::ok(UpdateServerSettingsResponse {
            settings: server_settings_to_proto(url).into(),
            ..Default::default()
        })
    }

    async fn get_playback_status(
        &self,
        _ctx: RequestContext,
        _request: ServiceRequest<'_, GetPlaybackStatusRequest>,
    ) -> ServiceResult<GetPlaybackStatusResponse> {
        let found = |program: &Result<ffmpeg::FoundProgram, String>| {
            program.as_ref().ok().map(|program| FoundProgram {
                path: program.path.display().to_string(),
                version: program.version.clone(),
                ..Default::default()
            })
        };
        let problem = |program: &Result<ffmpeg::FoundProgram, String>| {
            program.as_ref().err().cloned().unwrap_or_default()
        };
        let hardware = (self.running.found_hardware().await.iter())
            .map(|hardware| FoundHardware {
                api: hardware_choice_to_proto(HardwareChoice::Api(hardware.accel)).into(),
                tone_maps: hardware.tone_maps,
                ..Default::default()
            })
            .collect();
        Response::ok(GetPlaybackStatusResponse {
            ffmpeg: found(&self.ffmpeg.ffmpeg).into(),
            ffprobe: found(&self.ffmpeg.ffprobe).into(),
            ffmpeg_problem: problem(&self.ffmpeg.ffmpeg),
            ffprobe_problem: problem(&self.ffmpeg.ffprobe),
            hardware,
            ..Default::default()
        })
    }

    fn get_playback_settings<'a>(
        &'a self,
        _ctx: RequestContext,
        _request: ServiceRequest<'_, GetPlaybackSettingsRequest>,
    ) -> impl Future<
        Output = ServiceResult<impl Encodable<GetPlaybackSettingsResponse> + Send + use<'a>>,
    > + Send {
        future::ready(Response::ok(GetPlaybackSettingsResponse {
            settings: playback_settings_to_proto(self.settings.now().playback).into(),
            ..Default::default()
        }))
    }

    async fn update_playback_settings(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, UpdatePlaybackSettingsRequest>,
    ) -> ServiceResult<UpdatePlaybackSettingsResponse> {
        let change = PlaybackChange {
            segment_folder: request.settings.segment_folder,
            max_transcodes: request.settings.max_transcodes,
            hardware: hardware_choice(request.settings.hardware_acceleration),
            remote_limit: limit_from_proto(request.settings.remote_playback_limit),
        };
        let stored =
            (self.settings.change_playback(change).await).map_err(settings_change_failed)?;
        Response::ok(UpdatePlaybackSettingsResponse {
            settings: playback_settings_to_proto(stored).into(),
            ..Default::default()
        })
    }
}

/// The Server settings as the API has them.
fn server_settings_to_proto(url: String) -> ServerSettings {
    ServerSettings {
        url,
        ..Default::default()
    }
}

/// The Playback settings as the API has them.
fn playback_settings_to_proto(settings: PlaybackValues) -> proto::PlaybackSettings {
    proto::PlaybackSettings {
        segment_folder: settings.segment_folder,
        max_transcodes: settings.max_transcodes.map(NonZeroU32::get),
        remote_playback_limit: limit_to_proto(settings.remote_limit).into(),
        hardware_acceleration: hardware_choice_to_proto(settings.hardware).into(),
        ..Default::default()
    }
}

/// The refusal naming the field whose value broke a rule, or a generic error if storing failed.
fn settings_change_failed(err: server_settings::ChangeError) -> ConnectError {
    use server_settings::ChangeError::{Database, Invalid as Refused};
    match err {
        Database(err) => return database_failed(err),
        Refused(Invalid::NotAUrl) => refuse(
            RefusalReason::REFUSAL_REASON_NOT_A_URL,
            "the Server URL must be an http or https URL with no ? or #, such as \
             https://nama.example.com",
        )
        .field("settings.url"),
        Refused(Invalid::FolderNotAbsolute { folder }) => refuse(
            RefusalReason::REFUSAL_REASON_FOLDER_NOT_ABSOLUTE,
            "the segment folder must be an absolute path, such as /dev/shm/nama",
        )
        .field("settings.segment_folder")
        .param("folder", &folder),
        Refused(Invalid::NoTranscodes) => refuse(
            RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
            "the most Transcodes at once must be at least 1",
        )
        .field("settings.max_transcodes"),
    }
    .into()
}

/// The choice of what Transcodes run on that `choice` names, Automatic for one this Server
/// doesn't know.
fn hardware_choice(choice: EnumValue<HardwareAccelerationProto>) -> HardwareChoice {
    use HardwareAccelerationProto as P;
    match choice {
        EnumValue::Known(P::HARDWARE_ACCELERATION_OFF) => HardwareChoice::Off,
        EnumValue::Known(P::HARDWARE_ACCELERATION_VAAPI) => HardwareChoice::Api(Accel::Vaapi),
        EnumValue::Known(P::HARDWARE_ACCELERATION_QSV) => HardwareChoice::Api(Accel::Qsv),
        EnumValue::Known(P::HARDWARE_ACCELERATION_NVENC) => HardwareChoice::Api(Accel::Nvenc),
        EnumValue::Known(P::HARDWARE_ACCELERATION_VIDEOTOOLBOX) => {
            HardwareChoice::Api(Accel::VideoToolbox)
        }
        EnumValue::Known(P::HARDWARE_ACCELERATION_UNSPECIFIED) | EnumValue::Unknown(_) => {
            HardwareChoice::Automatic
        }
    }
}

/// `choice` as the API names it.
const fn hardware_choice_to_proto(choice: HardwareChoice) -> HardwareAccelerationProto {
    use HardwareAccelerationProto as P;
    match choice {
        HardwareChoice::Automatic => P::HARDWARE_ACCELERATION_UNSPECIFIED,
        HardwareChoice::Off => P::HARDWARE_ACCELERATION_OFF,
        HardwareChoice::Api(Accel::Vaapi) => P::HARDWARE_ACCELERATION_VAAPI,
        HardwareChoice::Api(Accel::Qsv) => P::HARDWARE_ACCELERATION_QSV,
        HardwareChoice::Api(Accel::Nvenc) => P::HARDWARE_ACCELERATION_NVENC,
        HardwareChoice::Api(Accel::VideoToolbox) => P::HARDWARE_ACCELERATION_VIDEOTOOLBOX,
    }
}
