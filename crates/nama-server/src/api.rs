//! The Server's ConnectRPC API (ADR 0007), served on axum.

use std::{ffi::OsStr, future, net::IpAddr, path::Path, sync::Arc, time::Instant};

use axum::{
    extract::Request,
    http::{
        HeaderMap, HeaderValue, StatusCode,
        header::{RETRY_AFTER, SET_COOKIE, USER_AGENT},
    },
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
use nama_plugin_api::metadata::{Candidate, ExternalId, SearchQuery, TitleKind};
use nama_proto::nama::v1::{
    self as proto, ApproveSignInCodeRequest, ApproveSignInCodeResponse, BrowseService,
    ChangePasswordRequest, ChangePasswordResponse, ClearMatchRequest, ClearMatchResponse,
    CreateLibraryRequest, CreateLibraryResponse, CreateRequestRequest, CreateRequestResponse,
    CreateUserRequest, CreateUserResponse, DeleteLibraryRequest, DeleteLibraryResponse,
    DeleteUserRequest, DeleteUserResponse, EventService, EventType, GetLibraryRequest,
    GetLibraryResponse, GetMovieRequest, GetMovieResponse, GetRequestRequest, GetRequestResponse,
    GetServerInfoRequest, GetServerInfoResponse, GetShowRequest, GetShowResponse, LibraryKind,
    LibraryService, ListContinueWatchingRequest, ListContinueWatchingResponse, ListDevicesRequest,
    ListDevicesResponse, ListEpisodesRequest, ListEpisodesResponse, ListLibrariesRequest,
    ListLibrariesResponse, ListMoviesRequest, ListMoviesResponse, ListRequestsRequest,
    ListRequestsResponse, ListShowsRequest, ListShowsResponse, ListUnmatchedItemsRequest,
    ListUnmatchedItemsResponse, ListUsersRequest, ListUsersResponse, LookUpMovieRequest,
    LookUpMovieResponse, LookUpSignInCodeRequest, LookUpSignInCodeResponse, MatchService,
    Permission, PollSignInCodeRequest, PollSignInCodeResponse, ReauthenticateRequest,
    ReauthenticateResponse, Refusal, RefusalReason, ReportProgressRequest, ReportProgressResponse,
    RequestService, ResetPasswordRequest, ResetPasswordResponse, RevokeDeviceRequest,
    RevokeDeviceResponse, ScanLibraryRequest, ScanLibraryResponse, SearchMetadataRequest,
    SearchMetadataResponse, SearchMoviesRequest, SearchMoviesResponse, ServerService,
    SessionDelivery, SetMatchRequest, SetMatchResponse, SetPermissionsRequest,
    SetPermissionsResponse, SetPlayedRequest, SetPlayedResponse, SetupRequest, SetupResponse,
    SignInCodeStatus, SignInRequest, SignInResponse, SignInService, SignOutRequest,
    SignOutResponse, StartSignInCodeRequest, StartSignInCodeResponse, SubscribeRequest,
    SubscribeResponse, UpdateLibraryRequest, UpdateLibraryResponse, UserService, WatchStateService,
    WhoAmIRequest, WhoAmIResponse, external_id, reauthenticate_request::Proof,
    set_played_request::Target,
};

use crate::{
    artwork::{self, Artwork},
    browse::{self, Browse, Started},
    config::Config,
    contents::{Contents, Episode, Item, MatchedTo, Metadata, Movie, Show, Unmatched, Version},
    db::{Database, now},
    events::{EventBus, Filter},
    forwarded::{RequestSource, identify_source},
    libraries::{self, FolderProblem, Kind, Libraries, Library},
    matching::{ManualError, Matcher},
    plugins::Registry,
    rate_limits::{Blocked, SignInCodeLimits, SignInLimits},
    requests::{self, Found, Requests},
    scanner::Scanner,
    sessions::{cleared_session_cookie, identify_caller, in_session_cookie, session_cookie},
    sign_in_codes::{self, Poll, SignInCodes},
    users::{self, Caller, NewDevice, Permissions, SetupCode, SignedIn, Users},
    versions,
    watch_states::{Progress, WatchState, WatchStates},
};

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
    plugins: Registry,
    scanner: Scanner,
    events: EventBus,
    setup_code: Option<SetupCode>,
) -> axum::Router {
    let users = Users::new(Arc::clone(&db), plugins.clone());
    let contents = Contents::new(Arc::clone(&db));
    let artwork = Artwork::new(&config.data_dir, plugins.clone());
    let artwork_router = artwork::router(artwork.clone(), contents.clone());
    let versions = versions::router(contents);
    let browse = Browse::new(Arc::clone(&db));
    let matcher = Matcher::new(Arc::clone(&db), plugins, events.clone());
    connectrpc::Router::new()
        .add_service(Arc::new(ServerServiceHandler {
            name: config.name.clone(),
            users: users.clone(),
        }))
        .add_service(Arc::new(SignInServiceHandler {
            users: users.clone(),
            setup_code,
            sign_in_limits: SignInLimits::default(),
            sign_in_codes: SignInCodes::default(),
            sign_in_code_limits: SignInCodeLimits::default(),
        }))
        .add_service(Arc::new(MatchServiceHandler {
            matcher: matcher.clone(),
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
        .add_service(Arc::new(RequestServiceHandler {
            requests: Requests::new(Arc::clone(&db), matcher, events.clone()),
            artwork,
        }))
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
        }))
        .into_axum_router()
        .merge(artwork_router)
        .merge(versions)
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

/// Asking for Movies.
const REQUEST_MOVIE: Access = Access::Needs(Permission::RequestMovie);

/// Who may call each RPC, by path; one missing here is refused to everyone (see AGENTS.md, "New
/// RPCs start closed"). Media has its own entry, [`MEDIA`].
// One row per RPC reads as a table; rustfmt would split most rows over four lines.
#[rustfmt::skip]
const ACCESS: &[(&str, Access)] = &[
    (proto::SERVER_SERVICE_GET_SERVER_INFO_SPEC.procedure,        Access::Anyone),
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
    (proto::EVENT_SERVICE_SUBSCRIBE_SPEC.procedure,               Access::SignedIn),
    (proto::USER_SERVICE_CREATE_USER_SPEC.procedure,              MANAGE_USERS),
    (proto::USER_SERVICE_LIST_USERS_SPEC.procedure,               MANAGE_USERS),
    (proto::USER_SERVICE_SET_PERMISSIONS_SPEC.procedure,          MANAGE_USERS),
    (proto::USER_SERVICE_DELETE_USER_SPEC.procedure,              MANAGE_USERS),
    (proto::USER_SERVICE_LIST_DEVICES_SPEC.procedure,             Access::SignedIn),
    (proto::USER_SERVICE_REVOKE_DEVICE_SPEC.procedure,            Access::SignedIn),
    (proto::USER_SERVICE_CHANGE_PASSWORD_SPEC.procedure,          Access::SignedIn),
    (proto::USER_SERVICE_RESET_PASSWORD_SPEC.procedure,           MANAGE_USERS),
    (proto::REQUEST_SERVICE_CREATE_REQUEST_SPEC.procedure,        REQUEST_MOVIE),
    (proto::REQUEST_SERVICE_LIST_REQUESTS_SPEC.procedure,         Access::SignedIn),
    (proto::REQUEST_SERVICE_GET_REQUEST_SPEC.procedure,           Access::SignedIn),
    (proto::REQUEST_SERVICE_SEARCH_MOVIES_SPEC.procedure,         REQUEST_MOVIE),
    (proto::REQUEST_SERVICE_LOOK_UP_MOVIE_SPEC.procedure,         REQUEST_MOVIE),
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
const NEEDS_RECENT_AUTHENTICATION: [&str; 5] = [
    proto::USER_SERVICE_CHANGE_PASSWORD_SPEC.procedure,
    proto::USER_SERVICE_RESET_PASSWORD_SPEC.procedure,
    proto::USER_SERVICE_SET_PERMISSIONS_SPEC.procedure,
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

/// `nama.v1.ServerService`: facts about this Server that any client can read.
struct ServerServiceHandler {
    name: String,
    users: Users,
}

#[expect(
    refining_impl_trait,
    reason = "plain `async fn` returning the owned response is the form the generated trait \
              documents; the refined signature isn't visible outside this crate"
)]
impl ServerService for ServerServiceHandler {
    async fn get_server_info(
        &self,
        _ctx: RequestContext,
        _request: ServiceRequest<'_, GetServerInfoRequest>,
    ) -> ServiceResult<GetServerInfoResponse> {
        let setup_required = !self.users.any().await.map_err(database_failed)?;
        Response::ok(GetServerInfoResponse {
            name: self.name.clone(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            setup_required,
            ..Default::default()
        })
    }
}

/// `nama.v1.SignInService`: signing Devices in, and saying who is signed in.
struct SignInServiceHandler {
    users: Users,
    /// What `Setup` must be given. `None` once the Server started with Users.
    setup_code: Option<SetupCode>,
    /// The recent failures of `Setup`, `SignIn` and `Reauthenticate`, which slow guessing down.
    sign_in_limits: SignInLimits,
    sign_in_codes: SignInCodes,
    /// The recent failed Sign-in Code lookups and approvals, which slow guessing codes down.
    sign_in_code_limits: SignInCodeLimits,
}

impl SignInServiceHandler {
    /// Looks up or approves a Sign-in Code with `use_code`, as the caller of `ctx` at the time it
    /// is given, unless their Device has failed too often lately. Only a code that is unknown or
    /// expired counts as a failure: that is what a guess gets.
    fn with_sign_in_code_limit<T>(
        &self,
        ctx: &RequestContext,
        use_code: impl FnOnce(&Caller, Instant) -> Result<T, sign_in_codes::Error>,
    ) -> Result<T, ConnectError> {
        let caller = caller(ctx)?;
        let now = Instant::now();
        let attempt = self.sign_in_code_limits.admit(caller.device.id, now)?;
        let used = use_code(caller, now);
        if !matches!(used, Err(sign_in_codes::Error::NotFound)) {
            self.sign_in_code_limits.wasnt_a_guess(&attempt);
        }
        Ok(used?)
    }
}

#[expect(
    refining_impl_trait,
    reason = "plain `async fn` returning the owned response is the form the generated trait \
              documents; the refined signature isn't visible outside this crate"
)]
impl SignInService for SignInServiceHandler {
    async fn setup(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, SetupRequest>,
    ) -> ServiceResult<SetupResponse> {
        let request = request.to_owned_message();
        let attempt = self.sign_in_limits.admit(client_ip(&ctx)?, None)?;
        let wants_token = wants_token(request.delivery)?;
        // The Server only made no code because it started with Users. Users are checked before the
        // code, so once there are any every attempt is refused as already set up, as documented.
        let setup_code = self.setup_code.as_ref().ok_or(users::Error::AlreadySetUp)?;
        if self.users.any().await.map_err(database_failed)? {
            return Err(users::Error::AlreadySetUp.into());
        }
        if !setup_code.matches(&request.setup_code) {
            return Err(refuse(
                RefusalReason::REFUSAL_REASON_WRONG_SETUP_CODE,
                "the Setup Code is wrong",
            )
            .field("setup_code")
            .into());
        }
        let device = new_device(request.device_name, &ctx)?;
        let signed_in = self
            .users
            .create_first_admin(&request.name, &request.password, device)
            .await?;
        self.sign_in_limits.succeeded(&attempt);
        Ok(hand_out_session(
            signed_in,
            wants_token,
            &ctx,
            |caller, token| SetupResponse {
                user: user_to_proto(caller.user).into(),
                device: device_to_proto(caller.device, true).into(),
                token,
                ..Default::default()
            },
        ))
    }

