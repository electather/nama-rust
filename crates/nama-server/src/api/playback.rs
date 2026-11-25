//! How each signed-in User's Device plays a Version: `nama.v1.PlaybackService`.

use std::sync::Arc;

use buffa::EnumValue;
use connectrpc::{ConnectError, RequestContext, Response, ServiceRequest, ServiceResult};
use nama_plugin_api::contract::SubtitleResult;
use nama_proto::nama::v1::{
    self as proto, DownloadSubtitleRequest, DownloadSubtitleResponse, FoundSubtitle,
    GetAutoplayRequest, GetAutoplayResponse, GetSubtitleAppearanceRequest,
    GetSubtitleAppearanceResponse, GetTrackPreferencesRequest, GetTrackPreferencesResponse,
    PickedTrackKind, PlanPlaybackRequest, PlanPlaybackResponse, PlaybackService, RefusalReason,
    ReportTrackPickRequest, ReportTrackPickResponse, SearchSubtitlesRequest,
    SearchSubtitlesResponse, SetAudioPreferenceRequest, SetAudioPreferenceResponse,
    SetAutoplayRequest, SetAutoplayResponse, SetSubtitleBackgroundRequest,
    SetSubtitleBackgroundResponse, SetSubtitleOffsetRequest, SetSubtitleOffsetResponse,
    SetSubtitlePreferenceRequest, SetSubtitlePreferenceResponse, SetSubtitleSizeRequest,
    SetSubtitleSizeResponse, StopPlaybackRequest, StopPlaybackResponse, SubtitleKind,
    UsersAudioPreference, UsersSubtitlePreference,
};

use crate::{
    db::Database,
    playback::{
        Asked, AskedSubtitle, Description, DeviceDecoding, DownloadError, Method, NAMED_LIMITS,
        Pick, PickError, PickedLimit, Planned, Plans, Preferred, Segment, SegmentKind, StartError,
        StartedPlan, SubtitleAppearance, SubtitleDownloads, SubtitlePreference, TrackPreferences,
        is_autoplay_on, is_language, limit_to_proto, method_to_proto, reason_to_proto,
        set_autoplay, set_subtitle_background, set_subtitle_size, subtitle_appearance_of,
        track_to_proto,
    },
    server_settings::PlaybackLimit,
};

use super::{caller, database_failed, refuse, sign_in::client_ip, unknown_enum_value};

/// `nama.v1.PlaybackService`: how each signed-in User's Device plays a Version.
pub(super) struct PlaybackServiceHandler {
    pub(super) plans: Plans,
    pub(super) preferences: TrackPreferences,
    pub(super) downloads: SubtitleDownloads,
    pub(super) db: Arc<Database>,
}

#[expect(
    refining_impl_trait,
    reason = "plain `async fn` returning the owned response is the form the generated trait \
              documents; the refined signature isn't visible outside this crate"
)]
impl PlaybackService for PlaybackServiceHandler {
    async fn plan_playback(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, PlanPlaybackRequest>,
    ) -> ServiceResult<PlanPlaybackResponse> {
        let caller = caller(&ctx)?;
        let client = client_ip(&ctx)?;
        // The rest of the request is for steps still to come (ADR 0027).
        let request = request.to_owned_message();
        // ponytail: `decoding.evidence` goes unread, unset meaning declared, since nothing passes
        // audio through yet; read it once something does, which needs exact (ADR 0027).
        let decoding = request.device_decoding.into_option().unwrap_or_default();
        let subtitle = match (request.subtitle_track, request.no_subtitles) {
            // A place past `usize::MAX` is in no file ffprobe reads.
            (Some(place), _) => {
                usize::try_from(place).map_or(AskedSubtitle::None, AskedSubtitle::BurnIn)
            }
            (None, true) => AskedSubtitle::None,
            (None, false) => AskedSubtitle::Preferred,
        };
        let asked = Asked {
            version: request.version_id,
            decoding: DeviceDecoding {
                containers: known(&decoding.containers),
                video_codecs: known(&decoding.video_codecs),
                audio_codecs: known(&decoding.audio_codecs),
                hdr: decoding.hdr,
                dolby_vision_profiles: decoding.dolby_vision_profiles,
                h264_10bit: decoding.h264_10bit,
                hevc_10bit: decoding.hevc_10bit,
                hls: decoding.native_hls || decoding.mse,
                audio_channels: decoding.audio_channels,
                subtitle_formats: known(&decoding.subtitle_formats),
            },
            failed: (request.failed_methods.iter())
                .filter_map(|method| method_from_proto(*method))
                .collect(),
            failed_recipes: request.failed_recipes.into_iter().collect(),
            // A place past `usize::MAX` is in no file, so the Server chooses.
            audio: (request.audio_track).and_then(|place| usize::try_from(place).ok()),
            subtitle,
            limit: picked_limit(request.playback_limit),
            // Past `i64::MAX` is past the end of any Version, so it plays from its last segment.
            start_ms: i64::try_from(request.start_position_ms).unwrap_or(i64::MAX),
            client,
        };
        let planned = (self.plans.plan(caller, asked).await)
            .map_err(|err| plan_failed(err, request.version_id))?
            .ok_or_else(version_not_found)?;
        Response::ok(planned_to_proto(planned, self.downloads.has_source()))
    }

