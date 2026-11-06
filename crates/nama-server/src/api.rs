//! The Server's ConnectRPC API (ADR 0007), served on axum.

use std::{future, sync::Arc};

use axum::http::{HeaderMap, header::SET_COOKIE};
use buffa::EnumValue;
use connectrpc::{
    ConnectError, Encodable, RequestContext, Response, ServiceRequest, ServiceResult, ServiceStream,
};
use futures_util::StreamExt;
use nama_plugin_api::metadata::{Candidate, ExternalId, SearchQuery, TitleKind};
use nama_proto::nama::v1::{
    self as proto, ClearMatchRequest, ClearMatchResponse, CreateLibraryRequest,
    CreateLibraryResponse, DeleteLibraryRequest, DeleteLibraryResponse, EventService, EventType,
    GetLibraryRequest, GetLibraryResponse, GetServerInfoRequest, GetServerInfoResponse,
    LibraryKind, LibraryService, ListLibrariesRequest, ListLibrariesResponse, MatchService,
    ScanLibraryRequest, ScanLibraryResponse, SearchMetadataRequest, SearchMetadataResponse,
    ServerService, SessionDelivery, SetMatchRequest, SetMatchResponse, SetupRequest, SetupResponse,
    SignInRequest, SignInResponse, SignInService, SignOutRequest, SignOutResponse,
    SubscribeRequest, SubscribeResponse, UpdateLibraryRequest, UpdateLibraryResponse,
    WhoAmIRequest, WhoAmIResponse, external_id,
};
use tokio::sync::OwnedMutexGuard;

use crate::{
    config::Config,
    contents::Contents,
    db::Database,
    events::{EventBus, Filter},
    libraries::{self, Kind, Libraries, Library},
    matching::{ManualError, Matcher},
    plugins::Registry,
    scanner::Scanner,
    sessions::{cleared_session_cookie, identify_caller, session_cookie},
    users::{self, Caller, Role, SetupCode, SignedIn, Users},
};

/// Routes every RPC the Server answers, each behind the layer that identifies the caller.
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
        }))
        .add_service(Arc::new(MatchServiceHandler {
            matcher: Matcher::new(Arc::clone(&db), plugins, events.clone()),
            contents: Contents::new(Arc::clone(&db)),
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
        .layer(axum::middleware::from_fn_with_state(users, identify_caller))
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
        let wants_token = wants_token(request.delivery)?;
        let signed_in = self
            .users
            .sign_in(&request.name, &request.password, &request.device_name)
            .await?;
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
        // ponytail: cleared even for a bearer session, where a client has no cookie to clear.
        Ok(Response::new(SignOutResponse::default())
            .with_header(SET_COOKIE, cleared_session_cookie(ctx.headers())))
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
}

/// `nama.v1.LibraryService`: managing the Server's Libraries.
//
// TODO(M2): managing Libraries is for admins only. There are no Users yet, so every caller is
// let through; once there are, check here that the caller's Role is Admin.
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

/// `nama.v1.MatchService`: fixing Matches by hand.
//
// TODO(M2): fixing Matches is for admins only. There are no Users yet, so every caller is let
// through; once there are, check here that the caller's Role is Admin.
struct MatchServiceHandler {
    matcher: Matcher,
    contents: Contents,
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
}

/// `nama.v1.EventService`: live updates.
//
// TODO(M2): once there are Users, only send a caller events about Libraries it may see.
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
        let signed_out = ctx
            .extensions()
            .get::<Caller>()
            .map(|caller| self.users.signed_out(caller.device.id));
        let filter = request
            .types
            .into_iter()
            .map(event_type_from_proto)
            .collect::<Result<_, _>>()
            .map(|types| Filter {
                types,
                libraries: request.library_ids,
            });
        let subscribed = filter.and_then(|filter| {
            let events = self.events.subscribe(filter).take_until(async move {
                match signed_out {
                    Some(signed_out) => signed_out.await,
                    None => future::pending().await,
                }
            });
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
