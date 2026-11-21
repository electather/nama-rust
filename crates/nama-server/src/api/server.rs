//! Facts about this Server, its settings, and the ffmpeg it found: `nama.v1.ServerService`.

use std::{future, io, num::NonZeroU32, path::PathBuf, sync::Arc};

use buffa::{EnumValue, MessageFieldView};
use connectrpc::{
    ConnectError, Encodable, RequestContext, Response, ServiceRequest, ServiceResult,
};
use nama_plugin_api::PluginId;
use nama_proto::nama::v1::{
    self as proto, FoundHardware, FoundProgram, GetDefaultHomeLayoutRequest,
    GetDefaultHomeLayoutResponse, GetImageSettingsRequest, GetImageSettingsResponse,
    GetPlaybackSettingsRequest, GetPlaybackSettingsResponse, GetPlaybackStatusRequest,
    GetPlaybackStatusResponse, GetServerInfoRequest, GetServerInfoResponse,
    GetServerSettingsRequest, GetServerSettingsResponse,
    HardwareAcceleration as HardwareAccelerationProto, HomeLayoutRow, ImageStoreSwitch,
    ListPlaysRequest, ListPlaysResponse, Play, PlaybackMethod, RefusalReason, ServerService,
    ServerSettings, SetDefaultHomeLayoutRequest, SetDefaultHomeLayoutResponse, StopPlayRequest,
    StopPlayResponse, SwitchImageStoreRequest, SwitchImageStoreResponse, ToneMapping,
    TranscodeHardware, UpdateImageSettingsRequest, UpdateImageSettingsResponse,
    UpdatePlaybackSettingsRequest, UpdatePlaybackSettingsResponse, UpdateServerSettingsRequest,
    UpdateServerSettingsResponse, home_layout_row::Row as LayoutRow,
};

use crate::{
    ffmpeg::{self, Abilities, Accel, Ffmpeg, ToneMapper},
    home_rows::HomeRows,
    images::{self, Blobs, ImageStore, Persist, SwitchRefused, Switching},
    playback::{
        MessageTooLong, PlayNow, RunningPlans, STOP_MESSAGE_LENGTH, accel_to_proto, limit_to_proto,
    },
    plugins::{ExtensionPoints, PluginBlobs},
    server_settings::{
        self, HardwareChoice, HomeLayout, HomeRowId, ImageValues, Invalid, PickedGpu, PlacedRow,
        PlaybackChange, PlaybackValues,
    },
    users::Users,
};

use super::{database_failed, playback::limit_from_proto, refuse, unknown_enum_value};