    async fn stop_playback(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, StopPlaybackRequest>,
    ) -> ServiceResult<StopPlaybackResponse> {
        let device = caller(&ctx)?.device.id;
        self.plans.stop(request.plan_id, device).await;
        Response::ok(StopPlaybackResponse::default())
    }

    async fn report_track_pick(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, ReportTrackPickRequest>,
    ) -> ServiceResult<ReportTrackPickResponse> {
        let user = caller(&ctx)?.user.id;
        let pick = match (request.kind, request.track) {
            (EnumValue::Known(PickedTrackKind::PICKED_TRACK_KIND_SUBTITLE), track) => {
                Pick::Subtitle(track.map(picked_place))
            }
            (EnumValue::Known(PickedTrackKind::PICKED_TRACK_KIND_AUDIO), Some(track)) => {
                Pick::Audio(picked_place(track))
            }
            (EnumValue::Known(PickedTrackKind::PICKED_TRACK_KIND_AUDIO), None) => {
                return Err(refuse(
                    RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
                    "say which audio Track was picked: audio is never off",
                )
                .field("track")
                .into());
            }
            _ => {
                return Err(unknown_enum_value(
                    "kind",
                    "say which kind of Track was picked: a subtitle or audio",
                ));
            }
        };
        (self.plans.keep_pick(user, request.version_id, pick))
            .await
            .map_err(pick_failed)?;
        Response::ok(ReportTrackPickResponse::default())
    }

    async fn get_track_preferences(
        &self,
        ctx: RequestContext,
        _request: ServiceRequest<'_, GetTrackPreferencesRequest>,
    ) -> ServiceResult<GetTrackPreferencesResponse> {
        let user = caller(&ctx)?.user.id;
        let subtitle = (self.preferences.users_subtitle(user).await).map_err(database_failed)?;
        let audio = (self.preferences.users_audio(user).await).map_err(database_failed)?;
        Response::ok(GetTrackPreferencesResponse {
            subtitle: users_subtitle_to_proto(subtitle).into(),
            audio: UsersAudioPreference {
                language: audio,
                ..Default::default()
            }
            .into(),
            ..Default::default()
        })
    }

    async fn set_subtitle_preference(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, SetSubtitlePreferenceRequest>,
    ) -> ServiceResult<SetSubtitlePreferenceResponse> {
        let user = caller(&ctx)?.user.id;
        let wanted = request
            .to_owned_message()
            .subtitle
            .into_option()
            .unwrap_or_default();
        let (preference, foreign_audio_only) = users_subtitle_from_proto(wanted)?;
        (self.preferences)
            .keep_users_subtitle(user, preference, foreign_audio_only)
            .await
            .map_err(database_failed)?;
        Response::ok(SetSubtitlePreferenceResponse::default())
    }

    async fn set_audio_preference(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, SetAudioPreferenceRequest>,
    ) -> ServiceResult<SetAudioPreferenceResponse> {
        let user = caller(&ctx)?.user.id;
        let wanted = request.to_owned_message().audio.into_option();
        let language = (wanted.and_then(|audio| audio.language))
            .map(|language| checked_language(&language, "audio.language"))
            .transpose()?;
        (self.preferences)
            .keep_users_audio(user, language)
            .await
            .map_err(database_failed)?;
        Response::ok(SetAudioPreferenceResponse::default())
    }

