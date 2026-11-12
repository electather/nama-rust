//! Users asking for Movies and Seasons of Shows: `nama.v1.RequestService`.

use std::collections::BTreeSet;

use connectrpc::{ConnectError, RequestContext, Response, ServiceRequest, ServiceResult};
use nama_plugin_api::{
    PluginId,
    fulfiller::{Movies, Shows},
    metadata::{Candidate, ExternalId},
};
use nama_proto::nama::v1::{
    self as proto, ApproveFollowedShowRequest, ApproveFollowedShowResponse, ApproveRequestRequest,
    ApproveRequestResponse, CancelFollowedShowRequest, CancelFollowedShowResponse,
    CancelRequestRequest, CancelRequestResponse, CountPendingRequestsRequest,
    CountPendingRequestsResponse, CreateFollowedShowRequest, CreateFollowedShowResponse,
    CreateRequestRequest, CreateRequestResponse, CreateSeasonRequestsRequest,
    CreateSeasonRequestsResponse, DeclineFollowedShowRequest, DeclineFollowedShowResponse,
    DeclineRequestRequest, DeclineRequestResponse, GetRequestRequest, GetRequestResponse,
    ListRequestsRequest, ListRequestsResponse, LookUpMovieRequest, LookUpMovieResponse,
    LookUpShowRequest, LookUpShowResponse, RefusalReason, RequestService, RequestState,
    RetryRequestRequest, RetryRequestResponse, SearchMoviesRequest, SearchMoviesResponse,
    SearchShowsRequest, SearchShowsResponse, WithdrawFollowedShowRequest,
    WithdrawFollowedShowResponse, WithdrawRequestRequest, WithdrawRequestResponse,
};

use crate::{
    artwork::Artwork,
    contents::Metadata,
    custom_images::{CustomImages, Role, Slot, Urls},
    qualities,
    requests::{self, Asked, Found, FoundShow, Hit, Requests},
};

use super::{Refused, caller, custom_image_urls, database_failed, refuse, title_kind_from_proto};

/// `nama.v1.RequestService`: Users asking for Movies and Seasons of Shows. Asking needs
/// `PERMISSION_REQUEST_MOVIE` or `PERMISSION_REQUEST_TV`, which
/// [`require_access`](super::require_access) enforces; without `PERMISSION_VIEW_REQUESTS`,
/// [`Requests`] shows a User only their own Requests.
pub(super) struct RequestServiceHandler {
    pub(super) requests: Requests,
    /// Serves the images of the Movies found, which no Match stores.
    pub(super) artwork: Artwork,
    /// Shown in place of the images found, for titles an admin gave one.
    pub(super) custom_images: CustomImages,
}

#[expect(
    refining_impl_trait,
    reason = "plain `async fn` returning the owned response is the form the generated trait \
              documents; the refined signature isn't visible outside this crate"
)]
impl RequestService for RequestServiceHandler {
    async fn create_request(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, CreateRequestRequest>,
    ) -> ServiceResult<CreateRequestResponse> {
        let asker = &caller(&ctx)?.user;
        let request = self
            .requests
            .create(asker, request.movie_tmdb_id, request.quality)
            .await?;
        Response::ok(CreateRequestResponse {
            request: request.into(),
            ..Default::default()
        })
    }

    async fn create_season_requests(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, CreateSeasonRequestsRequest>,
    ) -> ServiceResult<CreateSeasonRequestsResponse> {
        let asker = &caller(&ctx)?.user;
        let seasons: BTreeSet<u32> = request.season_numbers.iter().copied().collect();
        if seasons.is_empty() {
            return Err(refuse(
                RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
                "name at least one Season to ask for",
            )
            .field("season_numbers")
            .into());
        }
        let requests = self
            .requests
            .create_seasons(asker, request.show_tmdb_id, &seasons, request.quality)
            .await?;
        Response::ok(CreateSeasonRequestsResponse {
            requests,
            ..Default::default()
        })
    }

    async fn list_requests(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, ListRequestsRequest>,
    ) -> ServiceResult<ListRequestsResponse> {
        let viewer = &caller(&ctx)?.user;
        // Unspecified, or a state this Server doesn't know, lists every state.
        let state = request
            .state
            .as_known()
            .filter(|&state| state != RequestState::Unspecified);
        let size = match request.page_size {
            0 => 50,
            size => size.min(100),
        };
        let page = self
            .requests
            .list(viewer, state, size as usize, request.page_token)
            .await?;
        Response::ok(page)
    }