/// `nama.v1.ServerService`: facts about this Server, its settings, and the ffmpeg it found.
pub(super) struct ServerServiceHandler {
    pub(super) name: String,
    pub(super) users: Users,
    pub(super) settings: server_settings::Settings,
    pub(super) abilities: Abilities,
    pub(super) ffmpeg: Arc<Ffmpeg>,
    pub(super) images: ImageStore,
    pub(super) points: ExtensionPoints,
    /// Lists the Home rows the default Home layout places.
    pub(super) home_rows: HomeRows,
    /// Where local disk keeps the images.
    pub(super) data_dir: PathBuf,
    /// Whether the Server exports Diagnostics, so the web app records its own.
    pub(super) exports_diagnostics: bool,
    pub(super) running: RunningPlans,
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
            image_widths: images::WIDTHS.to_vec(),
            exports_diagnostics: self.exports_diagnostics,
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
        let settings = required_settings(&request.settings)?;
        let url = (self.settings.set_url(settings.url).await).map_err(settings_change_failed)?;
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
        let can_do = self.abilities.can_do().await;
        let automatic = can_do.automatic();
        let hardware = (can_do.hardware.iter())
            .map(|hardware| FoundHardware {
                api: accel_to_proto(hardware.accel).into(),
                gpu: (hardware.gpu.as_ref())
                    .map(|gpu| gpu_to_proto(gpu.id().to_owned(), gpu.name()))
                    .into(),
                tone_maps: hardware.tone_maps,
                tone_mapping: tone_mapping_to_proto(can_do.tone_mapper(Some(hardware))).into(),
                automatic: automatic == Some(hardware),
                ..Default::default()
            })
            .collect();
        Response::ok(GetPlaybackStatusResponse {
            ffmpeg: found(&self.ffmpeg.ffmpeg).into(),
            ffprobe: found(&self.ffmpeg.ffprobe).into(),
            ffmpeg_problem: problem(&self.ffmpeg.ffmpeg),
            ffprobe_problem: problem(&self.ffmpeg.ffprobe),
            hardware,
            has_libx264: can_do.libx264,
            has_zscale: can_do.zscale,
            can_transcode: can_do.transcodes(),
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
        let settings = required_settings(&request.settings)?;
        // An empty id is no GPU, as one stored with none reads back.
        let gpu = (settings.gpu.as_option())
            .map(|gpu| gpu.id)
            .filter(|id| !id.is_empty());
        let hardware = hardware_choice(settings.hardware_acceleration, gpu)?;
        // `limit_from_proto` takes an unknown limit as Original, right for a User's pick but not
        // for a setting an Admin saves.
        if let EnumValue::Unknown(_) = settings.remote_playback_limit {
            return Err(unknown_enum_value(
                "settings.remote_playback_limit",
                "the remote Playback Limit must be Original, 1080p, 720p or 480p",
            ));
        }
        let change = PlaybackChange {
            segment_folder: settings.segment_folder,
            max_transcodes: settings.max_transcodes,
            keep_behind_minutes: settings.keep_behind_minutes,
            hardware,
            remote_limit: limit_from_proto(settings.remote_playback_limit),
        };
        let passed = self.abilities.can_do().await.hardware;
        let stored = (self.settings.change_playback(change, &passed).await)
            .map_err(settings_change_failed)?;
        Response::ok(UpdatePlaybackSettingsResponse {
            settings: playback_settings_to_proto(stored).into(),
            ..Default::default()
        })
    }

    fn get_image_settings<'a>(
        &'a self,
        _ctx: RequestContext,
        _request: ServiceRequest<'_, GetImageSettingsRequest>,
    ) -> impl Future<
        Output = ServiceResult<impl Encodable<GetImageSettingsResponse> + Send + use<'a>>,
    > + Send {
        future::ready(Response::ok(GetImageSettingsResponse {
            settings: self
                .image_settings_to_proto(self.settings.now().images)
                .into(),
            switching: self.images.switching().map(switching_to_proto).into(),
            ..Default::default()
        }))
    }

    async fn update_image_settings(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, UpdateImageSettingsRequest>,
    ) -> ServiceResult<UpdateImageSettingsResponse> {
        let settings = required_settings(&request.settings)?;
        let stored = (self.settings)
            .change_images(settings.resize_images, settings.resized_cache_mib)
            .await
            .map_err(settings_change_failed)?;
        Response::ok(UpdateImageSettingsResponse {
            settings: self.image_settings_to_proto(stored).into(),
            ..Default::default()
        })
    }

    async fn get_default_home_layout(
        &self,
        _ctx: RequestContext,
        _request: ServiceRequest<'_, GetDefaultHomeLayoutRequest>,
    ) -> ServiceResult<GetDefaultHomeLayoutResponse> {
        Response::ok(GetDefaultHomeLayoutResponse {
            layout: self.default_home_layout().await?.into(),
            ..Default::default()
        })
    }

    async fn set_default_home_layout(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, SetDefaultHomeLayoutRequest>,
    ) -> ServiceResult<SetDefaultHomeLayoutResponse> {
        let layout = home_layout_from_proto(request.to_owned_message().layout.into_option())?;
        (self.settings.set_default_home_layout(layout).await).map_err(database_failed)?;
        Response::ok(SetDefaultHomeLayoutResponse {
            layout: self.default_home_layout().await?.into(),
            ..Default::default()
        })
    }