    async fn sign_in(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, SignInRequest>,
    ) -> ServiceResult<SignInResponse> {
        let request = request.to_owned_message();
        let attempt = self
            .sign_in_limits
            .admit(client_ip(&ctx)?, Some(&request.name))?;
        let wants_token = wants_token(request.delivery)?;
        let device = new_device(request.device_name, &ctx)?;
        let signed_in = self
            .users
            .sign_in(&request.name, &request.password, device)
            .await?;
        self.sign_in_limits.succeeded(&attempt);
        Ok(hand_out_session(
            signed_in,
            wants_token,
            &ctx,
            |caller, token| SignInResponse {
                user: user_to_proto(caller.user).into(),
                device: device_to_proto(caller.device, true).into(),
                token,
                ..Default::default()
            },
        ))
    }

    async fn sign_out(
        &self,
        ctx: RequestContext,
        _request: ServiceRequest<'_, SignOutRequest>,
    ) -> ServiceResult<SignOutResponse> {
        let device = caller(&ctx)?.device.id;
        self.users.sign_out(device).await.map_err(database_failed)?;
        Ok(signed_out(SignOutResponse::default(), &ctx))
    }

    fn who_am_i<'a>(
        &'a self,
        ctx: RequestContext,
        _request: ServiceRequest<'_, WhoAmIRequest>,
    ) -> impl Future<Output = ServiceResult<impl Encodable<WhoAmIResponse> + Send + use<'a>>> + Send
    {
        future::ready(caller(&ctx).and_then(|caller| {
            Response::ok(WhoAmIResponse {
                user: user_to_proto(caller.user.clone()).into(),
                device: device_to_proto(caller.device.clone(), true).into(),
                ..Default::default()
            })
        }))
    }

    async fn reauthenticate(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, ReauthenticateRequest>,
    ) -> ServiceResult<ReauthenticateResponse> {
        let caller = caller(&ctx)?;
        let attempt = self
            .sign_in_limits
            .admit(client_ip(&ctx)?, Some(&caller.user.name))?;
        let Some(Proof::Password(password)) = request.to_owned_message().proof else {
            return Err(refuse(
                RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
                "the proof of who you are is missing",
            )
            .field("proof")
            .into());
        };
        let reauthenticated = self.users.reauthenticate(caller, &password).await;
        // The Device is only found signed out after the password checked out, so that is no guess.
        if matches!(reauthenticated, Ok(()) | Err(users::Error::SignedOut)) {
            self.sign_in_limits.succeeded(&attempt);
        }
        reauthenticated?;
        Response::ok(ReauthenticateResponse::default())
    }

    fn start_sign_in_code<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, StartSignInCodeRequest>,
    ) -> impl Future<
        Output = ServiceResult<impl Encodable<StartSignInCodeResponse> + Send + use<'a>>,
    > + Send {
        let request = request.to_owned_message();
        let started = wants_token(request.delivery).and_then(|wants_token| {
            let device_name =
                users::checked_name("device_name", "a Device's name", &request.device_name)?;
            Ok(self
                .sign_in_codes
                .start(device_name, wants_token, Instant::now())?)
        });
        future::ready(started.and_then(|started| {
            Response::ok(StartSignInCodeResponse {
                code: started.code,
                handle: started.handle,
                ..Default::default()
            })
        }))
    }

    fn look_up_sign_in_code<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, LookUpSignInCodeRequest>,
    ) -> impl Future<
        Output = ServiceResult<impl Encodable<LookUpSignInCodeResponse> + Send + use<'a>>,
    > + Send {
        let looked_up = self
            .with_sign_in_code_limit(&ctx, |_, now| self.sign_in_codes.look_up(request.code, now));
        future::ready(looked_up.and_then(|device_name| {
            Response::ok(LookUpSignInCodeResponse {
                device_name,
                ..Default::default()
            })
        }))
    }

    fn approve_sign_in_code<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, ApproveSignInCodeRequest>,
    ) -> impl Future<
        Output = ServiceResult<impl Encodable<ApproveSignInCodeResponse> + Send + use<'a>>,
    > + Send {
        let approved = self.with_sign_in_code_limit(&ctx, |caller, now| {
            self.sign_in_codes.approve(request.code, &caller.user, now)
        });
        future::ready(approved.and_then(|()| Response::ok(ApproveSignInCodeResponse::default())))
    }

    async fn poll_sign_in_code(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, PollSignInCodeRequest>,
    ) -> ServiceResult<PollSignInCodeResponse> {
        // Before `poll`, which uses up an approved code: failing after it would lose the sign-in.
        let ip_address = client_ip(&ctx)?;
        let poll = self
            .sign_in_codes
            .poll(request.handle, Instant::now())
            .ok_or_else(|| {
                refuse(
                    RefusalReason::REFUSAL_REASON_HANDLE_NOT_FOUND,
                    "no sign-in is waiting on that handle",
                )
                .field("handle")
            })?;
        let (user, device_name, wants_token) = match poll {
            Poll::Waiting => return poll_status(SignInCodeStatus::SIGN_IN_CODE_STATUS_WAITING),
            Poll::Expired => return poll_status(SignInCodeStatus::SIGN_IN_CODE_STATUS_EXPIRED),
            Poll::Approved {
                user,
                device_name,
                wants_token,
            } => (user, device_name, wants_token),
        };
        let device = NewDevice {
            name: device_name,
            ip_address,
            user_agent: user_agent(ctx.headers()),
        };
        let signed_in = self.users.sign_in_as(user, None, device).await?;
        Ok(hand_out_session(
            signed_in,
            wants_token,
            &ctx,
            |caller, token| PollSignInCodeResponse {
                status: SignInCodeStatus::SIGN_IN_CODE_STATUS_SIGNED_IN.into(),
                user: user_to_proto(caller.user).into(),
                device: device_to_proto(caller.device, true).into(),
                token,
                ..Default::default()
            },
        ))
    }
}