    async fn get_request(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, GetRequestRequest>,
    ) -> ServiceResult<GetRequestResponse> {
        let viewer = &caller(&ctx)?.user;
        let request = self.requests.get(viewer, request.id).await?;
        Response::ok(GetRequestResponse {
            request: request.into(),
            ..Default::default()
        })
    }

    async fn count_pending_requests(
        &self,
        ctx: RequestContext,
        _request: ServiceRequest<'_, CountPendingRequestsRequest>,
    ) -> ServiceResult<CountPendingRequestsResponse> {
        let viewer = &caller(&ctx)?.user;
        let count = self
            .requests
            .count_pending(viewer)
            .await
            .map_err(database_failed)?;
        Response::ok(CountPendingRequestsResponse {
            count,
            ..Default::default()
        })
    }

    async fn search_movies(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, SearchMoviesRequest>,
    ) -> ServiceResult<SearchMoviesResponse> {
        let viewer = &caller(&ctx)?.user;
        let title = searched_title(request.to_owned_message().title)?;
        let found = self.requests.search(viewer, title).await?;
        let custom = custom_image_urls(&self.custom_images).await?;
        Response::ok(SearchMoviesResponse {
            movies: found
                .into_iter()
                .map(|found| self.searched_to_proto(found, &custom))
                .collect(),
            ..Default::default()
        })
    }

    async fn list_movie_qualities(
        &self,
        ctx: RequestContext,
        _request: ServiceRequest<'_, proto::ListMovieQualitiesRequest>,
    ) -> ServiceResult<proto::ListMovieQualitiesResponse> {
        let (qualities, personal_default) = self
            .requests
            .qualities::<Movies>(&caller(&ctx)?.user)
            .await
            .map_err(database_failed)?;
        Response::ok(proto::ListMovieQualitiesResponse {
            qualities,
            personal_default,
            ..Default::default()
        })
    }

    async fn list_default_qualities(
        &self,
        ctx: RequestContext,
        _request: ServiceRequest<'_, proto::ListDefaultQualitiesRequest>,
    ) -> ServiceResult<proto::ListDefaultQualitiesResponse> {
        let defaults = self
            .requests
            .personal_defaults(caller(&ctx)?.user.id)
            .await
            .map_err(database_failed)?;
        Response::ok(proto::ListDefaultQualitiesResponse {
            defaults,
            ..Default::default()
        })
    }

    async fn list_show_qualities(
        &self,
        ctx: RequestContext,
        _request: ServiceRequest<'_, proto::ListShowQualitiesRequest>,
    ) -> ServiceResult<proto::ListShowQualitiesResponse> {
        let (qualities, personal_default) = self
            .requests
            .qualities::<Shows>(&caller(&ctx)?.user)
            .await
            .map_err(database_failed)?;
        Response::ok(proto::ListShowQualitiesResponse {
            qualities,
            personal_default,
            ..Default::default()
        })
    }

    async fn set_default_quality(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, proto::SetDefaultQualityRequest>,
    ) -> ServiceResult<proto::SetDefaultQualityResponse> {
        let plugin = PluginId::from(request.plugin_id.to_owned());
        self.requests
            .set_personal_default(caller(&ctx)?.user.id, plugin, request.quality)
            .await?;
        Response::ok(proto::SetDefaultQualityResponse::default())
    }

    async fn look_up_movie(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, LookUpMovieRequest>,
    ) -> ServiceResult<LookUpMovieResponse> {
        let viewer = &caller(&ctx)?.user;
        let found = self.requests.look_up(viewer, request.movie_tmdb_id).await?;
        let custom = custom_image_urls(&self.custom_images).await?;
        Response::ok(LookUpMovieResponse {
            movie: self.looked_up_to_proto(found, &custom).into(),
            ..Default::default()
        })
    }

    async fn search_shows(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, SearchShowsRequest>,
    ) -> ServiceResult<SearchShowsResponse> {
        let title = searched_title(request.to_owned_message().title)?;
        let found = self.requests.search_shows(title).await?;
        let custom = custom_image_urls(&self.custom_images).await?;
        Response::ok(SearchShowsResponse {
            shows: found
                .into_iter()
                .map(|found| self.searched_show_to_proto(found, &custom))
                .collect(),
            ..Default::default()
        })
    }

    async fn look_up_show(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, LookUpShowRequest>,
    ) -> ServiceResult<LookUpShowResponse> {
        let viewer = &caller(&ctx)?.user;
        let found = self
            .requests
            .look_up_show(viewer, request.show_tmdb_id)
            .await?;
        let custom = custom_image_urls(&self.custom_images).await?;
        Response::ok(LookUpShowResponse {
            show: self.looked_up_show_to_proto(found, &custom).into(),
            ..Default::default()
        })
    }

