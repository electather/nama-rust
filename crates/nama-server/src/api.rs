//! The Server's ConnectRPC API (ADR 0007), served on axum.

use std::{future, net::IpAddr, sync::Arc};

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
use nama_plugin_api::metadata::{ExternalId, TitleKind};
use nama_proto::nama::v1::{
    self as proto, EventService, EventType, Permission, Refusal, RefusalReason, SubscribeRequest,
    SubscribeResponse, external_id,
};

use crate::{
    artwork::{self, Artwork},
    browse::Browse,
    config::Config,
    connections::Connections,
    contents::Contents,
    db::{Database, now},
    events::{EventBus, Filter},
    ffmpeg::Ffmpeg,
    forwarded::{RequestSource, identify_source},
    libraries::Libraries,
    matching::Matcher,
    playback::running::RunningPlans,
    plugin_settings::PluginSettings,
    rate_limits::{SignInCodeLimits, SignInLimits},
    requests::Requests,
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
mod requests;
mod server;
mod sign_in;
mod users;
mod watch_states;

use browse::BrowseServiceHandler;
use libraries::LibraryServiceHandler;
use matches::MatchServiceHandler;
use playback::PlaybackServiceHandler;
use plugins::PluginServiceHandler;
use requests::RequestServiceHandler;
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
                requester: crate::requests::visible_to(&caller.user),
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