/// A `PollSignInCode` answer that carries only `status`.
fn poll_status(status: SignInCodeStatus) -> ServiceResult<PollSignInCodeResponse> {
    Response::ok(PollSignInCodeResponse {
        status: status.into(),
        ..Default::default()
    })
}

/// `nama.v1.LibraryService`: managing the Server's Libraries. Needs `PERMISSION_MANAGE_LIBRARIES`,
/// which [`require_access`] enforces, except listing them, which any signed-in User may do to
/// browse.
struct LibraryServiceHandler {
    libraries: Libraries,
    scanner: Scanner,
}

#[expect(
    refining_impl_trait,
    reason = "plain `async fn` returning the owned response is the form the generated trait \
              documents; the refined signature isn't visible outside this crate"
)]
impl LibraryService for LibraryServiceHandler {
    async fn create_library(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, CreateLibraryRequest>,
    ) -> ServiceResult<CreateLibraryResponse> {
        let request = request.to_owned_message();
        let kind = kind_from_proto(request.kind)?;
        let library = self
            .libraries
            .create(&request.name, kind, request.folders)
            .await?;
        Response::ok(CreateLibraryResponse {
            library: library_to_proto(library).into(),
            ..Default::default()
        })
    }

    async fn list_libraries(
        &self,
        ctx: RequestContext,
        _request: ServiceRequest<'_, ListLibrariesRequest>,
    ) -> ServiceResult<ListLibrariesResponse> {
        // Disk paths are for those who manage Libraries (see AGENTS.md).
        let sees_folders = caller(&ctx)?
            .user
            .permissions
            .allow(Permission::ManageLibraries);
        let libraries = self.libraries.list().await?;
        Response::ok(ListLibrariesResponse {
            libraries: libraries
                .into_iter()
                .map(|library| Library {
                    folders: if sees_folders {
                        library.folders
                    } else {
                        Vec::new()
                    },
                    ..library
                })
                .map(library_to_proto)
                .collect(),
            ..Default::default()
        })
    }

    async fn get_library(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, GetLibraryRequest>,
    ) -> ServiceResult<GetLibraryResponse> {
        let library = self.libraries.get(request.id).await?;
        Response::ok(GetLibraryResponse {
            library: library_to_proto(library).into(),
            ..Default::default()
        })
    }

    async fn update_library(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, UpdateLibraryRequest>,
    ) -> ServiceResult<UpdateLibraryResponse> {
        let request = request.to_owned_message();
        let library = self
            .libraries
            .update(request.id, &request.name, request.folders)
            .await?;
        Response::ok(UpdateLibraryResponse {
            library: library_to_proto(library).into(),
            ..Default::default()
        })
    }

    async fn delete_library(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, DeleteLibraryRequest>,
    ) -> ServiceResult<DeleteLibraryResponse> {
        self.libraries.delete(request.id).await?;
        Response::ok(DeleteLibraryResponse::default())
    }

    async fn scan_library(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, ScanLibraryRequest>,
    ) -> ServiceResult<ScanLibraryResponse> {
        self.scanner.start(request.id).await?;
        Response::ok(ScanLibraryResponse::default())
    }
}

