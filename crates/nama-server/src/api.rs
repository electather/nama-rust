//! The Server's ConnectRPC API (ADR 0007), served on axum.

use std::{collections::BTreeSet, future, net::IpAddr, sync::Arc};

use axum::{
    extract::Request,
    http::{StatusCode, header::SET_COOKIE},
    middleware::{self, Next},
    response::IntoResponse,
};
use buffa::MessageName;
use buffa::{EnumValue, Enumeration};
use connectrpc::{
    ConnectError, Encodable, ErrorCode, ErrorDetail, RequestContext, Response, ServiceRequest,
    ServiceResult, ServiceStream,
};
use futures_util::StreamExt;
use nama_plugin_api::{
    PluginId,
    fulfiller::{Movies, Shows},
    metadata::{Candidate, ExternalId, TitleKind},
};
use nama_proto::nama::v1::{
    self as proto, ApproveFollowedShowRequest, ApproveFollowedShowResponse, ApproveRequestRequest,
    ApproveRequestResponse, CancelFollowedShowRequest, CancelFollowedShowResponse,
    CancelRequestRequest, CancelRequestResponse, CountPendingRequestsRequest,
    CountPendingRequestsResponse, CreateFollowedShowRequest, CreateFollowedShowResponse,
    CreateRequestRequest, CreateRequestResponse, CreateSeasonRequestsRequest,
    CreateSeasonRequestsResponse, DeclineFollowedShowRequest, DeclineFollowedShowResponse,
    DeclineRequestRequest, DeclineRequestResponse, EventService, EventType, GetRequestRequest,
    GetRequestResponse, ListRequestsRequest, ListRequestsResponse, LookUpMovieRequest,
    LookUpMovieResponse, LookUpShowRequest, LookUpShowResponse, Permission, Refusal, RefusalReason,
    RequestService, RequestState, RetryRequestRequest, RetryRequestResponse, SearchMoviesRequest,
    SearchMoviesResponse, SearchShowsRequest, SearchShowsResponse, SubscribeRequest,
    SubscribeResponse, WithdrawFollowedShowRequest, WithdrawFollowedShowResponse,
    WithdrawRequestRequest, WithdrawRequestResponse, external_id,
};

use crate::{
    artwork::{self, Artwork},
    browse::Browse,
    config::Config,
    connections::Connections,
    contents::{Contents, Metadata},
    db::{Database, now},
    events::{EventBus, Filter},
    ffmpeg::Ffmpeg,
    forwarded::{RequestSource, identify_source},
    libraries::Libraries,
    matching::Matcher,
    playback::running::RunningPlans,
    plugin_settings::PluginSettings,
    qualities,
    rate_limits::{SignInCodeLimits, SignInLimits},
    requests::{self, Found, FoundShow, Requests},
    scanner::Scanner,
    server_settings,
    sessions::{cleared_session_cookie, identify_caller, in_session_cookie},
    sign_in_codes::SignInCodes,
    tracks::Tracks,
    users::{Caller, SetupCode, Users},
    watch_states::WatchStates,
};

mod browse;
mod libraries;
mod matches;
mod playback;
mod plugins;
mod server;
mod sign_in;
mod users;
mod watch_states;

use browse::BrowseServiceHandler;
use libraries::LibraryServiceHandler;
use matches::MatchServiceHandler;
use playback::PlaybackServiceHandler;
use plugins::PluginServiceHandler;
use server::ServerServiceHandler;
use sign_in::SignInServiceHandler;
use users::UserServiceHandler;
use watch_states::WatchStateServiceHandler;

/// Routes every RPC the Server answers, its artwork and its Versions, each behind the layers
/// that identify the caller and refuse calls they may not make.
#[expect(
    clippy::too_many_arguments,
    reason = "each is a separate handle the services are built from; a struct bundling them would \
              exist only for this call"
)]
pub(crate) fn router(
    config: &Config,
    db: Arc<Database>,
    plugin_settings: PluginSettings,
    scanner: Scanner,
    events: EventBus,
    requests: Requests,
    settings: server_settings::Settings,
    running: RunningPlans,
    setup_code: Option<SetupCode>,
    ffmpeg: Arc<Ffmpeg>,
    subtitles: crate::playback::subtitles::Cache,
) -> axum::Router {
    let plugins = plugin_settings.registry().clone();
    let users = Users::new(Arc::clone(&db), plugins.clone());
    let contents = Contents::new(Arc::clone(&db));
    let artwork = Artwork::new(&config.data_dir, plugins.clone());
    let artwork_router = artwork::router(artwork.clone(), contents.clone());
    let media = crate::playback::router(
        contents,
        running.clone(),
        Tracks::new(Arc::clone(&db)),
        Arc::clone(&ffmpeg),
        subtitles,
    )
    .merge(crate::playback::running::router(running.clone()));
    let browse = Browse::new(Arc::clone(&db));
    let connections = Connections::new(Arc::clone(&db), plugins.clone());
    let playback = settings.playback();
    connectrpc::Router::new()
        .add_service(Arc::new(ServerServiceHandler {
            name: config.name.clone(),
            users: users.clone(),
            settings,
            running: running.clone(),
            ffmpeg,
        }))
        .add_service(Arc::new(SignInServiceHandler {
            users: users.clone(),
            setup_code,
            sign_in_limits: SignInLimits::default(),
            sign_in_codes: SignInCodes::default(),
            sign_in_code_limits: SignInCodeLimits::default(),
        }))
        .add_service(Arc::new(MatchServiceHandler {
            matcher: Matcher::new(Arc::clone(&db), plugins, events.clone()),
            contents: Contents::new(Arc::clone(&db)),
            libraries: Libraries::new(Arc::clone(&db)),
            scanner: scanner.clone(),
        }))
        .add_service(Arc::new(BrowseServiceHandler {
            browse: browse.clone(),
        }))
        .add_service(Arc::new(WatchStateServiceHandler {
            contents: Contents::new(Arc::clone(&db)),
            watch_states: WatchStates::new(Arc::clone(&db)),
            browse,
        }))
        .add_service(Arc::new(PlaybackServiceHandler {
            scanner: scanner.clone(),
            running,
            settings: playback,
        }))
        .add_service(Arc::new(RequestServiceHandler { requests, artwork }))
        .add_service(Arc::new(PluginServiceHandler { plugin_settings }))
        .add_service(Arc::new(LibraryServiceHandler {
            libraries: Libraries::new(db),
            scanner,
        }))
        .add_service(Arc::new(EventServiceHandler {
            events,
            users: users.clone(),
        }))
        .add_service(Arc::new(UserServiceHandler {
            users: users.clone(),
            connections,
        }))
        .into_axum_router()
        .merge(artwork_router)
        .merge(media)
        .layer(middleware::from_fn(require_access))
        .layer(middleware::from_fn_with_state(users, identify_caller))
        .layer(middleware::from_fn_with_state(
            Arc::from(config.trusted_proxies.as_slice()),
            identify_source,
        ))
}