    async fn get_autoplay(
        &self,
        ctx: RequestContext,
        _request: ServiceRequest<'_, GetAutoplayRequest>,
    ) -> ServiceResult<GetAutoplayResponse> {
        let on = is_autoplay_on(&self.db, caller(&ctx)?.user.id)
            .await
            .map_err(database_failed)?;
        Response::ok(GetAutoplayResponse {
            on,
            ..Default::default()
        })
    }

    async fn set_autoplay(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, SetAutoplayRequest>,
    ) -> ServiceResult<SetAutoplayResponse> {
        set_autoplay(&self.db, caller(&ctx)?.user.id, request.on)
            .await
            .map_err(database_failed)?;
        Response::ok(SetAutoplayResponse::default())
    }

    async fn search_subtitles(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, SearchSubtitlesRequest>,
    ) -> ServiceResult<SearchSubtitlesResponse> {
        let user = caller(&ctx)?.user.id;
        let request = request.to_owned_message();
        if request.languages.is_empty() {
            let refused = refuse(
                RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
                "name at least one language",
            );
            return Err(refused.field("languages").into());
        }
        let found = (self.downloads)
            .search(user, request.version_id, request.languages)
            .await
            .map_err(|err| subtitles_failed(err, "languages"))?;
        let subtitles = (found.into_iter())
            .map(|(plugin, found)| FoundSubtitle {
                plugin_id: plugin.to_string(),
                id: found.id,
                language: found.language,
                name: found.name,
                sdh: found.sdh,
                ..Default::default()
            })
            .collect();
        Response::ok(SearchSubtitlesResponse {
            subtitles,
            ..Default::default()
        })
    }

    async fn get_subtitle_appearance(
        &self,
        ctx: RequestContext,
        _request: ServiceRequest<'_, GetSubtitleAppearanceRequest>,
    ) -> ServiceResult<GetSubtitleAppearanceResponse> {
        let SubtitleAppearance { size, background } =
            subtitle_appearance_of(&self.db, caller(&ctx)?.user.id)
                .await
                .map_err(database_failed)?;
        Response::ok(GetSubtitleAppearanceResponse {
            size: size.into(),
            background: background.into(),
            ..Default::default()
        })
    }

    async fn download_subtitle(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, DownloadSubtitleRequest>,
    ) -> ServiceResult<DownloadSubtitleResponse> {
        let user = caller(&ctx)?.user.id;
        let request = request.to_owned_message();
        let Some(subtitle) = request.subtitle.into_option() else {
            let refused = refuse(
                RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
                "name the subtitle to download",
            );
            return Err(refused.field("subtitle").into());
        };
        let found = SubtitleResult {
            id: subtitle.id,
            language: subtitle.language,
            name: subtitle.name,
            sdh: subtitle.sdh,
            ..Default::default()
        };
        (self.downloads)
            .download(user, request.version_id, &subtitle.plugin_id.into(), found)
            .await
            .map_err(|err| subtitles_failed(err, "subtitle.language"))?;
        Response::ok(DownloadSubtitleResponse::default())
    }

    async fn set_subtitle_size(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, SetSubtitleSizeRequest>,
    ) -> ServiceResult<SetSubtitleSizeResponse> {
        let size = match request.to_owned_message().size {
            EnumValue::Known(proto::SubtitleSize::Unspecified) | EnumValue::Unknown(_) => {
                return Err(unknown_enum_value(
                    "size",
                    "the size must be one the Server knows",
                ));
            }
            EnumValue::Known(size) => size,
        };
        set_subtitle_size(&self.db, caller(&ctx)?.user.id, size)
            .await
            .map_err(database_failed)?;
        Response::ok(SetSubtitleSizeResponse::default())
    }

    async fn set_subtitle_background(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, SetSubtitleBackgroundRequest>,
    ) -> ServiceResult<SetSubtitleBackgroundResponse> {
        let background = match request.to_owned_message().background {
            EnumValue::Known(proto::SubtitleBackground::Unspecified) | EnumValue::Unknown(_) => {
                return Err(unknown_enum_value(
                    "background",
                    "the background must be one the Server knows",
                ));
            }
            EnumValue::Known(background) => background,
        };
        set_subtitle_background(&self.db, caller(&ctx)?.user.id, background)
            .await
            .map_err(database_failed)?;
        Response::ok(SetSubtitleBackgroundResponse::default())
    }