/// `nama.v1.MatchService`: fixing Matches by hand. Needs `PERMISSION_MANAGE_LIBRARIES`, which
/// [`require_access`] enforces.
struct MatchServiceHandler {
    matcher: Matcher,
    contents: Contents,
    libraries: Libraries,
    /// Where the scan lock a Match is fixed under comes from.
    scanner: Scanner,
}

#[expect(
    refining_impl_trait,
    reason = "plain `async fn` returning the owned response is the form the generated trait \
              documents; the refined signature isn't visible outside this crate"
)]
impl MatchService for MatchServiceHandler {
    async fn search_metadata(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, SearchMetadataRequest>,
    ) -> ServiceResult<SearchMetadataResponse> {
        let request = request.to_owned_message();
        if request.title.trim().is_empty() {
            return Err(refuse(
                RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
                "the title to search for must not be blank",
            )
            .field("title")
            .into());
        }
        let query = SearchQuery {
            kind: title_kind_from_proto(request.kind)?,
            title: request.title,
            year: request.year,
        };
        let candidates = self.matcher.search_metadata(&query).await?;
        Response::ok(SearchMetadataResponse {
            candidates: candidates
                .into_iter()
                .map(|(_, candidate)| candidate_to_proto(candidate))
                .collect(),
            ..Default::default()
        })
    }

    async fn set_match(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, SetMatchRequest>,
    ) -> ServiceResult<SetMatchResponse> {
        let request = request.to_owned_message();
        let title = request
            .title_id
            .into_option()
            .and_then(|id| id.id)
            .map(external_id_from_proto)
            .ok_or_else(|| {
                refuse(
                    RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
                    "the title to Match to is missing",
                )
                .field("title_id")
            })?;
        let scan = self.scanner.lock_library_of(request.item_id).await?;
        let summary = self
            .matcher
            .set_match(&scan, request.item_id, &title)
            .await?;
        Response::ok(SetMatchResponse {
            matched: summary.matched,
            unmatched: summary.unmatched,
            ..Default::default()
        })
    }

    async fn clear_match(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, ClearMatchRequest>,
    ) -> ServiceResult<ClearMatchResponse> {
        let scan = self.scanner.lock_library_of(request.item_id).await?;
        let summary = self.matcher.clear_match(&scan, request.item_id).await?;
        Response::ok(ClearMatchResponse {
            unmatched: summary.unmatched,
            ..Default::default()
        })
    }

    async fn list_unmatched_items(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, ListUnmatchedItemsRequest>,
    ) -> ServiceResult<ListUnmatchedItemsResponse> {
        self.libraries.get(request.library_id).await?;
        let unmatched = self
            .contents
            .unmatched(request.library_id)
            .await
            .map_err(database_failed)?;
        let unparsed_paths = self
            .contents
            .unparsed(request.library_id)
            .await
            .map_err(database_failed)?;
        Response::ok(ListUnmatchedItemsResponse {
            items: unmatched.into_iter().map(unmatched_to_proto).collect(),
            unparsed_paths,
            ..Default::default()
        })
    }
}

/// `nama.v1.BrowseService`: what a Library holds, for any signed-in User, which
/// [`require_access`] enforces.
struct BrowseServiceHandler {
    browse: Browse,
}

#[expect(
    refining_impl_trait,
    reason = "plain `async fn` returning the owned response is the form the generated trait \
              documents; the refined signature isn't visible outside this crate"
)]
impl BrowseService for BrowseServiceHandler {
    async fn list_movies(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, ListMoviesRequest>,
    ) -> ServiceResult<ListMoviesResponse> {
        let user = caller(&ctx)?.user.id;
        let movies = self.browse.movies(user, request.library_id).await?;
        Response::ok(ListMoviesResponse {
            movies: movies
                .into_iter()
                .map(|(movie, state)| movie_to_proto(movie, state))
                .collect(),
            ..Default::default()
        })
    }

    async fn get_movie(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, GetMovieRequest>,
    ) -> ServiceResult<GetMovieResponse> {
        let user = caller(&ctx)?.user.id;
        let request = request.to_owned_message();
        let id = required_id(request.movie_id.into_option(), "movie_id", "Movie")?;
        let (movie, state) = self.browse.movie(user, request.library_id, &id).await?;
        Response::ok(GetMovieResponse {
            movie: movie_to_proto(movie, state).into(),
            ..Default::default()
        })
    }

    async fn list_shows(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, ListShowsRequest>,
    ) -> ServiceResult<ListShowsResponse> {
        let shows = self.browse.shows(request.library_id).await?;
        Response::ok(ListShowsResponse {
            shows: shows.iter().map(show_to_proto).collect(),
            ..Default::default()
        })
    }

    async fn get_show(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, GetShowRequest>,
    ) -> ServiceResult<GetShowResponse> {
        let request = request.to_owned_message();
        let id = required_id(request.show_id.into_option(), "show_id", "Show")?;
        let show = self.browse.show(request.library_id, &id).await?;
        let seasons = show
            .seasons
            .iter()
            .map(|(&number, season)| proto::Season {
                number: proto_number(number),
                poster_url: season
                    .poster
                    .as_deref()
                    .map(|path| artwork::url(&show.plugin, path)),
                ..Default::default()
            })
            .collect();
        Response::ok(GetShowResponse {
            show: show_to_proto(&show).into(),
            seasons,
            ..Default::default()
        })
    }

    async fn list_episodes(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, ListEpisodesRequest>,
    ) -> ServiceResult<ListEpisodesResponse> {
        let user = caller(&ctx)?.user.id;
        let request = request.to_owned_message();
        let number = request.season_number;
        let show = required_id(request.show_id.into_option(), "show_id", "Show")?;
        let episodes = self
            .browse
            .episodes(user, request.library_id, &show, i64::from(number))
            .await?;
        Response::ok(ListEpisodesResponse {
            episodes: episodes
                .into_iter()
                .map(|(episode, state)| episode_to_proto(number, episode, state))
                .collect(),
            ..Default::default()
        })
    }

    async fn list_continue_watching(
        &self,
        ctx: RequestContext,
        _request: ServiceRequest<'_, ListContinueWatchingRequest>,
    ) -> ServiceResult<ListContinueWatchingResponse> {
        let started = self.browse.continue_watching(caller(&ctx)?.user.id).await?;
        Response::ok(ListContinueWatchingResponse {
            entries: started.into_iter().map(started_to_proto).collect(),
            ..Default::default()
        })
    }
}