    async fn switch_image_store(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, SwitchImageStoreRequest>,
    ) -> ServiceResult<SwitchImageStoreResponse> {
        let named = request.plugin_id.to_owned();
        let plugin = (!named.is_empty()).then(|| PluginId::from(named.clone()));
        if plugin == self.settings.now().image_store {
            return Err(refuse(
                RefusalReason::REFUSAL_REASON_IMAGE_STORE_IN_USE,
                "that store already keeps the images, so there is nothing to switch",
            )
            .field("plugin_id")
            .param("pluginId", &named)
            .into());
        }
        let target: Arc<dyn Blobs> = match plugin.clone() {
            None => images::local_disk(&self.data_dir),
            Some(id) if self.points.blob_stores().contains(&id) => {
                Arc::new(PluginBlobs::new(self.points.clone(), id))
            }
            Some(_) => {
                return Err(refuse(
                    RefusalReason::REFUSAL_REASON_NOT_A_BLOB_STORE,
                    format!("no enabled Plugin {named} keeps stored images"),
                )
                .field("plugin_id")
                .param("pluginId", &named)
                .into());
            }
        };
        let settings = self.settings.clone();
        let persist: Persist = Box::new(move || {
            Box::pin(
                async move { (settings.set_image_store(plugin).await).map_err(io::Error::other) },
            )
        });
        (self.images.switch_to(target, &named, persist).await)
            .map_err(|refused| switch_refused(&refused, &named))?;
        Response::ok(SwitchImageStoreResponse {
            switching: self.images.switching().map(switching_to_proto).into(),
            ..Default::default()
        })
    }

    async fn list_plays(
        &self,
        _ctx: RequestContext,
        _request: ServiceRequest<'_, ListPlaysRequest>,
    ) -> ServiceResult<ListPlaysResponse> {
        let plays = self.running.plays().await.map_err(database_failed)?;
        Response::ok(ListPlaysResponse {
            plays: plays.into_iter().map(play_to_proto).collect(),
            ..Default::default()
        })
    }

    async fn stop_play(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, StopPlayRequest>,
    ) -> ServiceResult<StopPlayResponse> {
        let StopPlayRequest {
            plan_id, message, ..
        } = request.to_owned_message();
        (self.running.stop_by_admin(plan_id, message).await).map_err(|MessageTooLong| {
            refuse(
                RefusalReason::REFUSAL_REASON_MESSAGE_TOO_LONG,
                format!("the message can be at most {STOP_MESSAGE_LENGTH} characters"),
            )
            .field("message")
            .param("maxLength", &STOP_MESSAGE_LENGTH)
        })?;
        Response::ok(StopPlayResponse::default())
    }
}

/// `play` as `ListPlays` sends it.
fn play_to_proto(play: PlayNow) -> Play {
    let transcodes = play.playback.method == PlaybackMethod::PLAYBACK_METHOD_TRANSCODE;
    let transcodes_on = transcodes.then(|| TranscodeHardware {
        api: (play.hardware.as_ref())
            .map_or(
                HardwareAccelerationProto::HARDWARE_ACCELERATION_OFF,
                |hardware| accel_to_proto(hardware.accel),
            )
            .into(),
        gpu: (play.hardware.and_then(|hardware| hardware.gpu))
            .map(|gpu| gpu_to_proto(gpu.id().to_owned(), gpu.name()))
            .into(),
        ..Default::default()
    });
    Play {
        playback: play.playback.into(),
        title: play.title.unwrap_or_default(),
        position_ms: play.progress.map(|progress| progress.position_ms),
        duration_ms: play.progress.map_or(0, |progress| progress.duration_ms),
        transcodes_on: transcodes_on.into(),
        ..Default::default()
    }
}

impl ServerServiceHandler {
    /// The default Home layout with every Home row Home could show now, as the API has it.
    async fn default_home_layout(&self) -> Result<proto::HomeLayout, ConnectError> {
        let placeable = self.home_rows.placeable().await.map_err(database_failed)?;
        let rows = std::iter::once((HomeRowId::ContinueWatching, String::new()))
            .chain(placeable)
            .collect();
        let layout = self.settings.now().default_home_layout;
        let rows = layout.arrange(rows);
        Ok(proto::HomeLayout {
            featured_hidden: !layout.featured_shown,
            rows: (rows.into_iter())
                .map(|((id, heading), shown)| layout_row_to_proto(id, heading, shown))
                .collect(),
            ..Default::default()
        })
    }

