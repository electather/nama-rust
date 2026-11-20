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
    SetSubtitleBackgroundResponse, SetSubtitlePreferenceRequest, SetSubtitlePreferenceResponse,
    SetSubtitleSizeRequest, SetSubtitleSizeResponse, StopPlaybackRequest, StopPlaybackResponse,
    SubtitleKind, UsersAudioPreference, UsersSubtitlePreference,
};

use crate::{
    db::Database,
    ffmpeg::Abilities,
    playback::{
        self, Asked, AskedAudio, AskedSubtitle, Description, DeviceDecoding, DownloadError, Method,
        NAMED_LIMITS, PickedLimit, Planned, Preferred, Probing, ProbingError, RunningPlans,
        Segment, SegmentKind, StartError, StartedPlan, SubtitleAppearance, SubtitleDownloads,
        SubtitlePreference, TrackKind, TrackPreferenceError, TrackPreferences, is_autoplay_on,
        is_language, limit_to_proto, method_to_proto, reason_to_proto, segments_of, set_autoplay,
        set_subtitle_background, set_subtitle_size, subtitle_appearance_of, track_to_proto,
    },
    server_settings::{PlaybackLimit, PlaybackSettings},
};

use super::{caller, client_ip, database_failed, refuse, unknown_enum_value};

