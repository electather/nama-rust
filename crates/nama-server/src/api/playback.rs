//! How each signed-in User's Device plays a Version: `nama.v1.PlaybackService`.

use buffa::EnumValue;
use connectrpc::{ConnectError, RequestContext, Response, ServiceRequest, ServiceResult};
use nama_proto::nama::v1::{
    self as proto, PlanPlaybackRequest, PlanPlaybackResponse, PlaybackService, RefusalReason,
    StopPlaybackRequest, StopPlaybackResponse,
};

use crate::{
    playback::{
        self, DeviceDecoding, Method, PickedLimit, Plan, PlaybackLimit, method_to_proto,
        running::{RunningPlans, StartError},
    },
    scanner::Scanner,
    server_settings::PlaybackSettings,
    tracks::track_to_proto,
};

use super::{caller, client_ip, database_failed, refuse, version_not_found};

/// `nama.v1.PlaybackService`: how each signed-in User's Device plays a Version.
pub(super) struct PlaybackServiceHandler {
    pub(super) scanner: Scanner,
    pub(super) running: RunningPlans,
    pub(super) settings: PlaybackSettings,
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
        let audio = (request.audio_track).and_then(|place| usize::try_from(place).ok());
        let subtitle = (request.subtitle_track).and_then(|place| usize::try_from(place).ok());
        let decoding = request.device_decoding.into_option().unwrap_or_default();
        let decoding = DeviceDecoding {
            content_types: decoding.content_types,
            hdr: decoding.hdr,
            hls: decoding.native_hls || decoding.mse,
            audio_channels: decoding.audio_channels,
        };
        let failed = (request.failed_methods.iter())
            .filter_map(|method| method_from_proto(*method))
            .collect();
        let plan = playback::plan(
            &self.scanner,
            &self.running,
            caller,
            request.version_id,
            decoding,
            failed,
            audio,
            picked_limit(request.playback_limit),
            client,
            self.settings.now(),
            subtitle,
        )
        .await
        .map_err(|err| match err {
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
            err @ (StartError::Keyframes(_) | StartError::Run(_)) => {
                tracing::error!(
                    version = request.version_id,
                    "could not start a plan: {err}"
                );
                ConnectError::internal("the Server failed")
            }
        })?
        .ok_or_else(version_not_found)?;
        Response::ok(PlanPlaybackResponse {
            plan: plan_to_proto(plan).into(),
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
}

/// Each Playback Limit below Original, by the API's name for it.
const NAMED_LIMITS: [(PlaybackLimit, proto::PlaybackLimit); 3] = [
    (
        PlaybackLimit::P1080,
        proto::PlaybackLimit::PLAYBACK_LIMIT_1080P,
    ),
    (
        PlaybackLimit::P720,
        proto::PlaybackLimit::PLAYBACK_LIMIT_720P,
    ),
    (
        PlaybackLimit::P480,
        proto::PlaybackLimit::PLAYBACK_LIMIT_480P,
    ),
];

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

/// `limit` as the API names it, `None` being Original.
pub(super) fn limit_to_proto(limit: Option<PlaybackLimit>) -> proto::PlaybackLimit {
    (NAMED_LIMITS.into_iter())
        .find(|(ours, _)| Some(*ours) == limit)
        .map_or(
            proto::PlaybackLimit::PLAYBACK_LIMIT_ORIGINAL,
            |(_, theirs)| theirs,
        )
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

fn plan_to_proto(plan: Plan) -> proto::PlaybackPlan {
    // A place past `u32::MAX` is in no file ffprobe reads.
    let place = |place: usize| u32::try_from(place).ok();
    let places = |places: Vec<usize>| places.into_iter().filter_map(place).collect();
    proto::PlaybackPlan {
        id: plan.id.unwrap_or_default(),
        method: method_to_proto(plan.method()).into(),
        url: plan.url.unwrap_or_default(),
        tracks: plan.tracks.iter().map(track_to_proto).collect(),
        reasons: plan.reasons,
        playback_limit: limit_to_proto(plan.limit).into(),
        audio_track: plan.audio.and_then(place),
        subtitle_track: plan.burn_in.and_then(place),
        audio_tracks: places(plan.offers.audio),
        burn_in_tracks: places(plan.offers.burn_in),
        playback_limits: (plan.offers.limits.into_iter())
            .map(|limit| limit_to_proto(Some(limit)).into())
            .collect(),
        subtitles: (plan.subtitles.iter())
            .map(|subtitle| proto::PlaybackSubtitle {
                track: track_to_proto(&subtitle.track).into(),
                url: subtitle.url.clone(),
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    }
}