/// Who may call an RPC or fetch media.
#[derive(Clone, Copy)]
enum Access {
    Anyone,
    SignedIn,
    /// A signed-in User whose Permissions pass a check for this one.
    Needs(Permission),
}

/// Managing Libraries and their Matches.
const MANAGE_LIBRARIES: Access = Access::Needs(Permission::ManageLibraries);

/// Managing Users and their Devices.
const MANAGE_USERS: Access = Access::Needs(Permission::ManageUsers);

/// What only an Admin may do.
const ADMIN: Access = Access::Needs(Permission::Admin);

/// Managing the Server's settings, such as its Plugins.
const MANAGE_SETTINGS: Access = Access::Needs(Permission::ManageSettings);

/// Approving, declining and cancelling every User's Requests.
const MANAGE_REQUESTS: Access = Access::Needs(Permission::ManageRequests);

/// Asking for Movies.
const REQUEST_MOVIE: Access = Access::Needs(Permission::RequestMovie);

/// Asking for Seasons of Shows.
const REQUEST_TV: Access = Access::Needs(Permission::RequestTv);

/// Who may call each RPC, by path; one missing here is refused to everyone (see AGENTS.md, "New
/// RPCs start closed"). Media has its own entry, [`MEDIA`].
// One row per RPC reads as a table; rustfmt would split most rows over four lines.
#[rustfmt::skip]
const ACCESS: &[(&str, Access)] = &[
    (proto::SERVER_SERVICE_GET_SERVER_INFO_SPEC.procedure,        Access::Anyone),
    (proto::SERVER_SERVICE_GET_SERVER_SETTINGS_SPEC.procedure,    MANAGE_SETTINGS),
    (proto::SERVER_SERVICE_UPDATE_SERVER_SETTINGS_SPEC.procedure, MANAGE_SETTINGS),
    (proto::SERVER_SERVICE_GET_PLAYBACK_STATUS_SPEC.procedure,    ADMIN),
    (proto::SERVER_SERVICE_GET_PLAYBACK_SETTINGS_SPEC.procedure,  ADMIN),
    (proto::SERVER_SERVICE_UPDATE_PLAYBACK_SETTINGS_SPEC.procedure, ADMIN),
    (proto::SIGN_IN_SERVICE_SETUP_SPEC.procedure,                 Access::Anyone),
    (proto::SIGN_IN_SERVICE_SIGN_IN_SPEC.procedure,               Access::Anyone),
    (proto::SIGN_IN_SERVICE_SIGN_OUT_SPEC.procedure,              Access::SignedIn),
    (proto::SIGN_IN_SERVICE_WHO_AM_I_SPEC.procedure,              Access::SignedIn),
    (proto::SIGN_IN_SERVICE_REAUTHENTICATE_SPEC.procedure,        Access::SignedIn),
    (proto::SIGN_IN_SERVICE_START_SIGN_IN_CODE_SPEC.procedure,    Access::Anyone),
    (proto::SIGN_IN_SERVICE_LOOK_UP_SIGN_IN_CODE_SPEC.procedure,  Access::SignedIn),
    (proto::SIGN_IN_SERVICE_APPROVE_SIGN_IN_CODE_SPEC.procedure,  Access::SignedIn),
    (proto::SIGN_IN_SERVICE_POLL_SIGN_IN_CODE_SPEC.procedure,     Access::Anyone),
    (proto::LIBRARY_SERVICE_CREATE_LIBRARY_SPEC.procedure,        MANAGE_LIBRARIES),
    (proto::LIBRARY_SERVICE_LIST_LIBRARIES_SPEC.procedure,        Access::SignedIn),
    (proto::LIBRARY_SERVICE_GET_LIBRARY_SPEC.procedure,           MANAGE_LIBRARIES),
    (proto::LIBRARY_SERVICE_UPDATE_LIBRARY_SPEC.procedure,        MANAGE_LIBRARIES),
    (proto::LIBRARY_SERVICE_DELETE_LIBRARY_SPEC.procedure,        MANAGE_LIBRARIES),
    (proto::LIBRARY_SERVICE_SCAN_LIBRARY_SPEC.procedure,          MANAGE_LIBRARIES),
    (proto::MATCH_SERVICE_SEARCH_METADATA_SPEC.procedure,         MANAGE_LIBRARIES),
    (proto::MATCH_SERVICE_SET_MATCH_SPEC.procedure,               MANAGE_LIBRARIES),
    (proto::MATCH_SERVICE_CLEAR_MATCH_SPEC.procedure,             MANAGE_LIBRARIES),
    (proto::MATCH_SERVICE_LIST_UNMATCHED_ITEMS_SPEC.procedure,    MANAGE_LIBRARIES),
    (proto::BROWSE_SERVICE_LIST_MOVIES_SPEC.procedure,            Access::SignedIn),
    (proto::BROWSE_SERVICE_GET_MOVIE_SPEC.procedure,              Access::SignedIn),
    (proto::BROWSE_SERVICE_LIST_SHOWS_SPEC.procedure,             Access::SignedIn),
    (proto::BROWSE_SERVICE_GET_SHOW_SPEC.procedure,               Access::SignedIn),
    (proto::BROWSE_SERVICE_LIST_EPISODES_SPEC.procedure,          Access::SignedIn),
    (proto::BROWSE_SERVICE_LIST_CONTINUE_WATCHING_SPEC.procedure, Access::SignedIn),
    (proto::WATCH_STATE_SERVICE_REPORT_PROGRESS_SPEC.procedure,   Access::SignedIn),
    (proto::WATCH_STATE_SERVICE_SET_PLAYED_SPEC.procedure,        Access::SignedIn),
    (proto::PLAYBACK_SERVICE_PLAN_PLAYBACK_SPEC.procedure,        Access::SignedIn),
    (proto::PLAYBACK_SERVICE_STOP_PLAYBACK_SPEC.procedure,        Access::SignedIn),
    (proto::EVENT_SERVICE_SUBSCRIBE_SPEC.procedure,               Access::SignedIn),
    (proto::USER_SERVICE_CREATE_USER_SPEC.procedure,              MANAGE_USERS),
    (proto::USER_SERVICE_LIST_USERS_SPEC.procedure,               MANAGE_USERS),
    (proto::USER_SERVICE_SET_PERMISSIONS_SPEC.procedure,          MANAGE_USERS),
    (proto::USER_SERVICE_DELETE_USER_SPEC.procedure,              MANAGE_USERS),
    (proto::USER_SERVICE_LIST_DEVICES_SPEC.procedure,             Access::SignedIn),
    (proto::USER_SERVICE_REVOKE_DEVICE_SPEC.procedure,            Access::SignedIn),
    (proto::USER_SERVICE_CHANGE_PASSWORD_SPEC.procedure,          Access::SignedIn),
    (proto::USER_SERVICE_RESET_PASSWORD_SPEC.procedure,           MANAGE_USERS),
    (proto::USER_SERVICE_GET_DEFAULT_PERMISSIONS_SPEC.procedure,  MANAGE_USERS),
    (proto::USER_SERVICE_SET_DEFAULT_PERMISSIONS_SPEC.procedure,  MANAGE_USERS),
    (proto::USER_SERVICE_LIST_PLUGIN_CONNECTIONS_SPEC.procedure,  Access::SignedIn),
    (proto::USER_SERVICE_START_PLUGIN_CONNECTION_SPEC.procedure,  Access::SignedIn),
    (proto::USER_SERVICE_DELETE_PLUGIN_CONNECTION_SPEC.procedure, Access::SignedIn),
    (proto::REQUEST_SERVICE_CREATE_REQUEST_SPEC.procedure,        REQUEST_MOVIE),
    (proto::REQUEST_SERVICE_CREATE_SEASON_REQUESTS_SPEC.procedure, REQUEST_TV),
    (proto::REQUEST_SERVICE_LIST_REQUESTS_SPEC.procedure,         Access::SignedIn),
    (proto::REQUEST_SERVICE_GET_REQUEST_SPEC.procedure,           Access::SignedIn),
    (proto::REQUEST_SERVICE_COUNT_PENDING_REQUESTS_SPEC.procedure, MANAGE_REQUESTS),
    (proto::REQUEST_SERVICE_SEARCH_MOVIES_SPEC.procedure,         REQUEST_MOVIE),
    (proto::REQUEST_SERVICE_LOOK_UP_MOVIE_SPEC.procedure,         REQUEST_MOVIE),
    (proto::REQUEST_SERVICE_LIST_MOVIE_QUALITIES_SPEC.procedure,  REQUEST_MOVIE),
    (proto::REQUEST_SERVICE_LIST_DEFAULT_QUALITIES_SPEC.procedure, Access::SignedIn),
    (proto::REQUEST_SERVICE_SET_DEFAULT_QUALITY_SPEC.procedure,   Access::SignedIn),
    (proto::REQUEST_SERVICE_SEARCH_SHOWS_SPEC.procedure,          REQUEST_TV),
    (proto::REQUEST_SERVICE_LIST_SHOW_QUALITIES_SPEC.procedure,   REQUEST_TV),
    (proto::REQUEST_SERVICE_LOOK_UP_SHOW_SPEC.procedure,          REQUEST_TV),
    (proto::REQUEST_SERVICE_APPROVE_REQUEST_SPEC.procedure,       MANAGE_REQUESTS),
    (proto::REQUEST_SERVICE_DECLINE_REQUEST_SPEC.procedure,       MANAGE_REQUESTS),
    (proto::REQUEST_SERVICE_CANCEL_REQUEST_SPEC.procedure,        MANAGE_REQUESTS),
    (proto::REQUEST_SERVICE_RETRY_REQUEST_SPEC.procedure,         MANAGE_REQUESTS),
    (proto::REQUEST_SERVICE_WITHDRAW_REQUEST_SPEC.procedure,      Access::SignedIn),
    (proto::REQUEST_SERVICE_CREATE_FOLLOWED_SHOW_SPEC.procedure,  REQUEST_TV),
    (proto::REQUEST_SERVICE_APPROVE_FOLLOWED_SHOW_SPEC.procedure, MANAGE_REQUESTS),
    (proto::REQUEST_SERVICE_DECLINE_FOLLOWED_SHOW_SPEC.procedure, MANAGE_REQUESTS),
    (proto::REQUEST_SERVICE_CANCEL_FOLLOWED_SHOW_SPEC.procedure,  MANAGE_REQUESTS),
    (proto::REQUEST_SERVICE_WITHDRAW_FOLLOWED_SHOW_SPEC.procedure, Access::SignedIn),
    (proto::REQUEST_SERVICE_LIST_OFFERED_QUALITIES_SPEC.procedure, MANAGE_REQUESTS),
    (proto::PLUGIN_SERVICE_LIST_PLUGINS_SPEC.procedure,           MANAGE_SETTINGS),
    (proto::PLUGIN_SERVICE_GET_PLUGIN_SPEC.procedure,             MANAGE_SETTINGS),
    (proto::PLUGIN_SERVICE_UPDATE_PLUGIN_SETTINGS_SPEC.procedure, MANAGE_SETTINGS),
    (proto::PLUGIN_SERVICE_ENABLE_PLUGIN_SPEC.procedure,          MANAGE_SETTINGS),
    (proto::PLUGIN_SERVICE_DISABLE_PLUGIN_SPEC.procedure,         MANAGE_SETTINGS),
    (proto::PLUGIN_SERVICE_TEST_PLUGIN_SETTINGS_SPEC.procedure,   MANAGE_SETTINGS),
    (proto::PLUGIN_SERVICE_LIST_QUALITIES_SPEC.procedure,         MANAGE_SETTINGS),
    (proto::PLUGIN_SERVICE_REPLACE_QUALITIES_SPEC.procedure,      MANAGE_SETTINGS),
];