    /// The image settings `values` as the API has them, with the store in use.
    fn image_settings_to_proto(&self, values: ImageValues) -> proto::ImageSettings {
        proto::ImageSettings {
            resize_images: values.resize,
            resized_cache_mib: values.resized_cache_mib.map(NonZeroU32::get),
            store_plugin_id: (self.settings.now().image_store)
                .map_or_default(|plugin| plugin.to_string()),
            ..Default::default()
        }
    }
}

/// A row of a Home layout, headed `heading` if a Plugin gives it, as the API has it.
fn layout_row_to_proto(id: HomeRowId, heading: String, shown: bool) -> HomeLayoutRow {
    let row = match id {
        HomeRowId::ContinueWatching => {
            LayoutRow::Kernel(proto::KernelHomeRow::KERNEL_HOME_ROW_CONTINUE_WATCHING.into())
        }
        HomeRowId::Shared { plugin, key } | HomeRowId::Own { plugin, key } => {
            LayoutRow::Plugin(Box::new(proto::PluginHomeRow {
                plugin_id: plugin.to_string(),
                key,
                heading,
                ..Default::default()
            }))
        }
        HomeRowId::UsersOwn { plugin } => LayoutRow::PluginUsersRows(plugin.to_string()),
    };
    HomeLayoutRow {
        row: Some(row),
        hidden: !shown,
        ..Default::default()
    }
}

/// The Home layout a save asks for in `layout`: refused when it's missing, or a row names none or
/// a Kernel row the Server doesn't know.
pub(super) fn home_layout_from_proto(
    layout: Option<proto::HomeLayout>,
) -> Result<HomeLayout, ConnectError> {
    let layout = layout.ok_or_else(|| {
        refuse(
            RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
            "the layout to save is missing",
        )
        .field("layout")
    })?;
    let place = |placed: &HomeLayoutRow| -> Result<_, ConnectError> {
        Ok(PlacedRow {
            row: home_row_from_proto(placed, "layout.rows")?,
            shown: !placed.hidden,
        })
    };
    Ok(HomeLayout {
        featured_shown: !layout.featured_hidden,
        rows: layout.rows.iter().map(place).collect::<Result<_, _>>()?,
    })
}

/// The Home row `placed` names, refused as the request's `field` if it names none the Kernel
/// knows.
pub(super) fn home_row_from_proto(
    placed: &HomeLayoutRow,
    field: &str,
) -> Result<HomeRowId, ConnectError> {
    Ok(match &placed.row {
        Some(LayoutRow::Kernel(kernel)) => match kernel.as_known() {
            Some(proto::KernelHomeRow::KERNEL_HOME_ROW_CONTINUE_WATCHING) => {
                HomeRowId::ContinueWatching
            }
            _ => {
                return Err(unknown_enum_value(
                    field,
                    "the Kernel row must be Continue Watching",
                ));
            }
        },
        Some(LayoutRow::Plugin(row)) => HomeRowId::Shared {
            plugin: PluginId::from(row.plugin_id.clone()),
            key: row.key.clone(),
        },
        Some(LayoutRow::PluginUsersRows(plugin)) => HomeRowId::UsersOwn {
            plugin: PluginId::from(plugin.clone()),
        },
        None => {
            return Err(refuse(
                RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
                "each row must name a Home row",
            )
            .field(field)
            .into());
        }
    })
}

/// A switch to another image store as the API has it.
fn switching_to_proto(switching: Switching) -> ImageStoreSwitch {
    ImageStoreSwitch {
        to_plugin_id: switching.to,
        copied: u32::try_from(switching.copied).unwrap_or(u32::MAX),
        total: u32::try_from(switching.total).unwrap_or(u32::MAX),
        failure: switching.failed.unwrap_or_default(),
        ..Default::default()
    }
}

/// The refusal saying why a switch to the store the client named `named` didn't start.
fn switch_refused(refused: &SwitchRefused, named: &str) -> ConnectError {
    match refused {
        SwitchRefused::Running => refuse(
            RefusalReason::REFUSAL_REASON_IMAGE_STORE_SWITCHING,
            refused.to_string(),
        ),
        SwitchRefused::FailedProbe(reason) => refuse(
            RefusalReason::REFUSAL_REASON_IMAGE_STORE_FAILED_CHECK,
            refused.to_string(),
        )
        .field("plugin_id")
        .param("pluginId", &named)
        .param("reason", reason),
    }
    .into()
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
        keep_behind_minutes: settings.keep_behind_minutes.map(NonZeroU32::get),
        remote_playback_limit: limit_to_proto(settings.remote_limit).into(),
        hardware_acceleration: hardware_choice_to_proto(&settings.hardware).into(),
        gpu: (settings.hardware.gpu())
            .map(|gpu| gpu_to_proto(gpu.id.clone(), gpu.name.clone()))
            .into(),
        ..Default::default()
    }
}

