//! Each signed-in User's progress through what they play: `nama.v1.WatchStateService`.

use connectrpc::{ConnectError, RequestContext, Response, ServiceRequest, ServiceResult};
use nama_plugin_api::points::WatchStateChange;
use nama_proto::nama::v1::{
    ExternalId, OfflineWatchStateChange, RefusalReason, ReportOfflineWatchStateRequest,
    ReportOfflineWatchStateResponse, ReportProgressRequest, ReportProgressResponse,
    SetHiddenFromContinueWatchingRequest, SetHiddenFromContinueWatchingResponse, SetPlayedRequest,
    SetPlayedResponse, WatchStateService, offline_watch_state_change::Title,
    set_hidden_from_continue_watching_request::Target as Hide, set_played_request::Target,
};

use crate::{
    browse,
    library::{Progress, TitleTarget},
    playback::{ReportedProgress, RunningPlans, viewer_of_caller},
};

use super::{
    browse::{Fields, refused, seen_by},
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
        let progress = Progress::new(request.position_ms, request.duration_ms)
            .ok_or_else(|| duration_out_of_range("duration_ms"))?;
        (seen_by(&self.browse, caller.user.id).await?)
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
        (self.running).report_progress(request.version_id, viewer_of_caller(caller), reported);
        Response::ok(ReportProgressResponse::default())
    }

    async fn report_offline_watch_state(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, ReportOfflineWatchStateRequest>,
    ) -> ServiceResult<ReportOfflineWatchStateResponse> {
        let user = caller(&ctx)?.user.id;
        let changes = (request.to_owned_message().changes.into_iter().enumerate())
            .map(|(index, change)| offline_change(change, &format!("changes[{index}]")))
            .collect::<Result<_, _>>()?;
        (seen_by(&self.browse, user).await?)
            .take_offline(user, changes)
            .await
            .map_err(refused(Fields::NONE))?;
        Response::ok(ReportOfflineWatchStateResponse::default())
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
        let browse = seen_by(&self.browse, user).await?;
        let marked = browse.mark(user, library, target, played).await;
        marked.map_err(refused(fields))?;
        Response::ok(SetPlayedResponse::default())
    }

    async fn set_hidden_from_continue_watching(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, SetHiddenFromContinueWatchingRequest>,
    ) -> ServiceResult<SetHiddenFromContinueWatchingResponse> {
        let user = caller(&ctx)?.user.id;
        let SetHiddenFromContinueWatchingRequest {
            library_id: library,
            target,
            hidden,
            ..
        } = request.to_owned_message();
        let target = target.ok_or_else(|| {
            refuse(
                RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
                "say which Movie or Show to hide",
            )
            .field("target")
        })?;
        let (target, fields) = match target {
            Hide::MovieId(id) => movie_target(*id)?,
            Hide::ShowId(id) => show_target(*id)?,
        };
        let browse = seen_by(&self.browse, user).await?;
        let hid = browse.hide(user, library, target, hidden).await;
        hid.map_err(refused(fields))?;
        Response::ok(SetHiddenFromContinueWatchingResponse::default())
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

/// The refusal of a duration `Progress` can't hold, in the request's `field`.
fn duration_out_of_range(field: &str) -> ConnectError {
    refuse(
        RefusalReason::REFUSAL_REASON_DURATION_OUT_OF_RANGE,
        "the duration must be more than zero and less than 2^63 milliseconds",
    )
    .field(field)
    .param("minDurationMs", &1)
    .param("maxDurationMs", &i64::MAX)
    .into()
}

/// A request's offline `change`, at `field`, as the change Browse looks up by its id.
fn offline_change(
    change: OfflineWatchStateChange,
    field: &str,
) -> Result<WatchStateChange, ConnectError> {
    let (episode, id) = match change.title {
        Some(Title::MovieId(id)) => (
            false,
            required_id(Some(*id), &format!("{field}.movie_id"), "Movie")?,
        ),
        Some(Title::EpisodeId(id)) => (
            true,
            required_id(Some(*id), &format!("{field}.episode_id"), "Episode")?,
        ),
        None => {
            return Err(refuse(
                RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
                "say which Movie or Episode changed",
            )
            .field(&format!("{field}.title"))
            .into());
        }
    };
    // A 0 of 0 change would mark it unplayed, which offline progress never does.
    if Progress::new(change.position_ms, change.duration_ms).is_none() {
        return Err(duration_out_of_range(&format!("{field}.duration_ms")));
    }
    Ok(WatchStateChange {
        episode,
        ids: vec![id.to_string()],
        played: change.played,
        position_ms: change.position_ms,
        duration_ms: change.duration_ms,
        changed_at_ms: change.changed_at_ms,
    })
}