    async fn approve_request(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, ApproveRequestRequest>,
    ) -> ServiceResult<ApproveRequestResponse> {
        let manager = &caller(&ctx)?.user;
        let approved = self
            .requests
            .approve(manager, request.id, request.quality_id)
            .await?;
        Response::ok(ApproveRequestResponse {
            request: approved.into(),
            ..Default::default()
        })
    }

    async fn decline_request(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, DeclineRequestRequest>,
    ) -> ServiceResult<DeclineRequestResponse> {
        let declined = self.requests.decline(request.id, request.reason).await?;
        Response::ok(DeclineRequestResponse {
            request: declined.into(),
            ..Default::default()
        })
    }

    async fn cancel_request(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, CancelRequestRequest>,
    ) -> ServiceResult<CancelRequestResponse> {
        self.requests.cancel(request.id).await?;
        Response::ok(CancelRequestResponse::default())
    }

    async fn retry_request(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, RetryRequestRequest>,
    ) -> ServiceResult<RetryRequestResponse> {
        let manager = &caller(&ctx)?.user;
        let retried = self
            .requests
            .retry(manager, request.id, request.quality_id)
            .await?;
        Response::ok(RetryRequestResponse {
            request: retried.into(),
            ..Default::default()
        })
    }

    async fn withdraw_request(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, WithdrawRequestRequest>,
    ) -> ServiceResult<WithdrawRequestResponse> {
        let requester = &caller(&ctx)?.user;
        self.requests.withdraw(requester, request.id).await?;
        Response::ok(WithdrawRequestResponse::default())
    }

    async fn create_followed_show(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, CreateFollowedShowRequest>,
    ) -> ServiceResult<CreateFollowedShowResponse> {
        let asker = &caller(&ctx)?.user;
        let (followed_show, requests) = self
            .requests
            .follow(asker, request.show_tmdb_id, request.quality)
            .await?;
        Response::ok(CreateFollowedShowResponse {
            followed_show: followed_show.into(),
            requests,
            ..Default::default()
        })
    }

    async fn list_offered_qualities(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, proto::ListOfferedQualitiesRequest>,
    ) -> ServiceResult<proto::ListOfferedQualitiesResponse> {
        let kind = title_kind_from_proto(request.kind)?;
        let qualities = self.requests.offered_qualities(kind).await?;
        Response::ok(proto::ListOfferedQualitiesResponse {
            qualities,
            ..Default::default()
        })
    }

    async fn approve_followed_show(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, ApproveFollowedShowRequest>,
    ) -> ServiceResult<ApproveFollowedShowResponse> {
        let manager = &caller(&ctx)?.user;
        let approved = self
            .requests
            .approve_followed(manager, request.id, request.quality_id)
            .await?;
        Response::ok(ApproveFollowedShowResponse {
            followed_show: approved.into(),
            ..Default::default()
        })
    }

    async fn decline_followed_show(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, DeclineFollowedShowRequest>,
    ) -> ServiceResult<DeclineFollowedShowResponse> {
        let declined = self
            .requests
            .decline_followed(request.id, request.reason)
            .await?;
        Response::ok(DeclineFollowedShowResponse {
            followed_show: declined.into(),
            ..Default::default()
        })
    }

    async fn cancel_followed_show(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, CancelFollowedShowRequest>,
    ) -> ServiceResult<CancelFollowedShowResponse> {
        self.requests.cancel_followed(request.id).await?;
        Response::ok(CancelFollowedShowResponse::default())
    }

    async fn withdraw_followed_show(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, WithdrawFollowedShowRequest>,
    ) -> ServiceResult<WithdrawFollowedShowResponse> {
        let requester = &caller(&ctx)?.user;
        self.requests
            .withdraw_followed(requester, request.id)
            .await?;
        Response::ok(WithdrawFollowedShowResponse::default())
    }
}

impl RequestServiceHandler {
    /// The URL of the image a title shows in `slot`: its Custom Image in `custom`, or else the
    /// one at `path` from `plugin`'s Provider, offered to clients.
    fn image_url(
        &self,
        custom: &Urls,
        slot: &Slot,
        plugin: &PluginId,
        path: Option<String>,
    ) -> Option<String> {
        (custom.of(slot)).or_else(|| path.map(|path| self.artwork.offer(plugin, &path)))
    }