/// Who may fetch media: every path under this prefix, which the media routes share.
const MEDIA: (&str, Access) = ("/media/", Access::SignedIn);

impl Access {
    /// Who may request `path`: an RPC's `/<service>/<method>` as [`ACCESS`] lists it, or media
    /// as [`MEDIA`] does. `None` if nobody may.
    fn of(path: &str) -> Option<Self> {
        let (media, media_access) = MEDIA;
        if path.starts_with(media) {
            return Some(media_access);
        }
        ACCESS
            .iter()
            .find(|(procedure, _)| *procedure == path)
            .map(|&(_, access)| access)
    }

    /// Why `caller`, or nobody, may not make a call that needs this access, if they may not.
    fn check(self, caller: Option<&Caller>) -> Result<(), ConnectError> {
        match (self, caller) {
            (Self::Anyone, _) | (Self::SignedIn, Some(_)) => Ok(()),
            (_, None) => Err(sign_in_required()),
            (Self::Needs(needed), Some(caller)) if caller.user.permissions.allow(needed) => Ok(()),
            (Self::Needs(needed), Some(_)) => Err(permission_required(needed)),
        }
    }
}

/// The refusal of a call that needs the Permission `needed`, which the caller lacks.
fn permission_required(needed: Permission) -> ConnectError {
    let name = needed.proto_name();
    refuse(
        RefusalReason::REFUSAL_REASON_PERMISSION_REQUIRED,
        format!("this needs {name}"),
    )
    .param("permission", &name)
    .into()
}

