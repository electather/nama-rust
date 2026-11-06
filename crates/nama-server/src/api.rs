//! The Server's ConnectRPC API (ADR 0007), served on axum.

use std::{
    future,
    net::{IpAddr, SocketAddr},
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
    self as proto, ApproveSignInCodeRequest, ApproveSignInCodeResponse, ClearMatchRequest,
    ClearMatchResponse, CreateLibraryRequest, CreateLibraryResponse, DeleteLibraryRequest,
    DeleteLibraryResponse, EventService, EventType, GetLibraryRequest, GetLibraryResponse,
    GetServerInfoRequest, GetServerInfoResponse, LIBRARY_SERVICE_SERVICE_NAME, LibraryKind,
    LibraryService, ListLibrariesRequest, ListLibrariesResponse, ListUnmatchedItemsRequest,
    ListUnmatchedItemsResponse, LookUpSignInCodeRequest, LookUpSignInCodeResponse,
    MATCH_SERVICE_SERVICE_NAME, MatchService, PollSignInCodeRequest, PollSignInCodeResponse,
    SERVER_SERVICE_GET_SERVER_INFO_SPEC, SIGN_IN_SERVICE_POLL_SIGN_IN_CODE_SPEC,
    SIGN_IN_SERVICE_SETUP_SPEC, SIGN_IN_SERVICE_SIGN_IN_SPEC,
    SIGN_IN_SERVICE_START_SIGN_IN_CODE_SPEC, ScanLibraryRequest, ScanLibraryResponse,
    SearchMetadataRequest, SearchMetadataResponse, ServerService, SessionDelivery, SetMatchRequest,
    SetMatchResponse, SetupRequest, SetupResponse, SignInCodeStatus, SignInRequest, SignInResponse,
    SignInService, SignOutRequest, SignOutResponse, StartSignInCodeRequest,
    StartSignInCodeResponse, SubscribeRequest, SubscribeResponse, UpdateLibraryRequest,
    UpdateLibraryResponse, WhoAmIRequest, WhoAmIResponse, external_id,
};
use tokio::sync::OwnedMutexGuard;

use crate::{
    config::Config,
    contents::{Contents, Item, Unmatched},
    db::Database,
    events::{EventBus, Filter},
    libraries::{self, Kind, Libraries, Library},
    matching::{ManualError, Matcher},
    plugins::Registry,
    rate_limits::{Blocked, SignInLimits},
    scanner::Scanner,
    sessions::{cleared_session_cookie, identify_caller, in_session_cookie, session_cookie},
    sign_in_codes::{self, Poll, SignInCodes},
    users::{self, Caller, Role, SetupCode, SignedIn, Users},
};

/// Routes every RPC the Server answers, each behind the layers that identify the caller and refuse
/// calls they may not make.
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
    connectrpc::Router::new()
        .add_service(Arc::new(ServerServiceHandler {
            name: config.name.clone(),
            users: users.clone(),
        }))
        .add_service(Arc::new(SignInServiceHandler {
            users: users.clone(),
            setup_code,
            limits: SignInLimits::default(),
            sign_in_codes: SignInCodes::default(),
        }))
        .add_service(Arc::new(MatchServiceHandler {
            matcher: Matcher::new(Arc::clone(&db), plugins, events.clone()),
            contents: Contents::new(Arc::clone(&db)),
            libraries: Libraries::new(Arc::clone(&db)),
            scanner: scanner.clone(),
        }))
        .add_service(Arc::new(LibraryServiceHandler {
            libraries: Libraries::new(db),
            scanner,
        }))
        .add_service(Arc::new(EventServiceHandler {
            events,
            users: users.clone(),
        }))
        .into_axum_router()
        .layer(middleware::from_fn(require_access))
        .layer(middleware::from_fn_with_state(users, identify_caller))
}

/// Who may call an RPC.
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

    /// Who may call the RPC at `path`, `/<service>/<method>`. Only the RPCs in
    /// [`Access::PUBLIC`] are public, so a new one needs a session until it is opened up.
    fn of(path: &str) -> Self {
        if Self::PUBLIC.contains(&path) {
            return Self::Anyone;
        }
        let service = path
            .trim_start_matches('/')
            .split_once('/')
            .map(|(service, _)| service);
        match service {
            Some(LIBRARY_SERVICE_SERVICE_NAME | MATCH_SERVICE_SERVICE_NAME) => Self::Admin,
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
    limits: SignInLimits,
    sign_in_codes: SignInCodes,
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
        let attempt = self.limits.admit(peer_ip(&ctx)?, None)?;
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
        self.limits.succeeded(&attempt);
        Ok(hand_out_session(
            signed_in,
            wants_token,
            ctx.headers(),
            |caller, token| SetupResponse {
                user: user_to_proto(caller.user).into(),
                device: device_to_proto(caller.device).into(),
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
        let attempt = self.limits.admit(peer_ip(&ctx)?, Some(&request.name))?;
        let wants_token = wants_token(request.delivery)?;
        let signed_in = self
            .users
            .sign_in(&request.name, &request.password, &request.device_name)
            .await?;
        self.limits.succeeded(&attempt);
        Ok(hand_out_session(
            signed_in,
            wants_token,
            ctx.headers(),
            |caller, token| SignInResponse {
                user: user_to_proto(caller.user).into(),
                device: device_to_proto(caller.device).into(),
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
        let response = Response::new(SignOutResponse::default());
        Ok(if in_session_cookie(ctx.headers()) {
            response.with_header(SET_COOKIE, cleared_session_cookie(ctx.headers()))
        } else {
            response
        })
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
                device: device_to_proto(caller.device.clone()).into(),
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
        _ctx: RequestContext,
        request: ServiceRequest<'_, LookUpSignInCodeRequest>,
    ) -> impl Future<
        Output = ServiceResult<impl Encodable<LookUpSignInCodeResponse> + Send + use<'a>>,
    > + Send {
        let looked_up = self.sign_in_codes.look_up(request.code, Instant::now());
        future::ready(
            looked_up
                .map_err(ConnectError::from)
                .and_then(|device_name| {
                    Response::ok(LookUpSignInCodeResponse {
                        device_name,
                        ..Default::default()
                    })
                }),
        )
    }

    fn approve_sign_in_code<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, ApproveSignInCodeRequest>,
    ) -> impl Future<
        Output = ServiceResult<impl Encodable<ApproveSignInCodeResponse> + Send + use<'a>>,
    > + Send {
        let approved = caller(&ctx).and_then(|caller| {
            Ok(self
                .sign_in_codes
                .approve(request.code, &caller.user, Instant::now())?)
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
                device: device_to_proto(caller.device).into(),
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
        Response::ok(ListUnmatchedItemsResponse {
            items: unmatched.into_iter().map(unmatched_to_proto).collect(),
            ..Default::default()
        })
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

/// A User as clients see them.
fn user_to_proto(user: users::User) -> proto::User {
    proto::User {
        id: user.id,
        name: user.name,
        role: match user.role {
            Role::Admin => proto::Role::ROLE_ADMIN,
            Role::Member => proto::Role::ROLE_MEMBER,
        }
        .into(),
        ..Default::default()
    }
}

/// A Device as clients see it.
fn device_to_proto(device: users::Device) -> proto::Device {
    proto::Device {
        id: device.id,
        name: device.name,
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
            users::Error::AlreadySetUp => Self::failed_precondition(err.to_string()),
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