/// `nama.v1.WatchStateService`: each signed-in User's progress through what they play.
struct WatchStateServiceHandler {
    contents: Contents,
    watch_states: WatchStates,
    /// Finds what `SetPlayed` marks as browsing does, so it fails as browsing does.
    browse: Browse,
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
            MatchedTo::NoSuchVersion => {
                return Err(refuse(
                    RefusalReason::REFUSAL_REASON_VERSION_NOT_FOUND,
                    "there is no such Version",
                )
                .field("version_id")
                .into());
            }
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

/// `nama.v1.UserService`: managing the Server's Users and their Devices. Needs
/// `PERMISSION_MANAGE_USERS`, which [`require_access`] enforces, but for the self-service RPCs
/// [`ACCESS`] opens to any session, which check for themselves with [`devices_owner`] or act only
/// on the caller.
struct UserServiceHandler {
    users: Users,
}

#[expect(
    refining_impl_trait,
    reason = "plain `async fn` returning the owned response is the form the generated trait \
              documents; the refined signature isn't visible outside this crate"
)]
impl UserService for UserServiceHandler {
    async fn create_user(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, CreateUserRequest>,
    ) -> ServiceResult<CreateUserResponse> {
        let request = request.to_owned_message();
        let id = self.users.create(&request.name, &request.password).await?;
        Response::ok(CreateUserResponse {
            id,
            ..Default::default()
        })
    }

    async fn list_users(
        &self,
        _ctx: RequestContext,
        _request: ServiceRequest<'_, ListUsersRequest>,
    ) -> ServiceResult<ListUsersResponse> {
        let users = self.users.list().await.map_err(database_failed)?;
        Response::ok(ListUsersResponse {
            users: users.into_iter().map(user_to_proto).collect(),
            ..Default::default()
        })
    }

    async fn set_permissions(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, SetPermissionsRequest>,
    ) -> ServiceResult<SetPermissionsResponse> {
        let request = request.to_owned_message();
        let permissions = permissions_from_proto(request.permissions)?;
        self.users
            .set_permissions(&caller(&ctx)?.user, request.user_id, permissions)
            .await?;
        Response::ok(SetPermissionsResponse::default())
    }

    async fn delete_user(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, DeleteUserRequest>,
    ) -> ServiceResult<DeleteUserResponse> {
        let caller = caller(&ctx)?;
        self.users.delete(&caller.user, request.user_id).await?;
        let response = DeleteUserResponse::default();
        Ok(if request.user_id == caller.user.id {
            signed_out(response, &ctx)
        } else {
            Response::new(response)
        })
    }

    async fn list_devices(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, ListDevicesRequest>,
    ) -> ServiceResult<ListDevicesResponse> {
        let caller = caller(&ctx)?;
        let owner = devices_owner(caller, request.user_id)?;
        let devices = self
            .users
            .devices(owner)
            .await
            .map_err(database_failed)?
            .ok_or_else(no_such_user)?;
        let current = caller.device.id;
        Response::ok(ListDevicesResponse {
            devices: devices
                .into_iter()
                .map(|device| {
                    let is_current = device.id == current;
                    device_to_proto(device, is_current)
                })
                .collect(),
            ..Default::default()
        })
    }

    async fn revoke_device(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, RevokeDeviceRequest>,
    ) -> ServiceResult<RevokeDeviceResponse> {
        let caller = caller(&ctx)?;
        let owner = devices_owner(caller, request.user_id)?;
        let revoked = self
            .users
            .revoke(owner, request.device_id)
            .await
            .map_err(database_failed)?;
        if !revoked {
            return Err(refuse(
                RefusalReason::REFUSAL_REASON_DEVICE_NOT_FOUND,
                "that User has no such Device",
            )
            .field("device_id")
            .into());
        }
        let response = RevokeDeviceResponse::default();
        Ok(if request.device_id == caller.device.id {
            signed_out(response, &ctx)
        } else {
            Response::new(response)
        })
    }

    async fn change_password(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, ChangePasswordRequest>,
    ) -> ServiceResult<ChangePasswordResponse> {
        let request = request.to_owned_message();
        self.users
            .change_password(caller(&ctx)?, &request.new_password)
            .await?;
        Response::ok(ChangePasswordResponse::default())
    }

    async fn reset_password(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, ResetPasswordRequest>,
    ) -> ServiceResult<ResetPasswordResponse> {
        let request = request.to_owned_message();
        let caller = caller(&ctx)?;
        // Resetting would sign out the calling Device too; `ChangePassword` keeps it.
        if request.user_id == caller.user.id {
            return Err(refuse(
                RefusalReason::REFUSAL_REASON_OWN_PASSWORD_RESET,
                "change your own password with ChangePassword",
            )
            .field("user_id")
            .into());
        }
        self.users
            .reset_password(&caller.user, request.user_id, &request.new_password)
            .await?;
        Response::ok(ResetPasswordResponse::default())
    }
}

/// The User whose Devices `caller` asks about with `user_id`: their own if it's unset. Only a User
/// who manages Users may ask about another User's. Anyone else who does gets `not_found`, as for a
/// User who doesn't exist, so they can't learn which IDs do.
fn devices_owner(caller: &Caller, user_id: Option<i64>) -> Result<i64, ConnectError> {
    match user_id {
        None => Ok(caller.user.id),
        Some(id)
            if id == caller.user.id || caller.user.permissions.allow(Permission::ManageUsers) =>
        {
            Ok(id)
        }
        Some(_) => Err(no_such_user()),
    }
}

/// The Connect error for the request's `user_id` naming nobody the caller may see.
fn no_such_user() -> ConnectError {
    users::Error::NoSuchUser.into()
}

/// `nama.v1.RequestService`: Users asking for Movies. Asking needs `PERMISSION_REQUEST_MOVIE`,
/// which [`require_access`] enforces; without `PERMISSION_VIEW_REQUESTS`, [`Requests`] shows a
/// User only their own Requests.
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
        let request = self.requests.create(asker, request.movie_tmdb_id).await?;
        Response::ok(CreateRequestResponse {
            request: request.into(),
            ..Default::default()
        })
    }

    async fn list_requests(
        &self,
        ctx: RequestContext,
        _request: ServiceRequest<'_, ListRequestsRequest>,
    ) -> ServiceResult<ListRequestsResponse> {
        let viewer = &caller(&ctx)?.user;
        let requests = self.requests.list(viewer).await.map_err(database_failed)?;
        Response::ok(ListRequestsResponse {
            requests,
            ..Default::default()
        })
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

    async fn search_movies(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, SearchMoviesRequest>,
    ) -> ServiceResult<SearchMoviesResponse> {
        let viewer = &caller(&ctx)?.user;
        let title = request.to_owned_message().title;
        if title.trim().is_empty() {
            return Err(refuse(
                RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
                "the title to search for must not be blank",
            )
            .field("title")
            .into());
        }
        let found = self.requests.search(viewer, title).await?;
        Response::ok(SearchMoviesResponse {
            movies: found
                .into_iter()
                .map(|found| self.searched_to_proto(found))
                .collect(),
            ..Default::default()
        })
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
            };
            let signed_out = self.users.signed_out(caller.device.id);
            let events = self.events.subscribe(filter).take_until(signed_out);
            let stream: ServiceStream<SubscribeResponse> = Box::pin(events.map(Ok));
            Response::ok(stream)
        });
        future::ready(subscribed)
    }
}

