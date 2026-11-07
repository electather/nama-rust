//! The Server's ConnectRPC API (ADR 0007), served on axum.

use std::{
    ffi::OsStr,
    future,
    net::{IpAddr, SocketAddr},
    path::Path,
    sync::Arc,
    time::Instant,
};

use axum::{
    extract::{ConnectInfo, Request},
    http::{
        HeaderMap, HeaderValue,
        header::{RETRY_AFTER, SET_COOKIE},
    },
    middleware::{self, Next},
    response::IntoResponse,
};
use buffa::EnumValue;
use connectrpc::{
    ConnectError, Encodable, RequestContext, Response, ServiceRequest, ServiceResult, ServiceStream,
};
use futures_util::StreamExt;
use nama_plugin_api::metadata::{Candidate, ExternalId, SearchQuery, TitleKind};
use nama_proto::nama::v1::{
    self as proto, ApproveSignInCodeRequest, ApproveSignInCodeResponse, BrowseService,
    ClearMatchRequest, ClearMatchResponse, CreateLibraryRequest, CreateLibraryResponse,
    CreateUserRequest, CreateUserResponse, DeleteLibraryRequest, DeleteLibraryResponse,
    DeleteUserRequest, DeleteUserResponse, EventService, EventType, GetLibraryRequest,
    GetLibraryResponse, GetMovieRequest, GetMovieResponse, GetServerInfoRequest,
    GetServerInfoResponse, GetShowRequest, GetShowResponse, LIBRARY_SERVICE_SERVICE_NAME,
    LibraryKind, LibraryService, ListDevicesRequest, ListDevicesResponse, ListEpisodesRequest,
    ListEpisodesResponse, ListLibrariesRequest, ListLibrariesResponse, ListMoviesRequest,
    ListMoviesResponse, ListShowsRequest, ListShowsResponse, ListUnmatchedItemsRequest,
    ListUnmatchedItemsResponse, ListUsersRequest, ListUsersResponse, LookUpSignInCodeRequest,
    LookUpSignInCodeResponse, MATCH_SERVICE_SERVICE_NAME, MatchService, PollSignInCodeRequest,
    PollSignInCodeResponse, ReportProgressRequest, ReportProgressResponse, RevokeDeviceRequest,
    RevokeDeviceResponse, SERVER_SERVICE_GET_SERVER_INFO_SPEC,
    SIGN_IN_SERVICE_POLL_SIGN_IN_CODE_SPEC, SIGN_IN_SERVICE_SETUP_SPEC,
    SIGN_IN_SERVICE_SIGN_IN_SPEC, SIGN_IN_SERVICE_START_SIGN_IN_CODE_SPEC, ScanLibraryRequest,
    ScanLibraryResponse, SearchMetadataRequest, SearchMetadataResponse, ServerService,
    SessionDelivery, SetMatchRequest, SetMatchResponse, SetRoleRequest, SetRoleResponse,
    SetupRequest, SetupResponse, SignInCodeStatus, SignInRequest, SignInResponse, SignInService,
    SignOutRequest, SignOutResponse, StartSignInCodeRequest, StartSignInCodeResponse,
    SubscribeRequest, SubscribeResponse, USER_SERVICE_LIST_DEVICES_SPEC,
    USER_SERVICE_REVOKE_DEVICE_SPEC, USER_SERVICE_SERVICE_NAME, UpdateLibraryRequest,
    UpdateLibraryResponse, UserService, WatchStateService, WhoAmIRequest, WhoAmIResponse,
    external_id,
};
use tokio::sync::OwnedMutexGuard;