/// The `settings` message an update carries.
///
/// # Errors
///
/// `FIELD_REQUIRED` naming `settings` when the request left it out, which buffa would otherwise
/// read as every field's default and save as a reset.
fn required_settings<T>(settings: &MessageFieldView<T>) -> Result<&T, ConnectError> {
    settings.as_option().ok_or_else(|| {
        refuse(
            RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
            "the settings to save are missing",
        )
        .field("settings")
        .into()
    })
}

/// The refusal naming the field whose value broke a rule, or a generic error if storing failed.
fn settings_change_failed(err: server_settings::ChangeError) -> ConnectError {
    use server_settings::ChangeError::{Database, Invalid as Refused};
    match err {
        Database(err) => return database_failed(err),
        Refused(Invalid::NotAUrl) => refuse(
            RefusalReason::REFUSAL_REASON_NOT_A_URL,
            "the Server URL must be an http or https URL with no user name, password, ? or #, \
             such as https://nama.example.com",
        )
        .field("settings.url"),
        Refused(Invalid::FolderNotAbsolute { folder }) => refuse(
            RefusalReason::REFUSAL_REASON_FOLDER_NOT_ABSOLUTE,
            "the segment folder must be an absolute path, such as /dev/shm/nama",
        )
        .field("settings.segment_folder")
        .param("folder", &folder),
        Refused(Invalid::FolderNotWritable { folder }) => refuse(
            RefusalReason::REFUSAL_REASON_FOLDER_NOT_WRITABLE,
            "the Server can't create the segment folder or write files in it",
        )
        .field("settings.segment_folder")
        .param("folder", &folder),
        Refused(Invalid::NoTranscodes) => refuse(
            RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
            "the most Transcodes at once must be at least 1",
        )
        .field("settings.max_transcodes")
        .param("minTranscodes", &1),
        Refused(Invalid::NothingKeptBehind) => refuse(
            RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
            "the time kept for skipping back must be at least 1 minute",
        )
        .field("settings.keep_behind_minutes")
        .param("minMinutes", &1),
        Refused(Invalid::NoResizedCache) => refuse(
            RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
            "the resized copies must be allowed at least 1 MiB",
        )
        .field("settings.resized_cache_mib")
        .param("minMib", &1),
        Refused(Invalid::GpuNotOffered { id }) => refuse(
            RefusalReason::REFUSAL_REASON_GPU_NOT_OFFERED,
            "the GPU must be one the hardware acceleration passed its test on when the Server \
             started, as the Playback status lists them",
        )
        .field("settings.gpu")
        .param("gpuId", &id),
    }
    .into()
}

/// The choice of what Transcodes run on that `choice` names, on the GPU at the PCI address `gpu`
/// if given, which [`Settings`] names.
///
/// # Errors
///
/// `UNKNOWN_ENUM_VALUE` for a `choice` this Server doesn't know, checked first, and
/// `GPU_NOT_OFFERED` for a GPU with Automatic or Off.
///
/// [`Settings`]: server_settings::Settings
fn hardware_choice(
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
fn gpu_to_proto(id: String, name: String) -> proto::Gpu {
    proto::Gpu {
        id,
        name,
        ..Default::default()
    }
}

/// What tone-maps HDR as the API names it.
const fn tone_mapping_to_proto(tone_mapper: Option<ToneMapper>) -> ToneMapping {
    match tone_mapper {
        None => ToneMapping::TONE_MAPPING_NONE,
        Some(ToneMapper::Hardware) => ToneMapping::TONE_MAPPING_GPU,
        Some(ToneMapper::Software) => ToneMapping::TONE_MAPPING_SOFTWARE,
    }
}