/// Whether a client asking for `delivery` wants its session token in the response body, rather
/// than as a cookie.
fn wants_token(delivery: EnumValue<SessionDelivery>) -> Result<bool, ConnectError> {
    match delivery {
        EnumValue::Known(SessionDelivery::SESSION_DELIVERY_TOKEN) => Ok(true),
        EnumValue::Known(SessionDelivery::SESSION_DELIVERY_COOKIE) => Ok(false),
        EnumValue::Known(SessionDelivery::SESSION_DELIVERY_UNSPECIFIED) | EnumValue::Unknown(_) => {
            Err(unknown_enum_value(
                "delivery",
                "the session delivery must be cookie or token",
            ))
        }
    }
}

/// The name a signing-in Device gave, or, when it gave none, one made from its `User-Agent`.
fn device_name(given: String, headers: &HeaderMap) -> String {
    if !given.trim().is_empty() {
        return given;
    }
    crate::device_names::from_user_agent(user_agent(headers).as_deref())
}

/// The Device that signs in with the request `ctx`, under the name [`device_name`] makes of `given`.
fn new_device(given: String, ctx: &RequestContext) -> Result<NewDevice, ConnectError> {
    Ok(NewDevice {
        name: device_name(given, ctx.headers()),
        ip_address: client_ip(ctx)?,
        user_agent: user_agent(ctx.headers()),
    })
}