    async fn set_subtitle_offset(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, SetSubtitleOffsetRequest>,
    ) -> ServiceResult<SetSubtitleOffsetResponse> {
        let user = caller(&ctx)?.user.id;
        let SetSubtitleOffsetRequest {
            version_id: version,
            track,
            offset_ms,
            ..
        } = request.to_owned_message();
        (self
            .plans
            .keep_offset(user, version, picked_place(track), offset_ms))
        .await
        .map_err(pick_failed)?;
        Response::ok(SetSubtitleOffsetResponse::default())
    }
}

/// The error a subtitle search or download that failed with `err` is answered with, naming the
/// request field `language_field` for a language it refuses.
fn subtitles_failed(err: DownloadError, language_field: &str) -> ConnectError {
    let message = err.to_string();
    match err {
        DownloadError::NoSuchVersion => version_not_found(),
        DownloadError::UnknownLanguage(language) => {
            refuse(RefusalReason::REFUSAL_REASON_UNKNOWN_LANGUAGE, message)
                .field(language_field)
                .param("language", &language)
                .into()
        }
        DownloadError::NoSource(_) => {
            refuse(RefusalReason::REFUSAL_REASON_NO_SUBTITLE_SOURCE, message).into()
        }
        DownloadError::Database(err) => database_failed(err),
        DownloadError::Store(err) => {
            tracing::error!("keeping a downloaded subtitle failed: {err}");
            ConnectError::internal("the Server couldn't keep the subtitle")
        }
    }
}

/// The error a Track pick or Subtitle Offset that couldn't be kept is answered with.
fn pick_failed(err: PickError) -> ConnectError {
    match err {
        PickError::NoSuchVersion => version_not_found(),
        err @ PickError::TrackNotFound => refuse(
            RefusalReason::REFUSAL_REASON_TRACK_NOT_FOUND,
            err.to_string(),
        )
        .field("track")
        .into(),
        PickError::Database(err) => database_failed(err),
    }
}

/// The error a plan for the Version `version` that couldn't start is answered with.
fn plan_failed(err: StartError, version: i64) -> ConnectError {
    match err {
        StartError::Database(err) => database_failed(err),
        StartError::Busy(max) => refuse(
            RefusalReason::REFUSAL_REASON_SERVER_BUSY,
            format!(
                "only a Transcode would play it here, and the Server is already running as \
                 many as it may at once, {max}; try again once one ends"
            ),
        )
        .param("maxTranscodes", &max)
        .into(),
        StartError::SegmentFolder { folder, source } => {
            let folder = folder.display();
            tracing::warn!(
                version,
                "could not create a plan's folder in the segment folder, {folder}: {source}"
            );
            refuse(
                RefusalReason::REFUSAL_REASON_SEGMENT_FOLDER_UNWRITABLE,
                format!(
                    "the Server can't write to its segment folder, {folder}, so it can't \
                     Remux or Transcode this; an Admin can set another under Playback"
                ),
            )
            .param("folder", &folder)
            .into()
        }
        err @ (StartError::Keyframes(_) | StartError::Run(_) | StartError::FailedOnServer(_)) => {
            tracing::error!(version, "could not start a plan: {err}");
            ConnectError::internal("the Server failed")
        }
    }
}

/// The User's own Subtitle Preference `preferred`, `None` being one never set, as the API sends
/// it.
fn users_subtitle_to_proto(
    preferred: Option<Preferred<SubtitlePreference>>,
) -> UsersSubtitlePreference {
    let Some(Preferred {
        preference: SubtitlePreference::Like(description),
        foreign_audio_only,
        ..
    }) = preferred
    else {
        return UsersSubtitlePreference::default();
    };
    let kind = match (description.forced, description.sdh) {
        (true, _) => SubtitleKind::SUBTITLE_KIND_FORCED,
        (false, true) => SubtitleKind::SUBTITLE_KIND_SDH,
        (false, false) => SubtitleKind::SUBTITLE_KIND_FULL,
    };
    UsersSubtitlePreference {
        language: description.language,
        kind: kind.into(),
        foreign_audio_only,
        ..Default::default()
    }
}

