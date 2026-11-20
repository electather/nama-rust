//! Each signed-in User's progress through what they play: `nama.v1.WatchStateService`.

use connectrpc::{ConnectError, RequestContext, Response, ServiceRequest, ServiceResult};
use nama_proto::nama::v1::{
    ExternalId, RefusalReason, ReportProgressRequest, ReportProgressResponse, SetPlayedRequest,
    SetPlayedResponse, WatchStateService, set_played_request::Target,
};

use crate::{
    browse,
    library::{Progress, TitleTarget},
    playback::{ReportedProgress, RunningPlans},
};

use super::{
    browse::{Fields, refused},
    caller, refuse, required_id,
};

/// `nama.v1.WatchStateService`: each signed-in User's progress through what they play.
pub(super) struct WatchStateServiceHandler {
    pub(super) browse: browse::Browse,
    pub(super) running: RunningPlans,
}

#[expect(
    refining_impl_trait,
    reason = "plain `async fn` returning the owned response is the form the generated trait \
              documents; the refined signature isn't visible outside this crate"
)]
impl WatchStateService for WatchStateServiceHandler {
    async fn report_progress(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, ReportProgressRequest>,
    ) -> ServiceResult<ReportProgressResponse> {
        let caller = caller(&ctx)?;
        let progress =
            Progress::new(request.position_ms, request.duration_ms).ok_or_else(|| {
                refuse(
                    RefusalReason::REFUSAL_REASON_DURATION_OUT_OF_RANGE,
                    "the duration must be more than zero and less than 2^63 milliseconds",
                )
                .field("duration_ms")
                .param("minDurationMs", &1)
                .param("maxDurationMs", &i64::MAX)
            })?;
        self.browse
            .report(caller.user.id, request.version_id, progress)
            .await
            .map_err(refused(Fields {
                version: "version_id",
                ..Fields::NONE
            }))?;
        let reported = ReportedProgress {
            position_ms: request.position_ms,
            duration_ms: request.duration_ms,
        };
        (self.running).report_progress(request.version_id, caller.device.id, reported);
        Response::ok(ReportProgressResponse::default())
    }

    async fn set_played(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, SetPlayedRequest>,
    ) -> ServiceResult<SetPlayedResponse> {
        let user = caller(&ctx)?.user.id;
        let SetPlayedRequest {
            library_id: library,
            target,
            played,
            ..
        } = request.to_owned_message();
        let target = target.ok_or_else(|| {
            refuse(
                RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
                "say which Movie, Episode, Season or Show to mark",
            )
            .field("target")
        })?;
        let (target, fields) = match target {
            Target::MovieId(id) => movie_target(*id)?,
            Target::EpisodeId(id) => {
                let fields = Fields {
                    title: "episode_id",
                    ..Fields::LIBRARY
                };
                let id = required_id(Some(*id), fields.title, "Episode")?;
                (TitleTarget::Episode(id), fields)
            }
            Target::Season(season) => {
                let fields = Fields {
                    title: "season.show_id",
                    season: "season.season_number",
                    ..Fields::LIBRARY
                };
                let show = required_id(season.show_id.into_option(), fields.title, "Show")?;
                let number = i64::from(season.season_number);
                (TitleTarget::Season { show, number }, fields)
            }
            Target::ShowId(id) => show_target(*id)?,
        };
        let marked = self.browse.mark(user, library, target, played).await;
        marked.map_err(refused(fields))?;
        Response::ok(SetPlayedResponse::default())
    }
}

/// A request's `movie_id`, and the fields its refusals name.
fn movie_target(id: ExternalId) -> Result<(TitleTarget, Fields), ConnectError> {
    let fields = Fields {
        title: "movie_id",
        ..Fields::LIBRARY
    };
    let id = required_id(Some(id), fields.title, "Movie")?;
    Ok((TitleTarget::Movie(id), fields))
}

/// A request's `show_id`, and the fields its refusals name.
fn show_target(id: ExternalId) -> Result<(TitleTarget, Fields), ConnectError> {
    let fields = Fields {
        title: "show_id",
        ..Fields::LIBRARY
    };
    let id = required_id(Some(id), fields.title, "Show")?;
    Ok((TitleTarget::Show(id), fields))
}