/// The RPCs that change who can sign in as whom, by path, which need Recent Authentication on top
/// of their [`Access`] (ADR 0014, and see AGENTS.md). A list of its own, not an `Access` variant:
/// `SetPermissions` needs a Permission as well, `ChangePassword` only a session.
const NEEDS_RECENT_AUTHENTICATION: [&str; 6] = [
    proto::USER_SERVICE_CHANGE_PASSWORD_SPEC.procedure,
    proto::USER_SERVICE_RESET_PASSWORD_SPEC.procedure,
    proto::USER_SERVICE_SET_PERMISSIONS_SPEC.procedure,
    // It can put ADMIN in what `CreateUser`, which needs none, gives out.
    proto::USER_SERVICE_SET_DEFAULT_PERMISSIONS_SPEC.procedure,
    proto::USER_SERVICE_DELETE_USER_SPEC.procedure,
    proto::SIGN_IN_SERVICE_APPROVE_SIGN_IN_CODE_SPEC.procedure,
];

/// Why `caller` may not request `path` yet, if it needs Recent Authentication their Device lacks.
fn check_recent_authentication(path: &str, caller: Option<&Caller>) -> Result<(), ConnectError> {
    let lacking = caller.is_some_and(|caller| !caller.device.authenticated_recently(now()));
    if lacking && NEEDS_RECENT_AUTHENTICATION.contains(&path) {
        return Err(refuse(
            RefusalReason::REFUSAL_REASON_RECENT_AUTHENTICATION_REQUIRED,
            "sign in again to do this",
        )
        .into());
    }
    Ok(())
}