    /// A Movie a search found, as clients see it, with its poster offered to them.
    fn searched_to_proto(&self, found: Found<Candidate>, custom: &Urls) -> proto::MovieHit {
        let Found {
            tmdb_id,
            plugin,
            movie,
            library,
            request,
        } = found;
        proto::MovieHit {
            tmdb_id,
            title: movie.title,
            year: movie.year,
            poster_url: self.image_url(
                custom,
                &movie_slot(tmdb_id, Role::Poster),
                &plugin,
                movie.poster,
            ),
            library_id: library,
            request: request.into(),
            ..Default::default()
        }
    }

    /// A Movie looked up, as clients see it, with its images offered to them.
    fn looked_up_to_proto(&self, found: Found<Metadata>, custom: &Urls) -> proto::FoundMovie {
        let Found {
            tmdb_id,
            plugin,
            movie,
            library,
            request,
        } = found;
        let image_url =
            |role, path| self.image_url(custom, &movie_slot(tmdb_id, role), &plugin, path);
        proto::FoundMovie {
            tmdb_id,
            title: movie.title,
            year: movie.release_date.as_deref().and_then(year_of),
            overview: movie.overview,
            runtime_minutes: movie.runtime_minutes,
            poster_url: image_url(Role::Poster, movie.poster),
            backdrop_url: image_url(Role::Backdrop, movie.backdrop),
            library_id: library,
            request: request.into(),
            ..Default::default()
        }
    }

    /// A Show a search found, as clients see it, with its poster offered to them.
    fn searched_show_to_proto(&self, hit: Hit, custom: &Urls) -> proto::ShowHit {
        let Hit {
            tmdb_id,
            plugin,
            candidate: show,
        } = hit;
        proto::ShowHit {
            tmdb_id,
            title: show.title,
            year: show.year,
            poster_url: self.image_url(
                custom,
                &show_slot(tmdb_id, Role::Poster),
                &plugin,
                show.poster,
            ),
            ..Default::default()
        }
    }

    /// A Show looked up, as clients see it, with its images offered to them.
    fn looked_up_show_to_proto(&self, found: FoundShow, custom: &Urls) -> proto::FoundShow {
        let FoundShow {
            tmdb_id,
            plugin,
            show,
            seasons,
            followed_show,
            quality,
        } = found;
        let image_url =
            |role, path| self.image_url(custom, &show_slot(tmdb_id, role), &plugin, path);
        proto::FoundShow {
            tmdb_id,
            title: show.title,
            year: show.first_air_date.as_deref().and_then(year_of),
            overview: show.overview,
            poster_url: image_url(Role::Poster, show.poster),
            backdrop_url: image_url(Role::Backdrop, show.backdrop),
            seasons,
            followed_show: followed_show.into(),
            quality: quality.into(),
            ..Default::default()
        }
    }
}

/// The slot of the Movie whose TMDB id is `tmdb_id`, as browsing identifies it.
fn movie_slot(tmdb_id: i64, role: Role) -> Slot {
    Slot::movie(&ExternalId::Tmdb(tmdb_id), role)
}

/// The slot of the Show whose TMDB id is `tmdb_id`, as browsing identifies it.
fn show_slot(tmdb_id: i64, role: Role) -> Slot {
    Slot::show(&ExternalId::Tmdb(tmdb_id), role)
}

/// `title`, unless it is blank, which a search for titles refuses.
fn searched_title(title: String) -> Result<String, ConnectError> {
    if title.trim().is_empty() {
        return Err(refuse(
            RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
            "the title to search for must not be blank",
        )
        .field("title")
        .into());
    }
    Ok(title)
}

/// The year of a `YYYY-MM-DD` date.
fn year_of(date: &str) -> Option<i32> {
    date.get(..4)?.parse().ok()
}

/// The refusal of a Quality a User named, in the field they named it in.
impl From<qualities::Refused> for ConnectError {
    fn from(err: qualities::Refused) -> Self {
        use RefusalReason as R;
        let message = err.to_string();
        let refused = match err {
            qualities::Refused::NotAllowed => {
                refuse(R::REFUSAL_REASON_QUALITY_NOT_ALLOWED, message).field(qualities::ASKED)
            }
            qualities::Refused::NotOffered { id, field } => {
                refuse(R::REFUSAL_REASON_QUALITY_NOT_OFFERED, message)
                    .field(field)
                    .param("qualityId", &id)
            }
            qualities::Refused::NotTheShows { named, shows } => {
                show_has_quality(message, qualities::ASKED, named, &shows)
            }
            qualities::Refused::ShowQualityFixed { named, shows } => {
                show_has_quality(message, qualities::CHANGED, named, &shows)
            }
        };
        refused.into()
    }
}