/// The `User-Agent` `headers` carry, cut short, since any client can send a long one and it is
/// stored with the Device.
fn user_agent(headers: &HeaderMap) -> Option<String> {
    const LONGEST: usize = 512;
    let value = headers.get(USER_AGENT)?.to_str().ok()?;
    Some(value.chars().take(LONGEST).collect())
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

/// The response `body` makes for a Device just `signed_in`, with its session token in the body for
/// a client that `wants_token`, or else in a cookie and not in the body.
fn hand_out_session<M>(
    signed_in: SignedIn,
    wants_token: bool,
    ctx: &RequestContext,
    body: impl FnOnce(Caller, String) -> M,
) -> Response<M> {
    let SignedIn { caller, token } = signed_in;
    if wants_token {
        Response::new(body(caller, token))
    } else {
        let cookie = session_cookie(&token, ctx.extensions());
        Response::new(body(caller, String::new())).with_header(SET_COOKIE, cookie)
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

fn candidate_to_proto(candidate: Candidate) -> proto::TitleCandidate {
    proto::TitleCandidate {
        id: external_id_to_proto(candidate.id).into(),
        title: candidate.title,
        original_title: candidate.original_title,
        year: candidate.year,
        ..Default::default()
    }
}

fn unmatched_to_proto(unmatched: Unmatched) -> proto::UnmatchedItem {
    let (kind, title, year, numbers) = match unmatched.item {
        Item::Movie { title, year, .. } => (proto::TitleKind::TITLE_KIND_MOVIE, title, year, None),
        Item::Episode {
            show,
            year,
            season,
            episode,
            ..
        } => (
            proto::TitleKind::TITLE_KIND_SHOW,
            show,
            year,
            Some((season, episode)),
        ),
    };
    proto::UnmatchedItem {
        item_id: unmatched.id,
        path: unmatched.path,
        kind: kind.into(),
        title,
        year: year.and_then(int32),
        season: numbers.and_then(|(season, _)| int32(season)),
        episode: numbers.and_then(|(_, episode)| int32(episode)),
        ..Default::default()
    }
}

/// A Movie as clients see it, with its images as URLs on this Server, and the caller's Watch
/// State of it, if they've started it.
fn movie_to_proto(movie: Movie, watch_state: Option<WatchState>) -> proto::Movie {
    let plugin = movie.plugin;
    let artwork_url = |path: String| artwork::url(&plugin, &path);
    proto::Movie {
        id: external_id_to_proto(movie.id).into(),
        title: movie.title,
        original_title: movie.original_title,
        overview: movie.overview,
        release_date: movie.release_date,
        runtime_minutes: movie
            .runtime_minutes
            .and_then(|minutes| u32::try_from(minutes).ok()),
        poster_url: movie.poster.map(artwork_url),
        backdrop_url: movie.backdrop.map(artwork_url),
        versions: movie.versions.iter().map(version_to_proto).collect(),
        watch_state: watch_state.map(watch_state_to_proto).into(),
        ..Default::default()
    }
}

/// A Continue Watching entry as clients see it.
fn started_to_proto(started: Started) -> proto::ContinueWatchingEntry {
    let (library_id, movie_or_episode, show) = match started {
        Started::Movie {
            library,
            movie,
            state,
        } => (library, movie_to_proto(movie, Some(state)).into(), None),
        Started::Episode {
            library,
            show,
            season,
            episode,
            state,
        } => {
            let episode = episode_to_proto(proto_number(season), episode, Some(state));
            (library, episode.into(), Some(show_to_proto(&show)))
        }
    };
    proto::ContinueWatchingEntry {
        library_id,
        movie_or_episode: Some(movie_or_episode),
        show: show.into(),
        ..Default::default()
    }
}

fn watch_state_to_proto(state: WatchState) -> proto::WatchState {
    // Stored positions and durations are never negative.
    let unsigned = |ms: i64| u64::try_from(ms).unwrap_or_default();
    proto::WatchState {
        position_ms: unsigned(state.position_ms),
        duration_ms: unsigned(state.duration_ms),
        played: state.played,
        updated_at: state.updated_at,
        ..Default::default()
    }
}

/// A Show as clients see it, without its Seasons, with its images as URLs on this Server.
fn show_to_proto(show: &Show) -> proto::Show {
    let artwork_url = |path: &str| artwork::url(&show.plugin, path);
    proto::Show {
        id: external_id_to_proto(show.id.clone()).into(),
        title: show.title.clone(),
        poster_url: show.poster().map(artwork_url),
        backdrop_url: show.backdrop.as_deref().map(artwork_url),
        ..Default::default()
    }
}

/// An Episode of Season `season` as clients see it, with the caller's Watch State of it, if
/// they've started it.
fn episode_to_proto(
    season: u32,
    episode: Episode,
    watch_state: Option<WatchState>,
) -> proto::Episode {
    proto::Episode {
        id: external_id_to_proto(episode.id).into(),
        season_number: season,
        episode_number: proto_number(episode.number),
        title: episode.title,
        overview: episode.overview,
        air_date: episode.air_date,
        runtime_minutes: episode
            .runtime_minutes
            .and_then(|minutes| u32::try_from(minutes).ok()),
        versions: episode.versions.iter().map(version_to_proto).collect(),
        watch_state: watch_state.map(watch_state_to_proto).into(),
        ..Default::default()
    }
}

/// A season or episode number as a proto `uint32`. A scan parses them from digits, so they are
/// never negative.
fn proto_number(n: i64) -> u32 {
    u32::try_from(n).unwrap_or_default()
}

/// A Version as clients see it: its file's name, not the folders it is in.
fn version_to_proto(version: &Version) -> proto::Version {
    // A stored path came from a `String`, so its name is UTF-8.
    let file_name = Path::new(&version.path)
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or_default()
        .to_owned();
    proto::Version {
        id: version.id,
        file_name,
        // A size is never negative.
        size: u64::try_from(version.size).unwrap_or_default(),
        ..Default::default()
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

fn kind_from_proto(kind: EnumValue<LibraryKind>) -> Result<Kind, ConnectError> {
    match kind {
        EnumValue::Known(LibraryKind::LIBRARY_KIND_MOVIES) => Ok(Kind::Movies),
        EnumValue::Known(LibraryKind::LIBRARY_KIND_SHOWS) => Ok(Kind::Shows),
        EnumValue::Known(LibraryKind::LIBRARY_KIND_UNSPECIFIED) | EnumValue::Unknown(_) => Err(
            unknown_enum_value("kind", "a Library's kind must be movies or shows"),
        ),
    }
}

fn library_to_proto(library: Library) -> proto::Library {
    proto::Library {
        id: library.id,
        name: library.name,
        kind: match library.kind {
            Kind::Movies => LibraryKind::LIBRARY_KIND_MOVIES,
            Kind::Shows => LibraryKind::LIBRARY_KIND_SHOWS,
        }
        .into(),
        folders: library.folders,
        ..Default::default()
    }
}

fn permissions_from_proto(
    permissions: Vec<EnumValue<Permission>>,
) -> Result<Permissions, ConnectError> {
    permissions
        .into_iter()
        .map(|permission| match permission {
            EnumValue::Known(Permission::Unspecified) | EnumValue::Unknown(_) => {
                Err(unknown_enum_value(
                    "permissions",
                    "every Permission must be one the Server knows",
                ))
            }
            EnumValue::Known(permission) => Ok(permission),
        })
        .collect()
}

fn user_to_proto(user: users::User) -> proto::User {
    proto::User {
        id: user.id,
        name: user.name,
        permissions: user.permissions.iter().map(EnumValue::from).collect(),
        created_at: user.created_at,
        ..Default::default()
    }
}

/// A Device as clients see it, where `is_current` says whether it is the one calling.
fn device_to_proto(device: users::Device, is_current: bool) -> proto::Device {
    proto::Device {
        id: device.id,
        name: device.name,
        created_at: device.created_at,
        last_seen_at: device.last_seen_at,
        is_current,
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
        | R::REFUSAL_REASON_FOLDER_REPEATED => ErrorCode::InvalidArgument,
        R::REFUSAL_REASON_SIGN_IN_REQUIRED
        | R::REFUSAL_REASON_WRONG_NAME_OR_PASSWORD
        | R::REFUSAL_REASON_WRONG_PASSWORD
        | R::REFUSAL_REASON_SIGNED_OUT => ErrorCode::Unauthenticated,
        R::REFUSAL_REASON_PERMISSION_REQUIRED | R::REFUSAL_REASON_WRONG_SETUP_CODE => {
            ErrorCode::PermissionDenied
        }
        R::REFUSAL_REASON_TOO_MANY_ATTEMPTS | R::REFUSAL_REASON_TOO_MANY_PENDING_SIGN_INS => {
            ErrorCode::ResourceExhausted
        }
        R::REFUSAL_REASON_ALREADY_SET_UP
        | R::REFUSAL_REASON_SIGN_IN_CODE_ALREADY_APPROVED
        | R::REFUSAL_REASON_LAST_ADMIN
        | R::REFUSAL_REASON_OWN_PASSWORD_RESET
        | R::REFUSAL_REASON_RECENT_AUTHENTICATION_REQUIRED
        | R::REFUSAL_REASON_ALREADY_IN_LIBRARY => ErrorCode::FailedPrecondition,
        R::REFUSAL_REASON_SIGN_IN_CODE_NOT_FOUND
        | R::REFUSAL_REASON_HANDLE_NOT_FOUND
        | R::REFUSAL_REASON_USER_NOT_FOUND
        | R::REFUSAL_REASON_DEVICE_NOT_FOUND
        | R::REFUSAL_REASON_LIBRARY_NOT_FOUND
        | R::REFUSAL_REASON_NOTHING_MATCHED
        | R::REFUSAL_REASON_SEASON_NOT_FOUND
        | R::REFUSAL_REASON_VERSION_NOT_FOUND
        | R::REFUSAL_REASON_ITEM_NOT_FOUND
        | R::REFUSAL_REASON_REQUEST_NOT_FOUND => ErrorCode::NotFound,
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

/// The Connect error a client gets for a change to the Users that failed. Server-side failures
/// are logged, not sent.
impl From<users::Error> for ConnectError {
    fn from(err: users::Error) -> Self {
        use RefusalReason as R;
        let message = err.to_string();
        let refused = match err {
            users::Error::NameLength { field, .. } => {
                refuse(R::REFUSAL_REASON_NAME_LENGTH, message)
                    .field(field)
                    .param("minLength", users::NAME_LENGTH.start())
                    .param("maxLength", users::NAME_LENGTH.end())
            }
            users::Error::PasswordRejected { field, .. } => {
                refuse(R::REFUSAL_REASON_PASSWORD_REJECTED, message).field(field)
            }
            // Only creating a User or setting up the first one takes a name to check.
            users::Error::NameTaken => {
                refuse(R::REFUSAL_REASON_USER_NAME_TAKEN, message).field("name")
            }
            // Every call that names a User names them in `user_id`.
            users::Error::NoSuchUser => {
                refuse(R::REFUSAL_REASON_USER_NOT_FOUND, message).field("user_id")
            }
            users::Error::AlreadySetUp => refuse(R::REFUSAL_REASON_ALREADY_SET_UP, message),
            users::Error::LastAdmin => refuse(R::REFUSAL_REASON_LAST_ADMIN, message),
            users::Error::AdminRequired => return permission_required(Permission::Admin),
            users::Error::WrongNameOrPassword => {
                refuse(R::REFUSAL_REASON_WRONG_NAME_OR_PASSWORD, message)
            }
            users::Error::WrongPassword => {
                refuse(R::REFUSAL_REASON_WRONG_PASSWORD, message).field("password")
            }
            users::Error::SignedOut => refuse(R::REFUSAL_REASON_SIGNED_OUT, message),
            users::Error::Database(err) => return database_failed(err),
            users::Error::NoAuthenticator
            | users::Error::Authenticator(_)
            | users::Error::Random(_) => {
                tracing::error!("a change to the Users failed: {err}");
                return Self::internal("the Server failed");
            }
        };
        refused.into()
    }
}

/// The Connect error a client gets for a Request that couldn't be made or found. Database failures
/// are logged, not sent.
impl From<requests::Error> for ConnectError {
    fn from(err: requests::Error) -> Self {
        use RefusalReason as R;
        let message = err.to_string();
        let refused = match err {
            // Only `GetRequest` names a Request, and only `CreateRequest` and `LookUpMovie` a
            // Movie, both in `movie_tmdb_id`.
            requests::Error::NoSuchRequest(id) => {
                refuse(R::REFUSAL_REASON_REQUEST_NOT_FOUND, message)
                    .field("id")
                    .param("requestId", &id)
            }
            requests::Error::InLibrary(id) => refuse(R::REFUSAL_REASON_ALREADY_IN_LIBRARY, message)
                .field("movie_tmdb_id")
                .param("tmdbId", &id),
            requests::Error::NoSuchMovie(id) => refuse(R::REFUSAL_REASON_UNKNOWN_TITLE, message)
                .field("movie_tmdb_id")
                .param("titleId", &ExternalId::Tmdb(id)),
            requests::Error::Unavailable => refuse(R::REFUSAL_REASON_NO_METADATA_PROVIDER, message),
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

/// The Connect error a client gets for a rate-limited attempt, with the delay to wait in
/// `Retry-After`.
impl From<Blocked> for ConnectError {
    fn from(blocked: Blocked) -> Self {
        let retry_after_secs = blocked.retry_after_secs();
        let headers = HeaderMap::from_iter([(RETRY_AFTER, HeaderValue::from(retry_after_secs))]);
        Self::from(
            refuse(
                RefusalReason::REFUSAL_REASON_TOO_MANY_ATTEMPTS,
                blocked.to_string(),
            )
            .param("retryAfterSeconds", &retry_after_secs),
        )
        .with_headers(headers)
    }
}

/// The Connect error a client gets for a Sign-in Code that couldn't be started, looked up or
/// approved. A random number generator that failed is logged, not sent.
impl From<sign_in_codes::Error> for ConnectError {
    fn from(err: sign_in_codes::Error) -> Self {
        use RefusalReason as R;
        let message = err.to_string();
        let refused = match err {
            sign_in_codes::Error::NotFound => {
                refuse(R::REFUSAL_REASON_SIGN_IN_CODE_NOT_FOUND, message).field("code")
            }
            sign_in_codes::Error::AlreadyApproved => {
                refuse(R::REFUSAL_REASON_SIGN_IN_CODE_ALREADY_APPROVED, message).field("code")
            }
            sign_in_codes::Error::TooManyPending => {
                refuse(R::REFUSAL_REASON_TOO_MANY_PENDING_SIGN_INS, message)
            }
            sign_in_codes::Error::Random(_) => {
                tracing::error!("starting a Sign-in Code failed: {err}");
                return Self::internal("the Server failed");
            }
        };
        refused.into()
    }
}

/// The Connect error a client gets for a Library request that failed. Database failures are
/// logged, not sent: they say nothing the client can act on.
impl From<libraries::Error> for ConnectError {
    fn from(err: libraries::Error) -> Self {
        use RefusalReason as R;
        let message = err.to_string();
        let refused = match err {
            libraries::Error::NameBlank => {
                refuse(R::REFUSAL_REASON_FIELD_REQUIRED, message).field("name")
            }
            libraries::Error::FoldersEmpty => {
                refuse(R::REFUSAL_REASON_FIELD_REQUIRED, message).field("folders")
            }
            libraries::Error::Folder {
                index,
                folder,
                problem,
            } => {
                let reason = match problem {
                    FolderProblem::NotAbsolute => R::REFUSAL_REASON_FOLDER_NOT_ABSOLUTE,
                    FolderProblem::NotFound(_) => R::REFUSAL_REASON_FOLDER_NOT_FOUND,
                    FolderProblem::NotReadable(_) => R::REFUSAL_REASON_FOLDER_NOT_READABLE,
                    FolderProblem::NotUtf8(_) => R::REFUSAL_REASON_FOLDER_NOT_UTF8,
                    FolderProblem::Repeated => R::REFUSAL_REASON_FOLDER_REPEATED,
                };
                let refused = refuse(reason, message)
                    .field(&format!("folders[{index}]"))
                    .param("folder", &folder);
                if let FolderProblem::NotUtf8(resolved) = problem {
                    refused.param("resolvedPath", &resolved)
                } else {
                    refused
                }
            }
            libraries::Error::NotFound(id) => {
                refuse(R::REFUSAL_REASON_LIBRARY_NOT_FOUND, message).param("libraryId", &id)
            }
            libraries::Error::FolderTaken { folder, library } => {
                refuse(R::REFUSAL_REASON_FOLDER_TAKEN, message)
                    .field("folders")
                    .param("folder", &folder)
                    .param("library", &library)
            }
            libraries::Error::Database(err) => return database_failed(err),
        };
        refused.into()
    }
}

/// The Connect error a client gets for a Library, title or Season that Browse didn't find. Database
/// failures are logged, not sent.
impl From<browse::Error> for ConnectError {
    fn from(err: browse::Error) -> Self {
        use RefusalReason as R;
        let message = err.to_string();
        let refused = match err {
            // Every Browse and Watch State call names its Library in `library_id`.
            browse::Error::NoSuchLibrary(id) => {
                refuse(R::REFUSAL_REASON_LIBRARY_NOT_FOUND, message)
                    .field("library_id")
                    .param("libraryId", &id)
            }
            browse::Error::NothingMatched => refuse(R::REFUSAL_REASON_NOTHING_MATCHED, message),
            browse::Error::NoSuchSeason => refuse(R::REFUSAL_REASON_SEASON_NOT_FOUND, message),
            browse::Error::WrongKind { wanted } => {
                let kind = match wanted {
                    Kind::Movies => "movies",
                    Kind::Shows => "shows",
                };
                refuse(R::REFUSAL_REASON_WRONG_LIBRARY_KIND, message)
                    .field("library_id")
                    .param("libraryKind", &kind)
            }
            browse::Error::Database(err) => return database_failed(err),
        };
        refused.into()
    }
}

/// The Connect error a client gets for fixing a Match that failed. Database failures are logged,
/// not sent: they say nothing the client can act on.
impl From<ManualError> for ConnectError {
    fn from(err: ManualError) -> Self {
        use RefusalReason as R;
        let message = err.to_string();
        let refused = match err {
            // Only `SetMatch` and `ClearMatch` name an item.
            ManualError::NoSuchItem(id) => refuse(R::REFUSAL_REASON_ITEM_NOT_FOUND, message)
                .field("item_id")
                .param("itemId", &id),
            ManualError::NoSuchTitle(id) => refuse(R::REFUSAL_REASON_UNKNOWN_TITLE, message)
                .field("title_id")
                .param("titleId", &id),
            ManualError::Unavailable => refuse(R::REFUSAL_REASON_NO_METADATA_PROVIDER, message),
            ManualError::Database(err) => return database_failed(err),
        };
        refused.into()
    }
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