/// Refuses `request` unless its caller, which [`identify_caller`] found, may make it, before any
/// handler sees it.
async fn require_access(request: Request, next: Next) -> axum::response::Response {
    let caller = request.extensions().get::<Caller>();
    let path = request.uri().path();
    let allowed = Access::of(path)
        .ok_or_else(|| unlisted(path))
        .and_then(|access| access.check(caller))
        .and_then(|()| check_recent_authentication(path, caller));
    match allowed {
        Ok(()) => next.run(request).await,
        Err(refused) => refused
            .into_http_response(request.headers())
            .into_response(),
    }
}

/// The refusal of a path [`ACCESS`] doesn't list, session or not. An RPC that exists but isn't
/// listed is the Server's bug, not the client's, so this is the router's own 404 for a path it
/// doesn't serve, without a Refusal.
fn unlisted(path: &str) -> ConnectError {
    // connectrpc's router words its 404 this way, naming the path without its leading slash.
    let path = path.strip_prefix('/').unwrap_or(path);
    ConnectError::unimplemented(format!("method not found: {path}"))
        .with_http_status(StatusCode::NOT_FOUND)
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

/// `nama.v1.RequestService`: Users asking for Movies and Seasons of Shows. Asking needs
/// `PERMISSION_REQUEST_MOVIE` or `PERMISSION_REQUEST_TV`, which [`require_access`] enforces;
/// without `PERMISSION_VIEW_REQUESTS`, [`Requests`] shows a User only their own Requests.
struct RequestServiceHandler {
    requests: Requests,
    /// Serves the images of the Movies found, which no Match stores.
    artwork: Artwork,
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
        Response::ok(SearchMoviesResponse {
            movies: found
                .into_iter()
                .map(|found| self.searched_to_proto(found))
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
        Response::ok(LookUpMovieResponse {
            movie: self.looked_up_to_proto(found).into(),
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
        Response::ok(SearchShowsResponse {
            shows: found
                .into_iter()
                .map(|found| self.searched_show_to_proto(found))
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
        Response::ok(LookUpShowResponse {
            show: self.looked_up_show_to_proto(found).into(),
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
    /// A Movie a search found, as clients see it, with its poster offered to them.
    fn searched_to_proto(&self, found: Found<Candidate>) -> proto::FoundMovie {
        let Found {
            tmdb_id,
            plugin,
            movie,
            library,
            request,
        } = found;
        proto::FoundMovie {
            tmdb_id,
            title: movie.title,
            year: movie.year,
            poster_url: movie.poster.map(|path| self.artwork.offer(&plugin, &path)),
            library_id: library,
            request: request.into(),
            ..Default::default()
        }
    }

    /// A Movie looked up, as clients see it, with its images offered to them.
    fn looked_up_to_proto(&self, found: Found<Metadata>) -> proto::FoundMovie {
        let Found {
            tmdb_id,
            plugin,
            movie,
            library,
            request,
        } = found;
        let offer = |path: String| self.artwork.offer(&plugin, &path);
        proto::FoundMovie {
            tmdb_id,
            title: movie.title,
            year: movie.release_date.as_deref().and_then(year_of),
            overview: movie.overview,
            runtime_minutes: movie.runtime_minutes,
            poster_url: movie.poster.map(offer),
            backdrop_url: movie.backdrop.map(offer),
            library_id: library,
            request: request.into(),
            ..Default::default()
        }
    }

    /// A Show a search found, as clients see it, with its poster offered to them.
    fn searched_show_to_proto(&self, found: FoundShow<Candidate>) -> proto::FoundShow {
        let FoundShow {
            tmdb_id,
            plugin,
            show,
            ..
        } = found;
        proto::FoundShow {
            tmdb_id,
            title: show.title,
            year: show.year,
            poster_url: show.poster.map(|path| self.artwork.offer(&plugin, &path)),
            ..Default::default()
        }
    }

    /// A Show looked up, as clients see it, with its images offered to them.
    fn looked_up_show_to_proto(
        &self,
        found: FoundShow<nama_plugin_api::metadata::Show>,
    ) -> proto::FoundShow {
        let FoundShow {
            tmdb_id,
            plugin,
            show,
            seasons,
            followed_show,
            quality,
        } = found;
        let offer = |path: String| self.artwork.offer(&plugin, &path);
        proto::FoundShow {
            tmdb_id,
            title: show.title,
            year: show.first_air_date.as_deref().and_then(year_of),
            overview: show.overview,
            poster_url: show.poster.map(offer),
            backdrop_url: show.backdrop.map(offer),
            seasons,
            followed_show: followed_show.into(),
            quality: quality.into(),
            ..Default::default()
        }
    }
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

/// `nama.v1.EventService`: live updates.
//
// TODO(M3): once there is per-Library access, only send a caller events about Libraries it may see.
struct EventServiceHandler {
    events: EventBus,
    /// Whose sign-outs end the streams their Devices opened.
    users: Users,
}

impl EventService for EventServiceHandler {
    fn subscribe(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, SubscribeRequest>,
    ) -> impl Future<
        Output = ServiceResult<ServiceStream<impl Encodable<SubscribeResponse> + Send + use<>>>,
    > + Send {
        let request = request.to_owned_message();
        let subscribed = caller(&ctx).and_then(|caller| {
            let types = request
                .types
                .into_iter()
                .map(event_type_from_proto)
                .collect::<Result<_, _>>()?;
            let filter = Filter {
                types,
                libraries: request.library_ids,
                // ponytail: Permissions as the stream opened, like any request's, so one who loses
                // `VIEW_REQUESTS` sees everyone's Request events until they resubscribe. End
                // streams on a Permission change if that matters.
                requester: requests::visible_to(&caller.user),
                player: crate::playback::visible_to(&caller.user),
            };
            let signed_out = self.users.signed_out(caller.device.id);
            let events = self.events.subscribe(filter).take_until(signed_out);
            let stream: ServiceStream<SubscribeResponse> = Box::pin(events.map(Ok));
            Response::ok(stream)
        });
        future::ready(subscribed)
    }
}

/// Who sent the request `ctx` is for, or `unauthenticated` if it carries no valid session.
fn caller(ctx: &RequestContext) -> Result<&Caller, ConnectError> {
    ctx.extensions()
        .get::<Caller>()
        .ok_or_else(sign_in_required)
}

/// The Connect error for a call that needs a signed-in Device, made without one.
fn sign_in_required() -> ConnectError {
    refuse(
        RefusalReason::REFUSAL_REASON_SIGN_IN_REQUIRED,
        "sign in first",
    )
    .into()
}

/// The response to a request whose own Device was just signed out, carrying `body`. A cookie
/// session has its cookie cleared.
fn signed_out<M>(body: M, ctx: &RequestContext) -> Response<M> {
    let response = Response::new(body);
    if in_session_cookie(ctx.headers()) {
        response.with_header(SET_COOKIE, cleared_session_cookie(ctx.extensions()))
    } else {
        response
    }
}

fn event_type_from_proto(kind: EnumValue<EventType>) -> Result<EventType, ConnectError> {
    match kind {
        EnumValue::Known(EventType::EVENT_TYPE_UNSPECIFIED) | EnumValue::Unknown(_) => Err(
            unknown_enum_value("types", "an event type must be one this Server knows"),
        ),
        EnumValue::Known(kind) => Ok(kind),
    }
}

fn title_kind_from_proto(kind: EnumValue<proto::TitleKind>) -> Result<TitleKind, ConnectError> {
    match kind {
        EnumValue::Known(proto::TitleKind::TITLE_KIND_MOVIE) => Ok(TitleKind::Movie),
        EnumValue::Known(proto::TitleKind::TITLE_KIND_SHOW) => Ok(TitleKind::Show),
        EnumValue::Known(proto::TitleKind::TITLE_KIND_UNSPECIFIED) | EnumValue::Unknown(_) => Err(
            unknown_enum_value("kind", "the kind of title must be movie or show"),
        ),
    }
}

/// The year of a `YYYY-MM-DD` date.
fn year_of(date: &str) -> Option<i32> {
    date.get(..4)?.parse().ok()
}

/// `n` as a proto `int32`, or `None` if it doesn't fit, which no name a scan parses gives.
fn int32(n: i64) -> Option<i32> {
    i32::try_from(n).ok()
}

fn external_id_from_proto(id: external_id::Id) -> ExternalId {
    match id {
        external_id::Id::Tmdb(id) => ExternalId::Tmdb(id),
        external_id::Id::Imdb(id) => ExternalId::Imdb(id),
        external_id::Id::Tvdb(id) => ExternalId::Tvdb(id),
    }
}

/// The id of the title of kind `what` that a request names in `field`, which it must name.
fn required_id(
    id: Option<proto::ExternalId>,
    field: &str,
    what: &str,
) -> Result<ExternalId, ConnectError> {
    id.and_then(|id| id.id)
        .map(external_id_from_proto)
        .ok_or_else(|| {
            refuse(
                RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
                format!("the {what}'s id is missing"),
            )
            .field(field)
            .into()
        })
}

fn external_id_to_proto(id: ExternalId) -> proto::ExternalId {
    let id = match id {
        ExternalId::Tmdb(id) => external_id::Id::Tmdb(id),
        ExternalId::Imdb(id) => external_id::Id::Imdb(id),
        ExternalId::Tvdb(id) => external_id::Id::Tvdb(id),
    };
    proto::ExternalId {
        id: Some(id),
        ..Default::default()
    }
}

/// A refusal of a client's request on its way to becoming a [`ConnectError`]: [`refuse`] starts
/// one, [`Refused::field`] and [`Refused::param`] fill it in.
struct Refused {
    code: ErrorCode,
    refusal: Refusal,
    message: String,
}

/// Refuses a request for breaking the rule `reason` names, telling the client `message`. Every
/// error a client can cause is built here (see AGENTS.md, "Refusals are typed").
fn refuse(reason: RefusalReason, message: impl Into<String>) -> Refused {
    Refused {
        code: refusal_code(reason),
        refusal: Refusal {
            reason: reason.into(),
            ..Default::default()
        },
        message: message.into(),
    }
}

/// The Connect code `reason` always comes with: API contract, as the reasons are (ADR 0019).
const fn refusal_code(reason: RefusalReason) -> ErrorCode {
    use RefusalReason as R;
    match reason {
        // Never sent; a client sees it only for a reason it doesn't know.
        R::REFUSAL_REASON_UNSPECIFIED
        | R::REFUSAL_REASON_FIELD_REQUIRED
        | R::REFUSAL_REASON_UNKNOWN_ENUM_VALUE
        | R::REFUSAL_REASON_WRONG_LIBRARY_KIND
        | R::REFUSAL_REASON_DURATION_OUT_OF_RANGE
        | R::REFUSAL_REASON_UNKNOWN_TITLE
        | R::REFUSAL_REASON_NAME_LENGTH
        | R::REFUSAL_REASON_PASSWORD_REJECTED
        | R::REFUSAL_REASON_FOLDER_NOT_ABSOLUTE
        | R::REFUSAL_REASON_FOLDER_NOT_FOUND
        | R::REFUSAL_REASON_FOLDER_NOT_READABLE
        | R::REFUSAL_REASON_FOLDER_NOT_UTF8
        | R::REFUSAL_REASON_FOLDER_REPEATED
        | R::REFUSAL_REASON_SETTING_WRONG_KIND
        | R::REFUSAL_REASON_NOT_A_URL
        | R::REFUSAL_REASON_QUALITY_REPEATED
        | R::REFUSAL_REASON_DEFAULT_QUALITY_REQUIRED
        | R::REFUSAL_REASON_QUALITY_NOT_OFFERED
        | R::REFUSAL_REASON_PAGE_TOKEN_INVALID
        | R::REFUSAL_REASON_SHOW_HAS_QUALITY => ErrorCode::InvalidArgument,
        R::REFUSAL_REASON_SIGN_IN_REQUIRED
        | R::REFUSAL_REASON_WRONG_NAME_OR_PASSWORD
        | R::REFUSAL_REASON_WRONG_PASSWORD
        | R::REFUSAL_REASON_SIGNED_OUT => ErrorCode::Unauthenticated,
        R::REFUSAL_REASON_PERMISSION_REQUIRED
        | R::REFUSAL_REASON_WRONG_SETUP_CODE
        | R::REFUSAL_REASON_QUALITY_NOT_ALLOWED => ErrorCode::PermissionDenied,
        R::REFUSAL_REASON_TOO_MANY_ATTEMPTS
        | R::REFUSAL_REASON_TOO_MANY_PENDING_SIGN_INS
        | R::REFUSAL_REASON_SERVER_BUSY => ErrorCode::ResourceExhausted,
        R::REFUSAL_REASON_ALREADY_SET_UP
        | R::REFUSAL_REASON_SIGN_IN_CODE_ALREADY_APPROVED
        | R::REFUSAL_REASON_LAST_ADMIN
        | R::REFUSAL_REASON_OWN_PASSWORD_RESET
        | R::REFUSAL_REASON_RECENT_AUTHENTICATION_REQUIRED
        | R::REFUSAL_REASON_ALREADY_IN_LIBRARY
        | R::REFUSAL_REASON_REQUEST_NOT_PENDING
        | R::REFUSAL_REASON_REQUEST_NOT_FAILED
        | R::REFUSAL_REASON_NOT_A_REQUESTER
        | R::REFUSAL_REASON_FOLLOWED_SHOW_NOT_PENDING
        | R::REFUSAL_REASON_NOT_A_FOLLOWED_SHOW_REQUESTER
        | R::REFUSAL_REASON_LAST_AUTHENTICATOR
        | R::REFUSAL_REASON_SETTINGS_CHECK_FAILED
        | R::REFUSAL_REASON_CHOICE_NOT_OFFERED
        | R::REFUSAL_REASON_FULFILLER_TAKEN
        | R::REFUSAL_REASON_PLUGIN_REFUSED => ErrorCode::FailedPrecondition,
        R::REFUSAL_REASON_SIGN_IN_CODE_NOT_FOUND
        | R::REFUSAL_REASON_HANDLE_NOT_FOUND
        | R::REFUSAL_REASON_USER_NOT_FOUND
        | R::REFUSAL_REASON_DEVICE_NOT_FOUND
        | R::REFUSAL_REASON_LIBRARY_NOT_FOUND
        | R::REFUSAL_REASON_NOTHING_MATCHED
        | R::REFUSAL_REASON_SEASON_NOT_FOUND
        | R::REFUSAL_REASON_VERSION_NOT_FOUND
        | R::REFUSAL_REASON_ITEM_NOT_FOUND
        | R::REFUSAL_REASON_REQUEST_NOT_FOUND
        | R::REFUSAL_REASON_FOLLOWED_SHOW_NOT_FOUND
        | R::REFUSAL_REASON_PLUGIN_NOT_FOUND => ErrorCode::NotFound,
        R::REFUSAL_REASON_USER_NAME_TAKEN | R::REFUSAL_REASON_FOLDER_TAKEN => {
            ErrorCode::AlreadyExists
        }
        R::REFUSAL_REASON_NO_METADATA_PROVIDER => ErrorCode::Unavailable,
    }
}

impl Refused {
    /// Names the request field, as a path (`season.show_id`), that the refusal is about.
    fn field(mut self, field: &str) -> Self {
        field.clone_into(&mut self.refusal.field);
        self
    }

    /// Names the Plugin setting `key` that the refusal is about, as `field` and a param.
    fn setting(self, key: &str) -> Self {
        self.field(&format!("settings.{key}"))
            .param("settingKey", &key)
    }

    /// Adds `value`, which the message mentions, as the param `key`: lowerCamelCase, with the unit
    /// in the key (AIP-193).
    fn param(mut self, key: &str, value: &impl ToString) -> Self {
        self.refusal
            .params
            .insert(key.to_owned(), value.to_string());
        self
    }
}

impl From<Refused> for ConnectError {
    fn from(refused: Refused) -> Self {
        let Refused {
            code,
            refusal,
            message,
        } = refused;
        let detail = ErrorDetail {
            // A Refusal is plain strings and an enum, so it always serializes.
            debug: serde_json::to_value(&refusal).ok(),
            ..ErrorDetail::from_message(Refusal::FULL_NAME, &refusal)
        };
        Self::new(code, message).with_detail(detail)
    }
}

/// The Connect error for the enum field `field` left unset, or set to a value this Server doesn't
/// know.
fn unknown_enum_value(field: &str, message: &str) -> ConnectError {
    refuse(RefusalReason::REFUSAL_REASON_UNKNOWN_ENUM_VALUE, message)
        .field(field)
        .into()
}

/// The Connect error a client gets when the database failed. The failure is logged, not sent: it
/// says nothing the client can act on.
#[expect(
    clippy::needless_pass_by_value,
    reason = "by value, so callers can write `.map_err(database_failed)`"
)]
fn database_failed(err: sqlx::Error) -> ConnectError {
    tracing::error!("a request failed: {err}");
    ConnectError::internal("the Server's database failed")
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
            requests::Error::NoSuchRequest(id) => {
                refuse(R::REFUSAL_REASON_REQUEST_NOT_FOUND, message)
                    .field("id")
                    .param("requestId", &id)
            }
            requests::Error::NotPending(id) => {
                refuse(R::REFUSAL_REASON_REQUEST_NOT_PENDING, message)
                    .field("id")
                    .param("requestId", &id)
            }
            requests::Error::NotFailed(id) => refuse(R::REFUSAL_REASON_REQUEST_NOT_FAILED, message)
                .field("id")
                .param("requestId", &id),
            requests::Error::NotARequester(id) => {
                refuse(R::REFUSAL_REASON_NOT_A_REQUESTER, message)
                    .field("id")
                    .param("requestId", &id)
            }
            requests::Error::NoSuchFollowedShow(id) => {
                refuse(R::REFUSAL_REASON_FOLLOWED_SHOW_NOT_FOUND, message)
                    .field("id")
                    .param("followedShowId", &id)
            }
            requests::Error::FollowedShowNotPending(id) => {
                refuse(R::REFUSAL_REASON_FOLLOWED_SHOW_NOT_PENDING, message)
                    .field("id")
                    .param("followedShowId", &id)
            }
            requests::Error::NotAFollowedShowRequester(id) => {
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

/// The client IP address `ctx` came from, which rate limits count by.
fn client_ip(ctx: &RequestContext) -> Result<IpAddr, ConnectError> {
    let source = ctx.extensions().get::<RequestSource>();
    source.map(|source| source.client_ip).ok_or_else(|| {
        tracing::error!("a request came without its peer's address");
        ConnectError::internal("the Server failed")
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{ACCESS, Access};

    /// Every RPC `buf generate` wrote a stub for, by path, read from the generated code so a new
    /// RPC shows up here without anyone listing it.
    fn generated_rpcs() -> Vec<String> {
        let generated = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../nama-proto/src/generated/connect"
        );
        let mut rpcs = Vec::new();
        for file in fs::read_dir(generated).expect("the generated code is checked in") {
            let code = fs::read_to_string(file.expect("readable").path()).expect("readable");
            rpcs.extend(
                code.lines()
                    .filter_map(|line| line.trim().strip_prefix("\"/nama."))
                    .filter_map(|rest| rest.strip_suffix("\","))
                    .map(|rest| format!("/nama.{rest}")),
            );
        }
        rpcs
    }

    #[test]
    fn every_generated_rpc_has_an_access_entry() {
        let rpcs = generated_rpcs();
        assert_eq!(rpcs.len(), ACCESS.len(), "the scan found {rpcs:?}");

        let unlisted: Vec<_> = rpcs
            .iter()
            .filter(|rpc| Access::of(rpc).is_none())
            .collect();

        assert!(unlisted.is_empty(), "list these in `ACCESS`: {unlisted:?}");
    }

    #[test]
    fn a_path_in_no_entry_has_no_access() {
        for path in [
            "/nama.v1.LibraryService/Unlisted",
            "/nama.v1.BrowseService/Unlisted",
            "/elsewhere",
        ] {
            assert!(Access::of(path).is_none(), "{path}");
        }
    }

    #[test]
    fn media_needs_a_session() {
        assert!(matches!(
            Access::of("/media/versions/1"),
            Some(Access::SignedIn)
        ));
    }
}