/// The User's own Subtitle Preference `wanted` names, and whether it wants it only over audio in
/// another language, refusing a language that isn't ISO 639, a kind this Server doesn't know, or
/// forced-only subtitles only over audio in another language.
fn users_subtitle_from_proto(
    wanted: UsersSubtitlePreference,
) -> Result<(SubtitlePreference, bool), ConnectError> {
    let Some(language) = wanted.language else {
        return Ok((SubtitlePreference::Off, false));
    };
    let language = checked_language(&language, "subtitle.language")?;
    let (forced, sdh) = match wanted.kind {
        EnumValue::Known(SubtitleKind::SUBTITLE_KIND_FULL) => (false, false),
        EnumValue::Known(SubtitleKind::SUBTITLE_KIND_SDH) => (false, true),
        EnumValue::Known(SubtitleKind::SUBTITLE_KIND_FORCED) => (true, false),
        EnumValue::Known(SubtitleKind::SUBTITLE_KIND_UNSPECIFIED) | EnumValue::Unknown(_) => {
            return Err(unknown_enum_value(
                "subtitle.kind",
                "say which subtitle in that language: full, SDH or forced-only",
            ));
        }
    };
    if forced && wanted.foreign_audio_only {
        return Err(refuse(
            RefusalReason::REFUSAL_REASON_FOREIGN_AUDIO_ONLY_NOT_FULL,
            "forced-only subtitles can't start only over audio in another language: they're for \
             audio in their own",
        )
        .field("subtitle.foreign_audio_only")
        .into());
    }
    let preference = SubtitlePreference::Like(Description {
        language: Some(language),
        forced,
        sdh,
        title: None,
    });
    Ok((preference, wanted.foreign_audio_only))
}

/// The language a User's own Preference names in the request field `field`, in lower case,
/// refusing one that isn't ISO 639.
fn checked_language(language: &str, field: &str) -> Result<String, ConnectError> {
    if !is_language(language) {
        return Err(refuse(
            RefusalReason::REFUSAL_REASON_UNKNOWN_LANGUAGE,
            format!("{language:?} isn't an ISO 639 language code, such as `en` or `eng`"),
        )
        .field(field)
        .param("language", &language)
        .into());
    }
    // So a client gets back the code it would have sent itself.
    Ok(language.to_ascii_lowercase())
}

/// The place in a Version's Tracks a pick names, a place past `usize::MAX` being past every Track.
fn picked_place(place: u32) -> usize {
    usize::try_from(place).unwrap_or(usize::MAX)
}

/// The Playback Limit `limit` names, `None` being Original, as is one not picked or one newer
/// than this Server.
pub(super) fn limit_from_proto(limit: EnumValue<proto::PlaybackLimit>) -> Option<PlaybackLimit> {
    (NAMED_LIMITS.into_iter())
        .find(|(_, theirs)| EnumValue::Known(*theirs) == limit)
        .map(|(ours, _)| ours)
}

/// The cap for remote playback `limit` names, `None` for none, refusing a value this Server doesn't
/// know, which `limit_from_proto` would take as none.
pub(super) fn cap_from_proto(
    limit: EnumValue<proto::PlaybackLimit>,
) -> Result<Option<PlaybackLimit>, ConnectError> {
    if let EnumValue::Unknown(_) = limit {
        return Err(unknown_enum_value(
            "remote_playback_limit",
            "the cap for remote playback must be Original, 1080p, 720p or 480p",
        ));
    }
    Ok(limit_from_proto(limit))
}

/// The Playback Limit the User picked, as [`limit_from_proto`] has it.
fn picked_limit(limit: EnumValue<proto::PlaybackLimit>) -> PickedLimit {
    if limit == EnumValue::Known(proto::PlaybackLimit::PLAYBACK_LIMIT_UNSPECIFIED) {
        PickedLimit::Unpicked
    } else {
        PickedLimit::Picked(limit_from_proto(limit))
    }
}

/// Those of `values` this Server's schema names, as a newer client may send ones it doesn't.
fn known<E: buffa::Enumeration>(values: &[EnumValue<E>]) -> Vec<E> {
    values.iter().filter_map(EnumValue::as_known).collect()
}

