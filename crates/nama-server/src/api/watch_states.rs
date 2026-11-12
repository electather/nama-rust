//! Each signed-in User's progress through what they play: `nama.v1.WatchStateService`.

use connectrpc::{RequestContext, Response, ServiceRequest, ServiceResult};
use nama_proto::nama::v1::{
    RefusalReason, ReportProgressRequest, ReportProgressResponse, SetPlayedRequest,
    SetPlayedResponse, WatchStateService, set_played_request::Target,
};

use crate::{browse, watch_states::Progress};

use super::{
    browse::{Fields, refused},
    caller, refuse, required_id,
};

/// `nama.v1.WatchStateService`: each signed-in User's progress through what they play.
pub(super) struct WatchStateServiceHandler {
    pub(super) browse: browse::Browse,
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
        let user = caller(&ctx)?.user.id;
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
            .report(user, request.version_id, progress)
            .await
            .map_err(refused(Fields {
                version: "version_id",
                ..Fields::NONE
            }))?;
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
            Target::MovieId(id) => {
                let fields = Fields {
                    title: "movie_id",
                    ..Fields::LIBRARY
                };
                let id = required_id(Some(*id), fields.title, "Movie")?;
                (browse::Target::Movie(id), fields)
            }
            Target::EpisodeId(id) => {
                let fields = Fields {
                    title: "episode_id",
                    ..Fields::LIBRARY
                };
                let id = required_id(Some(*id), fields.title, "Episode")?;
                (browse::Target::Episode(id), fields)
            }
            Target::Season(season) => {
                let fields = Fields {
                    title: "season.show_id",
                    season: "season.season_number",
                    ..Fields::LIBRARY
                };
                let show = required_id(season.show_id.into_option(), fields.title, "Show")?;
                let number = i64::from(season.season_number);
                (browse::Target::Season { show, number }, fields)
            }
            Target::ShowId(id) => {
                let fields = Fields {
                    title: "show_id",
                    ..Fields::LIBRARY
                };
                let id = required_id(Some(*id), fields.title, "Show")?;
                (browse::Target::Show(id), fields)
            }
        };
        let marked = self.browse.mark(user, library, target, played).await;
        marked.map_err(refused(fields))?;
        Response::ok(SetPlayedResponse::default())
    }
}