use crate::{
    artwork::{self, Artwork},
    config::Config,
    contents::{Contents, Episode, Item, Movie, MovieOf, Show, Unmatched, Version},
    db::Database,
    events::{EventBus, Filter},
    libraries::{self, Kind, Libraries, Library},
    matching::{ManualError, Matcher},
    plugins::Registry,
    rate_limits::{Blocked, SignInCodeLimits, SignInLimits},
    scanner::Scanner,
    sessions::{cleared_session_cookie, identify_caller, in_session_cookie, session_cookie},
    sign_in_codes::{self, Poll, SignInCodes},
    users::{self, Caller, Role, SetupCode, SignedIn, Users},
    watch_states::{Progress, WatchState, WatchStates},
};

/// Routes every RPC the Server answers, and its artwork, each behind the layers that identify the
/// caller and refuse calls they may not make.
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
    let artwork = artwork::router(
        Artwork::new(&config.data_dir, plugins.clone()),
        Contents::new(Arc::clone(&db)),
    );
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
            matcher: Matcher::new(Arc::clone(&db), plugins, events.clone()),
            contents: Contents::new(Arc::clone(&db)),
            libraries: Libraries::new(Arc::clone(&db)),
            scanner: scanner.clone(),
        }))
        .add_service(Arc::new(BrowseServiceHandler {
            libraries: Libraries::new(Arc::clone(&db)),
            contents: Contents::new(Arc::clone(&db)),
            watch_states: WatchStates::new(Arc::clone(&db)),
        }))
        .add_service(Arc::new(WatchStateServiceHandler {
            contents: Contents::new(Arc::clone(&db)),
            watch_states: WatchStates::new(Arc::clone(&db)),
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
        .merge(artwork)
        .layer(middleware::from_fn(require_access))
        .layer(middleware::from_fn_with_state(users, identify_caller))
}

/// Who may call an RPC or fetch media.
enum Access {
    Anyone,
    SignedIn,
    Admin,
}

impl Access {
    /// The RPCs anyone may call without a session, by path.
    const PUBLIC: [&str; 5] = [
        SERVER_SERVICE_GET_SERVER_INFO_SPEC.procedure,
        SIGN_IN_SERVICE_SIGN_IN_SPEC.procedure,
        SIGN_IN_SERVICE_SETUP_SPEC.procedure,
        SIGN_IN_SERVICE_START_SIGN_IN_CODE_SPEC.procedure,
        SIGN_IN_SERVICE_POLL_SIGN_IN_CODE_SPEC.procedure,
    ];

    /// The RPCs of an Admin-only service that any signed-in User may call, by path. Each decides
    /// for itself what a Member may do.
    const SELF_SERVICE: [&str; 2] = [
        USER_SERVICE_LIST_DEVICES_SPEC.procedure,
        USER_SERVICE_REVOKE_DEVICE_SPEC.procedure,
    ];

    /// Who may request `path`: an RPC's `/<service>/<method>`, or media under `/media/`. Only the
    /// RPCs in [`Access::PUBLIC`] are public, so media, and a new RPC until it is opened up, need a
    /// session.
    fn of(path: &str) -> Self {
        if Self::PUBLIC.contains(&path) {
            return Self::Anyone;
        }
        if Self::SELF_SERVICE.contains(&path) {
            return Self::SignedIn;
        }
        let service = path
            .trim_start_matches('/')
            .split_once('/')
            .map(|(service, _)| service);
        match service {
            Some(
                LIBRARY_SERVICE_SERVICE_NAME
                | MATCH_SERVICE_SERVICE_NAME
                | USER_SERVICE_SERVICE_NAME,
            ) => Self::Admin,
            _ => Self::SignedIn,
        }
    }

    /// Why `caller`, or nobody, may not make a call that needs this access, if they may not.
    fn check(&self, caller: Option<&Caller>) -> Result<(), ConnectError> {
        match (self, caller) {
            (Self::Anyone, _) | (Self::SignedIn, Some(_)) => Ok(()),
            (_, None) => Err(ConnectError::unauthenticated("sign in first")),
            (Self::Admin, Some(caller)) if caller.user.role == Role::Admin => Ok(()),
            (Self::Admin, Some(_)) => {
                Err(ConnectError::permission_denied("this needs the Admin Role"))
            }
        }
    }
}

/// Refuses `request` unless its caller, which [`identify_caller`] found, may make it.
async fn require_access(request: Request, next: Next) -> axum::response::Response {
    let caller = request.extensions().get::<Caller>();
    match Access::of(request.uri().path()).check(caller) {
        Ok(()) => next.run(request).await,
        Err(refused) => refused
            .into_http_response(request.headers())
            .into_response(),
    }
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
        let setup_required = !self
            .users
            .any()
            .await
            .map_err(|err| database_failed(&err))?;
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
    /// The recent failures of `Setup` and `SignIn`, which slow guessing down.
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
        let attempt = self.sign_in_limits.admit(peer_ip(&ctx)?, None)?;
        let wants_token = wants_token(request.delivery)?;
        // The Server only made no code because it started with Users. Users are checked before the
        // code, so once there are any every attempt is refused as already set up, as documented.
        let setup_code = self.setup_code.as_ref().ok_or(users::Error::AlreadySetUp)?;
        if self
            .users
            .any()
            .await
            .map_err(|err| database_failed(&err))?
        {
            return Err(users::Error::AlreadySetUp.into());
        }
        if !setup_code.matches(&request.setup_code) {
            return Err(ConnectError::permission_denied("the Setup Code is wrong"));
        }
        let signed_in = self
            .users
            .create_first_admin(&request.name, &request.password, &request.device_name)
            .await?;
        self.sign_in_limits.succeeded(&attempt);
        Ok(hand_out_session(
            signed_in,
            wants_token,
            ctx.headers(),
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
            .admit(peer_ip(&ctx)?, Some(&request.name))?;
        let wants_token = wants_token(request.delivery)?;
        let signed_in = self
            .users
            .sign_in(&request.name, &request.password, &request.device_name)
            .await?;
        self.sign_in_limits.succeeded(&attempt);
        Ok(hand_out_session(
            signed_in,
            wants_token,
            ctx.headers(),
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
        self.users
            .sign_out(device)
            .await
            .map_err(|err| database_failed(&err))?;
        Ok(signed_out(SignOutResponse::default(), ctx.headers()))
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

    fn start_sign_in_code<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, StartSignInCodeRequest>,
    ) -> impl Future<
        Output = ServiceResult<impl Encodable<StartSignInCodeResponse> + Send + use<'a>>,
    > + Send {
        let request = request.to_owned_message();
        let started = wants_token(request.delivery).and_then(|wants_token| {
            let device_name = users::checked_name("a Device's name", &request.device_name)?;
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
        let poll = self
            .sign_in_codes
            .poll(request.handle, Instant::now())
            .ok_or_else(|| ConnectError::not_found("no sign-in is waiting on that handle"))?;
        let (user, device_name, wants_token) = match poll {
            Poll::Waiting => return poll_status(SignInCodeStatus::SIGN_IN_CODE_STATUS_WAITING),
            Poll::Expired => return poll_status(SignInCodeStatus::SIGN_IN_CODE_STATUS_EXPIRED),
            Poll::Approved {
                user,
                device_name,
                wants_token,
            } => (user, device_name, wants_token),
        };
        let signed_in = self.users.sign_in_as(user, device_name).await?;
        Ok(hand_out_session(
            signed_in,
            wants_token,
            ctx.headers(),
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

/// `nama.v1.LibraryService`: managing the Server's Libraries. Admins only, which
/// [`require_access`] enforces.
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
        _ctx: RequestContext,
        _request: ServiceRequest<'_, ListLibrariesRequest>,
    ) -> ServiceResult<ListLibrariesResponse> {
        let libraries = self.libraries.list().await?;
        Response::ok(ListLibrariesResponse {
            libraries: libraries.into_iter().map(library_to_proto).collect(),
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

/// `nama.v1.MatchService`: fixing Matches by hand. Admins only, which [`require_access`] enforces.
struct MatchServiceHandler {
    matcher: Matcher,
    contents: Contents,
    /// Which Libraries there are.
    libraries: Libraries,
    /// Whose scan lock a Match is fixed under, so the scan doesn't change the items meanwhile.
    scanner: Scanner,
}

impl MatchServiceHandler {
    /// Waits for any scan of the Library the item `item` is in, and keeps another from starting
    /// until the returned guard is dropped.
    async fn lock_library_of(&self, item: i64) -> Result<OwnedMutexGuard<()>, ManualError> {
        let library = self
            .contents
            .library_of(item)
            .await?
            .ok_or(ManualError::NoSuchItem(item))?;
        Ok(self.scanner.lock(library).await)
    }
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
            return Err(ConnectError::invalid_argument(
                "the title to search for must not be blank",
            ));
        }
        let query = SearchQuery {
            kind: title_kind_from_proto(request.kind)?,
            title: request.title,
            year: request.year,
        };
        let candidates = self.matcher.search_metadata(&query).await?;
        Response::ok(SearchMetadataResponse {
            candidates: candidates.into_iter().map(candidate_to_proto).collect(),
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
            .ok_or_else(|| ConnectError::invalid_argument("the title to Match to is missing"))?;
        let _scan = self.lock_library_of(request.item_id).await?;
        let summary = self.matcher.set_match(request.item_id, &title).await?;
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
        let _scan = self.lock_library_of(request.item_id).await?;
        let summary = self.matcher.clear_match(request.item_id).await?;
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
            .map_err(|err| database_failed(&err))?;
        let unparsed_paths = self
            .contents
            .unparsed(request.library_id)
            .await
            .map_err(|err| database_failed(&err))?;
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
    libraries: Libraries,
    contents: Contents,
    watch_states: WatchStates,
}

impl BrowseServiceHandler {
    /// The Movies of the Library `library`, which must be a Movies Library.
    async fn movies(&self, library: i64) -> Result<Vec<Movie>, ConnectError> {
        self.require_kind(library, Kind::Movies).await?;
        self.contents
            .movies(library)
            .await
            .map_err(|err| database_failed(&err))
    }

    /// `movies` as the caller of `ctx` sees them, each with their Watch State.
    async fn movies_to_proto(
        &self,
        ctx: &RequestContext,
        movies: Vec<Movie>,
    ) -> Result<Vec<proto::Movie>, ConnectError> {
        let ids: Vec<&ExternalId> = movies.iter().map(|movie| &movie.id).collect();
        let states = self
            .watch_states
            .of_movies(caller(ctx)?.user.id, &ids)
            .await
            .map_err(|err| database_failed(&err))?;
        Ok(movies
            .into_iter()
            .zip(states)
            .map(|(movie, state)| movie_to_proto(movie, state))
            .collect())
    }

    /// The Shows of the Library `library`, which must be a Shows Library.
    // ponytail: reads every Episode and Version, even to list Shows or find one; query less if
    // Libraries get big.
    async fn shows(&self, library: i64) -> Result<Vec<Show>, ConnectError> {
        self.require_kind(library, Kind::Shows).await?;
        self.contents
            .shows(library)
            .await
            .map_err(|err| database_failed(&err))
    }

    /// The Show of the Shows Library `library` with the id `id`.
    async fn show(
        &self,
        library: i64,
        id: Option<proto::ExternalId>,
    ) -> Result<Show, ConnectError> {
        let id = required_id(id, "Show")?;
        self.shows(library)
            .await?
            .into_iter()
            .find(|show| show.id == id)
            .ok_or_else(|| ConnectError::not_found("nothing in the Library is Matched to that id"))
    }

    /// Fails with `not_found` if there is no Library `library`, and with `invalid_argument` if it
    /// doesn't hold `kind`.
    async fn require_kind(&self, library: i64, kind: Kind) -> Result<(), ConnectError> {
        if self.libraries.get(library).await?.kind == kind {
            return Ok(());
        }
        Err(ConnectError::invalid_argument(match kind {
            Kind::Movies => "the Library isn't a Movies Library",
            Kind::Shows => "the Library isn't a Shows Library",
        }))
    }
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
        let movies = self.movies(request.library_id).await?;
        Response::ok(ListMoviesResponse {
            movies: self.movies_to_proto(&ctx, movies).await?,
            ..Default::default()
        })
    }

    async fn get_movie(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, GetMovieRequest>,
    ) -> ServiceResult<GetMovieResponse> {
        let request = request.to_owned_message();
        let id = required_id(request.movie_id.into_option(), "Movie")?;
        // ponytail: reads the whole Library to find one Movie; query by id if Libraries get big.
        let movie = self
            .movies(request.library_id)
            .await?
            .into_iter()
            .find(|movie| movie.id == id)
            .ok_or_else(|| {
                ConnectError::not_found("nothing in the Library is Matched to that id")
            })?;
        let movie = self.movies_to_proto(&ctx, vec![movie]).await?.pop();
        Response::ok(GetMovieResponse {
            movie: movie.into(),
            ..Default::default()
        })
    }

    async fn list_shows(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, ListShowsRequest>,
    ) -> ServiceResult<ListShowsResponse> {
        let shows = self.shows(request.library_id).await?;
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
        let show = self
            .show(request.library_id, request.show_id.into_option())
            .await?;
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
        _ctx: RequestContext,
        request: ServiceRequest<'_, ListEpisodesRequest>,
    ) -> ServiceResult<ListEpisodesResponse> {
        let request = request.to_owned_message();
        let number = request.season_number;
        let mut show = self
            .show(request.library_id, request.show_id.into_option())
            .await?;
        let season = show
            .seasons
            .remove(&i64::from(number))
            .ok_or_else(|| ConnectError::not_found("the Show has no such Season in the Library"))?;
        Response::ok(ListEpisodesResponse {
            episodes: season
                .episodes
                .into_iter()
                .map(|episode| episode_to_proto(number, episode))
                .collect(),
            ..Default::default()
        })
    }
}

/// `nama.v1.WatchStateService`: each signed-in User's progress through what they play.
struct WatchStateServiceHandler {
    contents: Contents,
    watch_states: WatchStates,
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
                ConnectError::invalid_argument(
                    "the duration must be more than zero and less than 2^63 milliseconds",
                )
            })?;
        let movie_of = self.contents.movie_of(request.version_id).await;
        match movie_of.map_err(|err| database_failed(&err))? {
            MovieOf::NoSuchVersion => {
                return Err(ConnectError::not_found("there is no such Version"));
            }
            MovieOf::NoMatchedMovie => {}
            MovieOf::Movie(movie) => self
                .watch_states
                .report_movie(user, &movie, progress)
                .await
                .map_err(|err| database_failed(&err))?,
        }
        Response::ok(ReportProgressResponse::default())
    }
}

/// `nama.v1.UserService`: managing the Server's Users and their Devices. Admins only, which
/// [`require_access`] enforces, but for the RPCs in [`Access::SELF_SERVICE`], which check for
/// themselves with [`devices_owner`].
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
        let role = role_from_proto(request.role)?;
        let id = self
            .users
            .create(&request.name, &request.password, role)
            .await?;
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
        let users = self
            .users
            .list()
            .await
            .map_err(|err| database_failed(&err))?;
        Response::ok(ListUsersResponse {
            users: users.into_iter().map(user_to_proto).collect(),
            ..Default::default()
        })
    }

    async fn set_role(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, SetRoleRequest>,
    ) -> ServiceResult<SetRoleResponse> {
        let request = request.to_owned_message();
        let role = role_from_proto(request.role)?;
        self.users.set_role(request.user_id, role).await?;
        Response::ok(SetRoleResponse::default())
    }

    async fn delete_user(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, DeleteUserRequest>,
    ) -> ServiceResult<DeleteUserResponse> {
        let caller = caller(&ctx)?;
        self.users.delete(request.user_id).await?;
        let response = DeleteUserResponse::default();
        Ok(if request.user_id == caller.user.id {
            signed_out(response, ctx.headers())
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
            .map_err(|err| database_failed(&err))?
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
            .map_err(|err| database_failed(&err))?;
        if !revoked {
            return Err(ConnectError::not_found("that User has no such Device"));
        }
        let response = RevokeDeviceResponse::default();
        Ok(if request.device_id == caller.device.id {
            signed_out(response, ctx.headers())
        } else {
            Response::new(response)
        })
    }
}

/// The User whose Devices `caller` asks about with `user_id`: their own if it's unset. Only an
/// Admin may ask about another User's. A Member who does gets `not_found`, as for a User who
/// doesn't exist, so they can't learn which IDs do.
fn devices_owner(caller: &Caller, user_id: Option<i64>) -> Result<i64, ConnectError> {
    match user_id {
        None => Ok(caller.user.id),
        Some(id) if id == caller.user.id || caller.user.role == Role::Admin => Ok(id),
        Some(_) => Err(no_such_user()),
    }
}

/// The Connect error for a User ID that names nobody the caller may see.
fn no_such_user() -> ConnectError {
    ConnectError::not_found("no User has that ID")
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
        let signed_out = caller(&ctx).map(|caller| self.users.signed_out(caller.device.id));
        let filter = request
            .types
            .into_iter()
            .map(event_type_from_proto)
            .collect::<Result<_, _>>()
            .map(|types| Filter {
                types,
                libraries: request.library_ids,
            });
        let subscribed = signed_out.and_then(|signed_out| {
            let events = self.events.subscribe(filter?).take_until(signed_out);
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
            Err(ConnectError::invalid_argument(
                "the session delivery must be cookie or token",
            ))
        }
    }
}

/// Who sent the request `ctx` is for, or `unauthenticated` if it carries no valid session.
fn caller(ctx: &RequestContext) -> Result<&Caller, ConnectError> {
    ctx.extensions()
        .get::<Caller>()
        .ok_or_else(|| ConnectError::unauthenticated("sign in first"))
}

/// The response to a request whose own Device was just signed out, carrying `body`. A cookie
/// session has its cookie cleared.
fn signed_out<M>(body: M, request_headers: &HeaderMap) -> Response<M> {
    let response = Response::new(body);
    if in_session_cookie(request_headers) {
        response.with_header(SET_COOKIE, cleared_session_cookie(request_headers))
    } else {
        response
    }
}

/// The response `body` makes for a Device just `signed_in`, with its session token in the body for
/// a client that `wants_token`, or else in a cookie and not in the body.
fn hand_out_session<M>(
    signed_in: SignedIn,
    wants_token: bool,
    request_headers: &HeaderMap,
    body: impl FnOnce(Caller, String) -> M,
) -> Response<M> {
    let SignedIn { caller, token } = signed_in;
    if wants_token {
        Response::new(body(caller, token))
    } else {
        let cookie = session_cookie(&token, request_headers);
        Response::new(body(caller, String::new())).with_header(SET_COOKIE, cookie)
    }
}

/// An event type a filter names, which must be one the Server knows.
fn event_type_from_proto(kind: EnumValue<EventType>) -> Result<EventType, ConnectError> {
    match kind {
        EnumValue::Known(EventType::EVENT_TYPE_UNSPECIFIED) | EnumValue::Unknown(_) => Err(
            ConnectError::invalid_argument("an event type must be one this Server knows"),
        ),
        EnumValue::Known(kind) => Ok(kind),
    }
}

/// The kind of title a request asks for, which must be one the Server knows.
fn title_kind_from_proto(kind: EnumValue<proto::TitleKind>) -> Result<TitleKind, ConnectError> {
    match kind {
        EnumValue::Known(proto::TitleKind::TITLE_KIND_MOVIE) => Ok(TitleKind::Movie),
        EnumValue::Known(proto::TitleKind::TITLE_KIND_SHOW) => Ok(TitleKind::Show),
        EnumValue::Known(proto::TitleKind::TITLE_KIND_UNSPECIFIED) | EnumValue::Unknown(_) => Err(
            ConnectError::invalid_argument("the kind of title must be movie or show"),
        ),
    }
}

/// A search result as clients see it.
fn candidate_to_proto(candidate: Candidate) -> proto::TitleCandidate {
    proto::TitleCandidate {
        id: external_id_to_proto(candidate.id).into(),
        title: candidate.title,
        original_title: candidate.original_title,
        year: candidate.year,
        ..Default::default()
    }
}

/// An Item that needs a Match as clients see it.
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

/// A Watch State as clients see it.
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

/// An Episode of Season `season` as clients see it.
fn episode_to_proto(season: u32, episode: Episode) -> proto::Episode {
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

/// `n` as a proto `int32`, or `None` if it doesn't fit, which no name a scan parses gives.
fn int32(n: i64) -> Option<i32> {
    i32::try_from(n).ok()
}

/// An id of a title as clients send it.
fn external_id_from_proto(id: external_id::Id) -> ExternalId {
    match id {
        external_id::Id::Tmdb(id) => ExternalId::Tmdb(id),
        external_id::Id::Imdb(id) => ExternalId::Imdb(id),
        external_id::Id::Tvdb(id) => ExternalId::Tvdb(id),
    }
}

/// The id of the title of kind `what` that a request names, which it must name.
fn required_id(id: Option<proto::ExternalId>, what: &str) -> Result<ExternalId, ConnectError> {
    id.and_then(|id| id.id)
        .map(external_id_from_proto)
        .ok_or_else(|| ConnectError::invalid_argument(format!("the {what}'s id is missing")))
}

/// An id of a title as clients see it.
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

/// The kind a request asks for, which must be one the Server knows.
fn kind_from_proto(kind: EnumValue<LibraryKind>) -> Result<Kind, ConnectError> {
    match kind {
        EnumValue::Known(LibraryKind::LIBRARY_KIND_MOVIES) => Ok(Kind::Movies),
        EnumValue::Known(LibraryKind::LIBRARY_KIND_SHOWS) => Ok(Kind::Shows),
        EnumValue::Known(LibraryKind::LIBRARY_KIND_UNSPECIFIED) | EnumValue::Unknown(_) => Err(
            ConnectError::invalid_argument("a Library's kind must be movies or shows"),
        ),
    }
}

/// A Library as clients see it.
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

/// The Role a request asks for, which must be one the Server knows.
fn role_from_proto(role: EnumValue<proto::Role>) -> Result<Role, ConnectError> {
    match role {
        EnumValue::Known(proto::Role::ROLE_ADMIN) => Ok(Role::Admin),
        EnumValue::Known(proto::Role::ROLE_MEMBER) => Ok(Role::Member),
        EnumValue::Known(proto::Role::ROLE_UNSPECIFIED) | EnumValue::Unknown(_) => Err(
            ConnectError::invalid_argument("a User's Role must be admin or member"),
        ),
    }
}

/// A Role as clients see it.
fn role_to_proto(role: Role) -> proto::Role {
    match role {
        Role::Admin => proto::Role::ROLE_ADMIN,
        Role::Member => proto::Role::ROLE_MEMBER,
    }
}

/// A User as clients see them.
fn user_to_proto(user: users::User) -> proto::User {
    proto::User {
        id: user.id,
        name: user.name,
        role: role_to_proto(user.role).into(),
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

/// The Connect error a client gets when the database failed. The failure is logged, not sent: it
/// says nothing the client can act on.
fn database_failed(err: &sqlx::Error) -> ConnectError {
    tracing::error!("a request failed: {err}");
    ConnectError::internal("the Server's database failed")
}

/// The Connect error a client gets for a change to the Users that failed. Server-side failures
/// are logged, not sent.
impl From<users::Error> for ConnectError {
    fn from(err: users::Error) -> Self {
        match err {
            users::Error::Invalid(_) => Self::invalid_argument(err.to_string()),
            users::Error::NameTaken => Self::already_exists(err.to_string()),
            users::Error::NoSuchUser => Self::not_found(err.to_string()),
            users::Error::AlreadySetUp | users::Error::LastAdmin => {
                Self::failed_precondition(err.to_string())
            }
            users::Error::WrongNameOrPassword => Self::unauthenticated(err.to_string()),
            users::Error::Database(err) => database_failed(&err),
            users::Error::NoAuthenticator
            | users::Error::Authenticator(_)
            | users::Error::Random(_) => {
                tracing::error!("a change to the Users failed: {err}");
                Self::internal("the Server failed")
            }
        }
    }
}

/// The IP address `ctx` came from, which rate limits count by. Forwarded-for headers are ignored:
/// any client can set them to dodge its limit.
fn peer_ip(ctx: &RequestContext) -> Result<IpAddr, ConnectError> {
    let peer = ctx.extensions().get::<ConnectInfo<SocketAddr>>();
    peer.map(|ConnectInfo(addr)| addr.ip()).ok_or_else(|| {
        tracing::error!("a request came without its peer's address");
        ConnectError::internal("the Server failed")
    })
}

/// The Connect error a client gets for a rate-limited attempt, with the delay to wait in
/// `Retry-After`.
impl From<Blocked> for ConnectError {
    fn from(blocked: Blocked) -> Self {
        let retry_after = HeaderValue::from(blocked.retry_after_secs());
        let headers = HeaderMap::from_iter([(RETRY_AFTER, retry_after)]);
        Self::resource_exhausted(blocked.to_string()).with_headers(headers)
    }
}

/// The Connect error a client gets for a Sign-in Code that couldn't be started, looked up or
/// approved. A random number generator that failed is logged, not sent.
impl From<sign_in_codes::Error> for ConnectError {
    fn from(err: sign_in_codes::Error) -> Self {
        match err {
            sign_in_codes::Error::NotFound => Self::not_found(err.to_string()),
            sign_in_codes::Error::AlreadyApproved => Self::failed_precondition(err.to_string()),
            sign_in_codes::Error::TooManyPending => Self::resource_exhausted(err.to_string()),
            sign_in_codes::Error::Random(_) => {
                tracing::error!("starting a Sign-in Code failed: {err}");
                Self::internal("the Server failed")
            }
        }
    }
}

/// The Connect error a client gets for a Library request that failed. Database failures are
/// logged, not sent: they say nothing the client can act on.
impl From<libraries::Error> for ConnectError {
    fn from(err: libraries::Error) -> Self {
        match err {
            libraries::Error::Invalid(_) => Self::invalid_argument(err.to_string()),
            libraries::Error::NotFound(_) => Self::not_found(err.to_string()),
            libraries::Error::FolderTaken { .. } => Self::already_exists(err.to_string()),
            libraries::Error::Database(_) => {
                tracing::error!("a Library request failed: {err}");
                Self::internal("the Server's database failed")
            }
        }
    }
}

/// The Connect error a client gets for fixing a Match that failed. Database failures are logged,
/// not sent: they say nothing the client can act on.
impl From<ManualError> for ConnectError {
    fn from(err: ManualError) -> Self {
        match err {
            ManualError::NoSuchItem(_) => Self::not_found(err.to_string()),
            ManualError::NoSuchTitle(_) => Self::invalid_argument(err.to_string()),
            ManualError::Unavailable => Self::unavailable(err.to_string()),
            ManualError::Database(_) => {
                tracing::error!("fixing a Match failed: {err}");
                Self::internal("the Server's database failed")
            }
        }
    }
}