/// The refusal of the Quality `named` in `field`, as it isn't the Show's, `shows`.
fn show_has_quality(
    message: String,
    field: &str,
    named: i64,
    shows: &proto::RequestedQuality,
) -> Refused {
    refuse(RefusalReason::REFUSAL_REASON_SHOW_HAS_QUALITY, message)
        .field(field)
        .param("qualityId", &named)
        .param("showQualityId", &shows.id)
        .param("showQualityName", &shows.name)
}

/// The Connect error a client gets for a Request that couldn't be made or found. Database failures
/// are logged, not sent.
impl From<requests::Error> for ConnectError {
    fn from(err: requests::Error) -> Self {
        use RefusalReason as R;
        let message = err.to_string();
        let refused = match err {
            // Every RPC naming a Request or Followed Show names it in `id`, only `CreateRequest`
            // and `LookUpMovie` name a Movie, both in `movie_tmdb_id`, and only
            // `CreateSeasonRequests`, `CreateFollowedShow` and `LookUpShow` a Show, all in
            // `show_tmdb_id`.
            requests::Error::NoSuch(Asked::Request, id) => {
                refuse(R::REFUSAL_REASON_REQUEST_NOT_FOUND, message)
                    .field("id")
                    .param("requestId", &id)
            }
            requests::Error::NotPending(Asked::Request, id) => {
                refuse(R::REFUSAL_REASON_REQUEST_NOT_PENDING, message)
                    .field("id")
                    .param("requestId", &id)
            }
            requests::Error::NotFailed(id) => refuse(R::REFUSAL_REASON_REQUEST_NOT_FAILED, message)
                .field("id")
                .param("requestId", &id),
            requests::Error::NotARequester(Asked::Request, id) => {
                refuse(R::REFUSAL_REASON_NOT_A_REQUESTER, message)
                    .field("id")
                    .param("requestId", &id)
            }
            requests::Error::NoSuch(Asked::FollowedShow, id) => {
                refuse(R::REFUSAL_REASON_FOLLOWED_SHOW_NOT_FOUND, message)
                    .field("id")
                    .param("followedShowId", &id)
            }
            requests::Error::NotPending(Asked::FollowedShow, id) => {
                refuse(R::REFUSAL_REASON_FOLLOWED_SHOW_NOT_PENDING, message)
                    .field("id")
                    .param("followedShowId", &id)
            }
            requests::Error::NotARequester(Asked::FollowedShow, id) => {
                refuse(R::REFUSAL_REASON_NOT_A_FOLLOWED_SHOW_REQUESTER, message)
                    .field("id")
                    .param("followedShowId", &id)
            }
            requests::Error::InLibrary(id) => refuse(R::REFUSAL_REASON_ALREADY_IN_LIBRARY, message)
                .field("movie_tmdb_id")
                .param("tmdbId", &id),
            requests::Error::SeasonInLibrary { show, season } => {
                refuse(R::REFUSAL_REASON_ALREADY_IN_LIBRARY, message)
                    .field("season_numbers")
                    .param("tmdbId", &show)
                    .param("seasonNumber", &season)
            }
            requests::Error::NoSuchMovie(id) => refuse(R::REFUSAL_REASON_UNKNOWN_TITLE, message)
                .field("movie_tmdb_id")
                .param("titleId", &ExternalId::Tmdb(id)),
            requests::Error::NoSuchShow(id) => refuse(R::REFUSAL_REASON_UNKNOWN_TITLE, message)
                .field("show_tmdb_id")
                .param("titleId", &ExternalId::Tmdb(id)),
            requests::Error::NoSuchSeason { show, season } => {
                refuse(R::REFUSAL_REASON_UNKNOWN_TITLE, message)
                    .field("season_numbers")
                    .param("titleId", &ExternalId::Tmdb(show))
                    .param("seasonNumber", &season)
            }
            requests::Error::Unavailable => refuse(R::REFUSAL_REASON_NO_METADATA_PROVIDER, message),
            requests::Error::Quality(refused) => return refused.into(),
            requests::Error::PageTokenInvalid => {
                refuse(R::REFUSAL_REASON_PAGE_TOKEN_INVALID, message).field("page_token")
            }
            requests::Error::Database(err) => return database_failed(err),
        };
        refused.into()
    }
}
