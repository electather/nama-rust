//! Each signed-in User's progress through what they play: `nama.v1.WatchStateService`.

use connectrpc::{RequestContext, Response, ServiceRequest, ServiceResult};
use nama_plugin_api::metadata::ExternalId;
use nama_proto::nama::v1::{
    RefusalReason, ReportProgressRequest, ReportProgressResponse, SetPlayedRequest,
    SetPlayedResponse, WatchStateService, set_played_request::Target,
};

use crate::{
    browse::Browse,
    contents::{Contents, MatchedTo},
    watch_states::{Progress, WatchStates},
};

use super::{caller, database_failed, refuse, required_id, version_not_found};

/// `nama.v1.WatchStateService`: each signed-in User's progress through what they play.
pub(super) struct WatchStateServiceHandler {
    pub(super) contents: Contents,
    pub(super) watch_states: WatchStates,
    /// Finds what `SetPlayed` marks as browsing does, so it fails as browsing does.
    pub(super) browse: Browse,
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
        let matched_to = self.contents.matched_to(request.version_id).await;
        let reported = match matched_to.map_err(database_failed)? {
            MatchedTo::NoSuchVersion => return Err(version_not_found()),
            MatchedTo::Nothing => Ok(()),
            MatchedTo::Movie(movie) => self.watch_states.report_movie(user, &movie, progress).await,
            MatchedTo::Episodes(episodes) => {
                self.watch_states
                    .report_episodes(user, &episodes, progress)
                    .await
            }
        };
        reported.map_err(database_failed)?;
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
        let browse = &self.browse;
        let watch_states = &self.watch_states;
        let marked = match target {
            Target::MovieId(id) => {
                let id = required_id(Some(*id), "movie_id", "Movie")?;
                let (movie, _watch_state) = browse.movie(user, library, &id).await?;
                watch_states.mark_movie(user, &movie.id, played).await
            }
            Target::EpisodeId(id) => {
                let id = required_id(Some(*id), "episode_id", "Episode")?;
                let episode = browse.episode(library, &id).await?;
                watch_states
                    .mark_episodes(user, &[&episode.id], played)
                    .await
            }
            Target::Season(season) => {
                let show = required_id(season.show_id.into_option(), "season.show_id", "Show")?;
                let season = browse
                    .season(library, &show, i64::from(season.season_number))
                    .await?;
                let ids: Vec<&ExternalId> =
                    season.episodes.iter().map(|episode| &episode.id).collect();
                watch_states.mark_episodes(user, &ids, played).await
            }
            Target::ShowId(id) => {
                let id = required_id(Some(*id), "show_id", "Show")?;
                let show = browse.show(library, &id).await?;
                let ids: Vec<&ExternalId> = show.episodes().map(|episode| &episode.id).collect();
                watch_states.mark_episodes(user, &ids, played).await
            }
        };
        marked.map_err(database_failed)?;
        Response::ok(SetPlayedResponse::default())
    }
}
