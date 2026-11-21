//! Users asking for Movies and Seasons of Shows: `nama.v1.RequestService`.

use std::collections::BTreeSet;

use connectrpc::{ConnectError, RequestContext, Response, ServiceRequest, ServiceResult};
use nama_plugin_api::{
    PluginId,
    fulfiller::{Movies, Shows},
    metadata::{ExternalId, ExternalIds},
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
    RetryRequestRequest, RetryRequestResponse, WithdrawFollowedShowRequest,
    WithdrawFollowedShowResponse, WithdrawRequestRequest, WithdrawRequestResponse,
};

use crate::{
    library::{
        Candidate, Downloaded, ImageRef, ImageSources, Metadata, Role, ShownImages, TitleImage,
        Trailer,
    },
    requests::{
        self, Asked, Found, FoundShow, Hit, Listed, QUALITY_ASKED_FIELD, QUALITY_CHANGED_FIELD,
        QualityRefused, Requests, year_of,
    },
};

use super::{
    Refused, caller, database_failed, external_id_to_proto, refuse, required_id,
    title_kind_from_proto,
};

/// `nama.v1.RequestService`: Users asking for Movies and Seasons of Shows. Asking needs
/// `PERMISSION_REQUEST_MOVIE` or `PERMISSION_REQUEST_TV`, which
/// [`require_access`](super::require_access) enforces; without `PERMISSION_VIEW_REQUESTS`,
/// [`Requests`] shows a User only their own Requests.
pub(super) struct RequestServiceHandler {
    pub(super) requests: Requests,
    /// The images found, or those the Server shows in their place, with their thumbhashes.
    pub(super) images: ImageSources,
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
        let request = request.to_owned_message();
        let request = self
            .requests
            .create(
                asker,
                &required_id(request.movie_id.into_option(), "movie_id", "Movie")?,
                request.quality,
            )
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
        let request = request.to_owned_message();
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
            .create_seasons(
                asker,
                &required_id(request.show_id.into_option(), "show_id", "Show")?,
                &seasons,
                request.quality,
            )
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
            .filter
            .state
            .as_known()
            .filter(|&state| state != RequestState::Unspecified);
        let page = self
            .requests
            .list(viewer, state, &request.to_owned_message().page)
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
        let id = required_id(
            request.to_owned_message().movie_id.into_option(),
            "movie_id",
            "Movie",
        )?;
        let not_found = || title_not_found(&id, "movie_id", "Movie");
        let found = self.requests.look_up(viewer, &id).await;
        let found = found.map_err(|err| {
            if matches!(err, requests::Error::NoSuchMovie(_)) {
                not_found()
            } else {
                err.into()
            }
        })?;
        let images = self.images.shown().await.map_err(database_failed)?;
        Response::ok(LookUpMovieResponse {
            movie: looked_up_to_proto(found, &images).into(),
            ..Default::default()
        })
    }

    async fn look_up_show(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, LookUpShowRequest>,
    ) -> ServiceResult<LookUpShowResponse> {
        let viewer = &caller(&ctx)?.user;
        let id = required_id(
            request.to_owned_message().show_id.into_option(),
            "show_id",
            "Show",
        )?;
        let not_found = || title_not_found(&id, "show_id", "Show");
        let found = self.requests.look_up_show(viewer, &id).await;
        let found = found.map_err(|err| {
            if matches!(err, requests::Error::NoSuchShow(_)) {
                not_found()
            } else {
                err.into()
            }
        })?;
        let images = self.images.shown().await.map_err(database_failed)?;
        Response::ok(LookUpShowResponse {
            show: looked_up_show_to_proto(found, &images).into(),
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
        let request = request.to_owned_message();
        let (followed_show, requests) = self
            .requests
            .follow(
                asker,
                &required_id(request.show_id.into_option(), "show_id", "Show")?,
                request.quality,
            )
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

/// A Movie a search found, as clients see it, with its poster offered to them.
pub(super) fn searched_to_proto(found: Found<Candidate>, images: &ShownImages) -> proto::MovieHit {
    let Found {
        id,
        plugin,
        movie,
        library,
        request,
    } = found;
    let path = movie.poster.as_deref();
    let downloaded = path.map(|path| Downloaded::Offered {
        plugin: &plugin,
        path,
    });
    let poster = images.of_title(&id, &movie.ids, MOVIE_POSTER, downloaded);
    proto::MovieHit {
        id: external_id_to_proto(&id).into(),
        title: movie.title,
        year: movie.year,
        poster_url: poster.url,
        poster_thumbhash: poster.thumbhash,
        library_id: library,
        request: request.into(),
        ..Default::default()
    }
}

/// A Movie looked up, as clients see it, with its images offered to them, each under the Plugin
/// whose artwork list had it.
fn looked_up_to_proto(found: Found<Metadata>, images: &ShownImages) -> proto::FoundMovie {
    let Found {
        id,
        movie,
        library,
        request,
        ..
    } = found;
    let shown = |role, image: Option<&ImageRef>| {
        let downloaded = image.map(Downloaded::offered);
        images.of_title(&id, &movie.ids, TitleImage::Movie(role), downloaded)
    };
    let poster = shown(Role::Poster, movie.poster.as_ref());
    let backdrop = shown(Role::Backdrop, movie.backdrop.as_ref());
    proto::FoundMovie {
        id: external_id_to_proto(&id).into(),
        title: movie.title,
        year: movie.release_date.as_deref().and_then(year_of),
        overview: movie.overview,
        runtime_minutes: movie.runtime_minutes,
        score: movie.score,
        original_language: movie.original_language,
        trailers: trailers_to_proto(movie.trailers),
        poster_url: poster.url,
        poster_thumbhash: poster.thumbhash,
        backdrop_url: backdrop.url,
        backdrop_thumbhash: backdrop.thumbhash,
        library_id: library,
        request: request.into(),
        ..Default::default()
    }
}

/// A Show a search found, as clients see it, with its poster offered to them.
pub(super) fn searched_show_to_proto(hit: Hit, images: &ShownImages) -> proto::ShowHit {
    let Hit {
        id,
        plugin,
        candidate: show,
    } = hit;
    let path = show.poster.as_deref();
    let downloaded = path.map(|path| Downloaded::Offered {
        plugin: &plugin,
        path,
    });
    let poster = images.of_title(&id, &show.ids, SHOW_POSTER, downloaded);
    proto::ShowHit {
        id: external_id_to_proto(&id).into(),
        title: show.title,
        year: show.year,
        poster_url: poster.url,
        poster_thumbhash: poster.thumbhash,
        ..Default::default()
    }
}

/// A Show looked up, as clients see it, with its images offered to them.
fn looked_up_show_to_proto(found: FoundShow, images: &ShownImages) -> proto::FoundShow {
    let FoundShow {
        id,
        plugin,
        show,
        seasons,
        followed_show,
        quality,
    } = found;
    let shown = |role, path: Option<&str>| {
        let downloaded = path.map(|path| Downloaded::Offered {
            plugin: &plugin,
            path,
        });
        images.of_title(&id, &show.ids, TitleImage::Show(role), downloaded)
    };
    let poster = shown(Role::Poster, show.poster.as_deref());
    let backdrop = shown(Role::Backdrop, show.backdrop.as_deref());
    proto::FoundShow {
        id: external_id_to_proto(&id).into(),
        title: show.title,
        year: show.first_air_date.as_deref().and_then(year_of),
        overview: show.overview,
        score: show.score,
        original_language: show.original_language,
        trailers: trailers_to_proto(show.trailers),
        poster_url: poster.url,
        poster_thumbhash: poster.thumbhash,
        backdrop_url: backdrop.url,
        backdrop_thumbhash: backdrop.thumbhash,
        seasons,
        followed_show: followed_show.into(),
        quality: quality.into(),
        ..Default::default()
    }
}

/// A title's Trailers, as clients see them.
pub(super) fn trailers_to_proto(trailers: Vec<Trailer>) -> Vec<proto::Trailer> {
    (trailers.into_iter())
        .map(|trailer| proto::Trailer {
            name: trailer.name,
            url: trailer.url,
            ..Default::default()
        })
        .collect()
}

const MOVIE_POSTER: TitleImage = TitleImage::Movie(Role::Poster);
const SHOW_POSTER: TitleImage = TitleImage::Show(Role::Poster);

/// A Movie missing from the Server, as a search for one finds it, with its open `request`. It has
/// only its Request's id, so it shows a Custom Image kept under that one.
pub(super) fn missing_movie_to_proto(
    movie: Listed,
    request: Option<Box<proto::Request>>,
    images: &ShownImages,
) -> proto::MovieHit {
    let downloaded = movie.poster.as_ref().map(Downloaded::offered);
    let poster = images.of_title(&movie.id, &ExternalIds::default(), MOVIE_POSTER, downloaded);
    proto::MovieHit {
        id: external_id_to_proto(&movie.id).into(),
        title: movie.title,
        year: movie.year,
        poster_url: poster.url,
        poster_thumbhash: poster.thumbhash,
        request: request.map(|request| *request).into(),
        ..Default::default()
    }
}

/// A Show missing from the Server, as a search for one finds it.
pub(super) fn missing_show_to_proto(show: Listed, images: &ShownImages) -> proto::ShowHit {
    let downloaded = show.poster.as_ref().map(Downloaded::offered);
    let poster = images.of_title(&show.id, &ExternalIds::default(), SHOW_POSTER, downloaded);
    proto::ShowHit {
        id: external_id_to_proto(&show.id).into(),
        title: show.title,
        year: show.year,
        poster_url: poster.url,
        poster_thumbhash: poster.thumbhash,
        ..Default::default()
    }
}

/// The refusal of a Quality a User named, in the field they named it in.
impl From<QualityRefused> for ConnectError {
    fn from(err: QualityRefused) -> Self {
        use RefusalReason as R;
        let message = err.to_string();
        let refused = match err {
            QualityRefused::NotAllowed => {
                refuse(R::REFUSAL_REASON_QUALITY_NOT_ALLOWED, message).field(QUALITY_ASKED_FIELD)
            }
            QualityRefused::NotOffered { id, field } => {
                refuse(R::REFUSAL_REASON_QUALITY_NOT_OFFERED, message)
                    .field(field)
                    .param("qualityId", &id)
            }
            QualityRefused::NotTheShows { named, shows } => {
                show_has_quality(message, QUALITY_ASKED_FIELD, named, &shows)
            }
            QualityRefused::ShowQualityFixed { named, shows } => {
                show_has_quality(message, QUALITY_CHANGED_FIELD, named, &shows)
            }
        };
        refused.into()
    }
}

/// The refusal of a look-up of the `what` `id`, named in `field`, which no title source has or
/// can look up. Unlike the create calls' `REFUSAL_REASON_UNKNOWN_TITLE`, a look-up names no title
/// to act on, so it is `not_found`.
fn title_not_found(id: &ExternalId, field: &str, what: &str) -> ConnectError {
    refuse(
        RefusalReason::REFUSAL_REASON_TITLE_NOT_FOUND,
        format!("no title source has a {what} with the id {id}"),
    )
    .field(field)
    .param("titleId", id)
    .into()
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
            // Every RPC naming a Request or Followed Show names it in `id`. The create RPCs name
            // a Movie in `movie_id` and a Show in `show_id`, which the refusals below name;
            // `LookUpMovie` and `LookUpShow` answer a title no source has themselves.
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
                .field("movie_id")
                .param("titleId", &id),
            requests::Error::SeasonInLibrary { show, season } => {
                refuse(R::REFUSAL_REASON_ALREADY_IN_LIBRARY, message)
                    .field("season_numbers")
                    .param("titleId", &show)
                    .param("seasonNumber", &season)
            }
            requests::Error::NoSuchMovie(id) => refuse(R::REFUSAL_REASON_UNKNOWN_TITLE, message)
                .field("movie_id")
                .param("titleId", &id),
            requests::Error::NoSuchShow(id) => refuse(R::REFUSAL_REASON_UNKNOWN_TITLE, message)
                .field("show_id")
                .param("titleId", &id),
            requests::Error::NoSuchSeason { show, season } => {
                refuse(R::REFUSAL_REASON_UNKNOWN_TITLE, message)
                    .field("season_numbers")
                    .param("titleId", &show)
                    .param("seasonNumber", &season)
            }
            requests::Error::Unavailable => refuse(R::REFUSAL_REASON_NO_TITLE_SOURCE, message),
            requests::Error::Quality(refused) => return refused.into(),
            requests::Error::PageTokenInvalid(_) => {
                refuse(R::REFUSAL_REASON_PAGE_TOKEN_INVALID, message).field("page.token")
            }
            requests::Error::Database(err) => return database_failed(err),
        };
        refused.into()
    }
}