/// `None` for no method, or one newer than this Server, which it can't plan anyway.
fn method_from_proto(method: EnumValue<proto::PlaybackMethod>) -> Option<Method> {
    match method {
        EnumValue::Known(proto::PlaybackMethod::PLAYBACK_METHOD_DIRECT_PLAY) => {
            Some(Method::DirectPlay)
        }
        EnumValue::Known(proto::PlaybackMethod::PLAYBACK_METHOD_REMUX) => Some(Method::Remux),
        EnumValue::Known(proto::PlaybackMethod::PLAYBACK_METHOD_TRANSCODE) => {
            Some(Method::Transcode)
        }
        EnumValue::Known(proto::PlaybackMethod::PLAYBACK_METHOD_UNSPECIFIED)
        | EnumValue::Unknown(_) => None,
    }
}

fn planned_to_proto(planned: Planned, can_search_subtitles: bool) -> PlanPlaybackResponse {
    let (id, url, plan, playable_versions) = match planned {
        Planned::Started(StartedPlan { id, url, plan }) => (id, url, plan, vec![]),
        // The API's plan id 0 is no plan.
        Planned::Unplayable {
            plan,
            playable_versions,
        } => (0, String::new(), plan, playable_versions),
    };
    // A place past `u32::MAX` is in no file ffprobe reads.
    let place = |place: usize| u32::try_from(place).ok();
    let places = |places: Vec<usize>| places.into_iter().filter_map(place).collect();
    let recipe = plan.recipe();
    let plan = proto::PlaybackPlan {
        id,
        method: method_to_proto(plan.method()).into(),
        url,
        tracks: plan.tracks.iter().map(track_to_proto).collect(),
        reasons: plan.reasons.iter().map(reason_to_proto).collect(),
        playback_limit: limit_to_proto(plan.limit).into(),
        audio_track: plan.audio.and_then(place),
        subtitle_track: plan.burn_in.and_then(place),
        starting_subtitle: plan.starting_subtitle.and_then(place),
        audio_tracks: places(plan.offers.audio),
        burn_in_tracks: places(plan.offers.burn_in),
        playback_limits: (plan.offers.limits.into_iter())
            .map(|limit| limit_to_proto(Some(limit)).into())
            .collect(),
        remote_playback_limit: (plan.offers.cap)
            .map_or_else(EnumValue::default, |cap| limit_to_proto(Some(cap)).into()),
        subtitles: (plan.subtitles.iter())
            .map(|subtitle| proto::PlaybackSubtitle {
                track: track_to_proto(&subtitle.track).into(),
                place: place(subtitle.place).unwrap_or_default(),
                url: subtitle.url.clone(),
                format: subtitle.format.into(),
                font_urls: subtitle.font_urls.clone(),
                offset_ms: plan.subtitle_offsets.of(subtitle.place, &subtitle.track),
                ..Default::default()
            })
            .collect(),
        chapters: (plan.chapters.into_iter())
            .map(|chapter| proto::Chapter {
                // Probing never stores one before the file's start.
                start_ms: u64::try_from(chapter.start_ms).unwrap_or_default(),
                title: chapter.title,
                ..Default::default()
            })
            .collect(),
        segments: plan.segments.into_iter().map(segment_to_proto).collect(),
        preview_frames: plan.preview_frames.into(),
        can_search_subtitles,
        recipe,
        ..Default::default()
    };
    PlanPlaybackResponse {
        plan: plan.into(),
        playable_versions,
        ..Default::default()
    }
}

fn segment_to_proto(segment: Segment) -> proto::Segment {
    // Segments are stored only from a Version's start on.
    let ms = |ms: i64| u64::try_from(ms).unwrap_or_default();
    proto::Segment {
        kind: match segment.kind {
            SegmentKind::Intro => proto::SegmentKind::SEGMENT_KIND_INTRO,
            SegmentKind::Recap => proto::SegmentKind::SEGMENT_KIND_RECAP,
            SegmentKind::Credits => proto::SegmentKind::SEGMENT_KIND_CREDITS,
        }
        .into(),
        start_ms: ms(segment.start_ms),
        end_ms: segment.end_ms.map(ms),
        ..Default::default()
    }
}

/// The refusal of a request whose `version_id` names no Version.
fn version_not_found() -> ConnectError {
    refuse(
        RefusalReason::REFUSAL_REASON_VERSION_NOT_FOUND,
        "there is no such Version",
    )
    .field("version_id")
    .into()
}