/// `nama.v1.PlaybackService`: how each signed-in User's Device plays a Version.
pub(super) struct PlaybackServiceHandler {
    pub(super) probing: Probing,
    pub(super) abilities: Abilities,
    pub(super) running: RunningPlans,
    pub(super) settings: PlaybackSettings,
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
        let decoding = request.device_decoding.into_option().unwrap_or_default();
        let subtitle = match (request.subtitle_track, request.no_subtitles) {
            // A place past `usize::MAX` is in no file ffprobe reads.
            (Some(place), _) => {
                usize::try_from(place).map_or(AskedSubtitle::None, AskedSubtitle::BurnIn)
            }
            (None, true) => AskedSubtitle::None,
            (None, false) => AskedSubtitle::Preferred(
                (self.preferences)
                    .subtitle(caller.user.id, request.version_id)
                    .await
                    .map_err(database_failed)?,
            ),
        };
        let audio =
            (self.asked_audio(caller.user.id, request.version_id, request.audio_track)).await?;
        let asked = Asked {
            version: request.version_id,
            decoding: DeviceDecoding {
                containers: known(&decoding.containers),
                video_codecs: known(&decoding.video_codecs),
                audio_codecs: known(&decoding.audio_codecs),
                hdr: decoding.hdr,
                h264_10bit: decoding.h264_10bit,
                hls: decoding.native_hls || decoding.mse,
                audio_channels: decoding.audio_channels,
                subtitle_formats: known(&decoding.subtitle_formats),
            },
            failed: (request.failed_methods.iter())
                .filter_map(|method| method_from_proto(*method))
                .collect(),
            audio,
            subtitle,
            limit: picked_limit(request.playback_limit),
            // Past `i64::MAX` is past the end of any Version, so it plays from its last segment.
            start_ms: i64::try_from(request.start_position_ms).unwrap_or(i64::MAX),
            client,
        };
        let plan = playback::plan(
            &self.probing,
            &self.abilities,
            &self.running,
            caller,
            asked,
            self.settings.now(),
        )
        .await
        .map_err(|err| plan_failed(err, request.version_id))?
        .ok_or_else(version_not_found)?;
        let segments =
            (segments_of(&self.db, request.version_id).await).map_err(database_failed)?;
        Response::ok(PlanPlaybackResponse {
            plan: plan_to_proto(plan, segments).into(),
            ..Default::default()
        })
    }

    async fn stop_playback(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, StopPlaybackRequest>,
    ) -> ServiceResult<StopPlaybackResponse> {
        let device = caller(&ctx)?.device.id;
        self.running.stop_for(request.plan_id, device).await;
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
                Pick::Subtitle(track)
            }
            (EnumValue::Known(PickedTrackKind::PICKED_TRACK_KIND_AUDIO), Some(track)) => {
                Pick::Audio(track)
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
        let kept_for =
            (self.preferences.kept_for(request.version_id).await).map_err(|err| match err {
                TrackPreferenceError::NoSuchVersion => version_not_found(),
                TrackPreferenceError::Database(err) => database_failed(err),
            })?;
        // An Unmatched Item keeps nothing, as it keeps no Watch State (ADR 0013): its Tracks aren't
        // read, so the pick never fails, whatever it names.
        let Some(kept_for) = kept_for else {
            return Response::ok(ReportTrackPickResponse::default());
        };
        let version = request.version_id;
        let stored = match pick {
            Pick::Subtitle(None) => {
                (self.preferences)
                    .keep_subtitle(user, kept_for, SubtitlePreference::Off)
                    .await
            }
            Pick::Subtitle(Some(place)) => {
                let picked = self.picked(version, place, is_subtitle).await?;
                (self.preferences)
                    .keep_subtitle(user, kept_for, SubtitlePreference::Like(picked))
                    .await
            }
            Pick::Audio(place) => {
                let picked = self.picked(version, place, is_audio).await?;
                self.preferences.keep_audio(user, kept_for, picked).await
            }
        };
        stored.map_err(database_failed)?;
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
        _ctx: RequestContext,
        request: ServiceRequest<'_, SearchSubtitlesRequest>,
    ) -> ServiceResult<SearchSubtitlesResponse> {
        let request = request.to_owned_message();
        if request.languages.is_empty() {
            let refused = refuse(
                RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
                "name at least one language",
            );
            return Err(refused.field("languages").into());
        }
        let found = (self.downloads)
            .search(request.version_id, request.languages)
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
        _ctx: RequestContext,
        request: ServiceRequest<'_, DownloadSubtitleRequest>,
    ) -> ServiceResult<DownloadSubtitleResponse> {
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
            .download(request.version_id, &subtitle.plugin_id.into(), found)
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

/// The Track a `ReportTrackPick` call says the User picked, by its place in the Version's Tracks.
enum Pick {
    /// A subtitle, or `None` for subtitles turned off.
    Subtitle(Option<u32>),
    Audio(u32),
}

const fn is_subtitle(kind: &TrackKind) -> bool {
    matches!(kind, TrackKind::Subtitle { .. })
}

const fn is_audio(kind: &TrackKind) -> bool {
    matches!(kind, TrackKind::Audio { .. })
}

impl PlaybackServiceHandler {
    /// What a plan request from the User `user` for the Version `version` asks of audio: the
    /// Track at `place`, or else the Server's choice by their Audio Preference.
    async fn asked_audio(
        &self,
        user: i64,
        version: i64,
        place: Option<u32>,
    ) -> Result<AskedAudio, ConnectError> {
        Ok(match place {
            // As for subtitles, a place past `usize::MAX` is in no file.
            Some(place) => {
                usize::try_from(place).map_or(AskedAudio::Preferred(None), AskedAudio::Chosen)
            }
            None => AskedAudio::Preferred(
                (self.preferences)
                    .audio(user, version)
                    .await
                    .map_err(database_failed)?,
            ),
        })
    }

    /// What describes the Track at `place` in the Version `version`'s Tracks, probing it first if
    /// they aren't current. Refused unless `is_kind` says it is of the kind picked.
    async fn picked(
        &self,
        version: i64,
        place: u32,
        is_kind: fn(&TrackKind) -> bool,
    ) -> Result<Description, ConnectError> {
        let probe = match self.probing.current(version).await {
            Ok(probe) => Some(probe),
            // Its Tracks aren't known, so no plan offered one to pick.
            Err(ProbingError::Probe(_)) => None,
            Err(ProbingError::NoSuchVersion(_)) => return Err(version_not_found()),
            Err(ProbingError::Database(err)) => return Err(database_failed(err)),
        };
        let track = (probe.as_ref())
            .zip(usize::try_from(place).ok())
            .and_then(|(probe, place)| probe.tracks.get(place))
            .filter(|track| is_kind(&track.kind));
        track.map(Description::of).ok_or_else(|| {
            refuse(
                RefusalReason::REFUSAL_REASON_TRACK_NOT_FOUND,
                "the Version has no Track of the kind picked at that place",
            )
            .field("track")
            .into()
        })
    }
}

/// The Playback Limit `limit` names, `None` being Original, as is one not picked or one newer
/// than this Server.
pub(super) fn limit_from_proto(limit: EnumValue<proto::PlaybackLimit>) -> Option<PlaybackLimit> {
    (NAMED_LIMITS.into_iter())
        .find(|(_, theirs)| EnumValue::Known(*theirs) == limit)
        .map(|(ours, _)| ours)
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

fn plan_to_proto(planned: Planned, segments: Vec<Segment>) -> proto::PlaybackPlan {
    let (id, url, plan) = match planned {
        Planned::Started(StartedPlan { id, url, plan }) => (id, url, plan),
        // The API's plan id 0 is no plan.
        Planned::Unplayable(plan) => (0, String::new(), plan),
    };
    // A place past `u32::MAX` is in no file ffprobe reads.
    let place = |place: usize| u32::try_from(place).ok();
    let places = |places: Vec<usize>| places.into_iter().filter_map(place).collect();
    proto::PlaybackPlan {
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
        subtitles: (plan.subtitles.iter())
            .map(|subtitle| proto::PlaybackSubtitle {
                track: track_to_proto(&subtitle.track).into(),
                place: place(subtitle.place).unwrap_or_default(),
                url: subtitle.url.clone(),
                format: subtitle.format.into(),
                font_urls: subtitle.font_urls.clone(),
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
        segments: segments.into_iter().map(segment_to_proto).collect(),
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
