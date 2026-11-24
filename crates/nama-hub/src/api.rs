//! The Hub API: the ConnectRPC services in `nama.hub.v1`. Proto types stop here: areas take and
//! return their own, and this converts (ADR 0099).

mod accounts;
mod links;
mod track_preferences;
mod viewing;
mod watch_states;
mod watchlists;

use std::{sync::Arc, time::Instant};

use axum::{
    Extension,
    body::Bytes,
    extract::{Request, State},
    http::{StatusCode, header},
    middleware,
    middleware::Next,
    response::{IntoResponse as _, Response},
    routing::{get, post},
};
use buffa::MessageName as _;
use connectrpc::{
    ConnectError, ErrorCode, ErrorDetail, Interceptor, PayloadStream, RequestContext,
    ServiceRequest, ServiceResult,
    interceptor::{self, StreamRequest, StreamResponse, UnaryRequest, UnaryResponse},
};
use nama_forwarded::{IpRange, identify_source};
use nama_hub_proto::nama::hub::v1::{
    ACCOUNT_SERVICE_SERVICE_NAME, GetHubInfoRequest, GetHubInfoResponse, HUB_SERVICE_SERVICE_NAME,
    HubService, LINK_SERVICE_SERVICE_NAME, LINKED_SERVER_SERVICE_SERVICE_NAME, Refusal,
    RefusalReason, SERVER_LINK_SERVICE_SERVICE_NAME, VIEWING_EVENT_SERVICE_SERVICE_NAME,
};
use sqlx::PgPool;
use tracing::Instrument as _;
use url::Url;

use crate::{
    accounts::{
        AccountSession, Accounts, RECENT_AUTHENTICATION_MINS, RecentAuthenticationRequired,
    },
    db,
    diagnostics::{Backend, NotTaken, Shared, Signal},
    dns::ServerNames,
    links::{LinkedServer, Links},
    mail::Mail,
    recommendations::Recommendations,
    signing::Keys,
    sync::{TrackPreferences, WatchStates, Watchlists},
    viewing::Viewing,
};

use self::{
    accounts::{AccountServiceHandler, COOKIE_NAME, signed_in},
    links::{LinkServiceHandler, LinkedServerServiceHandler, ServerLinkServiceHandler},
    viewing::ViewingEventServiceHandler,
};

/// The Hub's routes: every service, each call in a span of its own, knowing where it came from
/// through `trusted_proxies` (ADR 0018) and which Account or Linked Server, if any, sent it, the
/// public half of `keys` at `/.well-known/jwks.json`, and OTLP/HTTP's `/otlp/v1/traces` and
/// `/otlp/v1/logs` taking what Linked Servers share for `diagnostics`. Sign-in Links go out
/// through `mail`, if it is set up, and Linked Servers are named through `names`, if it is.
/// Passkeys are for the Hub at `public_url`.
#[expect(
    clippy::too_many_arguments,
    reason = "each is a separate handle the Hub is built from, as `serve`'s are"
)]
pub(crate) fn router(
    db: PgPool,
    keys: Arc<Keys>,
    trusted_proxies: &[IpRange],
    mail: Option<Mail>,
    names: Option<ServerNames>,
    diagnostics: Option<Arc<dyn Backend>>,
    public_url: Option<&Url>,
) -> axum::Router {
    let accounts = Accounts::new(db.clone(), mail, public_url);
    let links = Links::new(db.clone(), accounts.clone(), Arc::clone(&keys), names);
    let rpcs = services(db, &accounts, &links)
        .into_axum_service()
        .with_interceptor(RefuseUnserved);
    let jwks = async move || {
        let jwks = keys.jwks().to_owned();
        ([(header::CONTENT_TYPE, "application/json")], jwks)
    };
    let shared = Shared::new(diagnostics);
    axum::Router::new()
        .route("/.well-known/jwks.json", get(jwks))
        .route(
            "/otlp/v1/traces",
            post(async |state, server, batch| share(Signal::Traces, state, server, batch).await),
        )
        .route(
            "/otlp/v1/logs",
            post(async |state, server, batch| share(Signal::Logs, state, server, batch).await),
        )
        .with_state(shared)
        .fallback_service(rpcs)
        .layer(middleware::from_fn_with_state(
            Callers { accounts, links },
            identify_caller,
        ))
        .layer(middleware::from_fn_with_state(
            Arc::from(trusted_proxies),
            identify_source,
        ))
        .layer(middleware::from_fn(trace_call))
}

/// Every Hub service, on `db`, asking `accounts` and `links`.
fn services(db: PgPool, accounts: &Accounts, links: &Links) -> connectrpc::Router {
    let viewing = Viewing::new(db.clone());
    let watch_states = WatchStates::new(db.clone());
    let recommendations = Recommendations::new(db.clone(), viewing.clone());
    let track_preferences = TrackPreferences::new(db.clone());
    let watchlists = Watchlists::new(db.clone(), viewing.clone());
    connectrpc::Router::new()
        .add_service(Arc::new(HubServiceHandler {
            db: db.clone(),
            sends_sign_in_links: accounts.sends_sign_in_links(),
            offers_passkeys: accounts.offers_passkeys(),
        }))
        .add_service(Arc::new(AccountServiceHandler {
            accounts: accounts.clone(),
            links: links.clone(),
        }))
        .add_service(Arc::new(ServerLinkServiceHandler {
            links: links.clone(),
        }))
        .add_service(Arc::new(LinkServiceHandler {
            links: links.clone(),
        }))
        .add_service(Arc::new(LinkedServerServiceHandler {
            links: links.clone(),
            watch_states,
            track_preferences,
            watchlists,
            viewing: viewing.clone(),
            recommendations,
            db,
        }))
        .add_service(Arc::new(ViewingEventServiceHandler { viewing }))
}

/// Takes a Linked Server's batch of `signal`, as OTLP/HTTP protobuf, and forwards it unchanged
/// (ADR 0054). OTLP/HTTP rather than a ConnectRPC method, so the Hub passes on what the Server's
/// exporter wrote byte for byte. Anyone else gets 401, a Server that shared too much lately 429,
/// and a failing backend 502.
async fn share(
    signal: Signal,
    State(shared): State<Shared>,
    server: Option<Extension<LinkedServer>>,
    batch: Bytes,
) -> StatusCode {
    let Some(Extension(server)) = server else {
        return StatusCode::UNAUTHORIZED;
    };
    match shared.take(server.id, signal, batch, Instant::now()).await {
        Ok(()) => StatusCode::OK,
        Err(NotTaken::TooMuch) => StatusCode::TOO_MANY_REQUESTS,
        Err(NotTaken::Backend(err)) => {
            tracing::warn!(server = %server.id, "{err}");
            StatusCode::BAD_GATEWAY
        }
    }
}

/// `nama.hub.v1.HubService`: facts about the Hub, for anyone.
struct HubServiceHandler {
    db: PgPool,
    sends_sign_in_links: bool,
    offers_passkeys: bool,
}

#[expect(
    refining_impl_trait,
    reason = "plain `async fn` returning the owned response is the form the generated trait \
              documents; the refined signature isn't visible outside this crate"
)]
impl HubService for HubServiceHandler {
    async fn get_hub_info(
        &self,
        _ctx: RequestContext,
        _request: ServiceRequest<'_, GetHubInfoRequest>,
    ) -> ServiceResult<GetHubInfoResponse> {
        db::ping(&self.db).await.map_err(|err| {
            tracing::error!("the Hub can't reach its database: {err}");
            ConnectError::unavailable("the Hub can't reach its database")
        })?;
        connectrpc::Response::ok(GetHubInfoResponse {
            version: env!("CARGO_PKG_VERSION").to_owned(),
            sign_in_links: self.sends_sign_in_links,
            passkeys: self.offers_passkeys,
            ..Default::default()
        })
    }
}

/// What the auth layer looks callers up in.
#[derive(Clone)]
struct Callers {
    accounts: Accounts,
    links: Links,
}

/// The Hub's one auth layer: adds to the request's extensions the `AccountSession` its session
/// cookie names, recording the session's use, or the `LinkedServer` its `Authorization: Bearer`
/// credential names. A request with neither is anonymous, and gets none.
async fn identify_caller(
    State(callers): State<Callers>,
    mut request: Request,
    next: Next,
) -> Response {
    let headers = request.headers();
    let session_token = headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|cookies| cookies.split(';'))
        .find_map(|cookie| cookie.trim().strip_prefix(COOKIE_NAME)?.strip_prefix('='))
        .map(str::to_owned);
    let credential = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok()?.strip_prefix("Bearer "))
        .map(str::to_owned);
    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let found = match (session_token, credential) {
        (Some(token), _) => used_session(&callers.accounts, &token, user_agent.as_deref())
            .await
            .map(|session| {
                session.map(|session| request.extensions_mut().insert(session));
            }),
        (None, Some(credential)) => callers.links.server(&credential).await.map(|server| {
            server.map(|server| request.extensions_mut().insert(server));
        }),
        (None, None) => Ok(()),
    };
    match found {
        Ok(()) => next.run(request).await,
        Err(err) => {
            tracing::error!("could not look up a caller: {err}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// The session `token` names, if any, recording that a browser sending `user_agent` used it now.
async fn used_session(
    accounts: &Accounts,
    token: &str,
    user_agent: Option<&str>,
) -> Result<Option<AccountSession>, sqlx::Error> {
    let Some(session) = accounts.session(token).await? else {
        return Ok(None);
    };
    // Only the Account page's list reads it, so a failed write mustn't refuse the request.
    if let Err(err) = accounts.record_use(&session, user_agent).await {
        tracing::warn!("could not record a session's use: {err}");
    }
    Ok(Some(session))
}

/// The caller a Hub service serves.
#[derive(Clone, Copy)]
enum Serves {
    /// Anyone, signed in or not.
    Anyone,
    /// A Linked Server, by its credential.
    LinkedServer,
    /// An Account, by its session.
    AccountSession,
}

/// Who each Hub service serves (`AGENTS.md`, "Callers").
const SERVES: [(&str, Serves); 6] = [
    (HUB_SERVICE_SERVICE_NAME, Serves::Anyone),
    // Its methods mix callers signed in with callers signing in, so its signed-in ones check.
    (ACCOUNT_SERVICE_SERVICE_NAME, Serves::Anyone),
    (SERVER_LINK_SERVICE_SERVICE_NAME, Serves::Anyone),
    (LINK_SERVICE_SERVICE_NAME, Serves::AccountSession),
    (VIEWING_EVENT_SERVICE_SERVICE_NAME, Serves::AccountSession),
    (LINKED_SERVER_SERVICE_SERVICE_NAME, Serves::LinkedServer),
];

/// The auth layer's refusal, after [`identify_caller`], of a caller the service it calls doesn't
/// serve, before the request is decoded: who calls is the first rule a request can break (ADR
/// 0019).
struct RefuseUnserved;

#[connectrpc::async_trait]
impl Interceptor for RefuseUnserved {
    async fn intercept_unary(
        &self,
        req: UnaryRequest,
        next: interceptor::Next<'_>,
    ) -> Result<UnaryResponse, ConnectError> {
        admit(&req.ctx)?;
        next.run(req).await
    }

    async fn intercept_streaming(
        &self,
        req: StreamRequest,
        inbound: PayloadStream,
        next: interceptor::NextStream<'_>,
    ) -> Result<StreamResponse, ConnectError> {
        admit(&req.ctx)?;
        next.run(req, inbound).await
    }
}

/// Refuses the request `ctx` is for unless its service serves its caller.
fn admit(ctx: &RequestContext) -> Result<(), ConnectError> {
    let path = ctx.path().unwrap_or_default();
    match service_and_method(path).and_then(|(service, _)| serves(service)) {
        Some(Serves::Anyone) => Ok(()),
        Some(Serves::LinkedServer) if ctx.extensions().get::<LinkedServer>().is_some() => Ok(()),
        Some(Serves::LinkedServer) => Err(refuse(
            RefusalReason::REFUSAL_REASON_SERVER_NOT_LINKED,
            "send a Linked Server's credential",
        )
        .into()),
        Some(Serves::AccountSession) => signed_in(ctx).map(|_session| ()),
        None => Err(hub_failed(&format_args!("{path} isn't in SERVES"))),
    }
}

/// Who `service` serves, or `None` for a service missing from [`SERVES`], which serves no one.
fn serves(service: &str) -> Option<Serves> {
    SERVES
        .iter()
        .find(|&&(name, _)| name == service)
        .map(|&(_, serves)| serves)
}

/// A refusal of a client's request on its way to becoming a [`ConnectError`]: [`refuse`] starts
/// one, [`Refused::field`] and [`Refused::param`] fill it in.
struct Refused {
    code: ErrorCode,
    refusal: Refusal,
    message: String,
}

/// Refuses a request for breaking the rule `reason` names, telling the client `message` (ADR
/// 0019).
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

impl Refused {
    /// Names the request field the refusal is about.
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
    fn from(
        Refused {
            code,
            refusal,
            message,
        }: Refused,
    ) -> Self {
        let detail = ErrorDetail {
            // A Refusal is plain strings and an enum, so it always serializes.
            debug: serde_json::to_value(&refusal).ok(),
            ..ErrorDetail::from_message(Refusal::FULL_NAME, &refusal)
        };
        Self::new(code, message).with_detail(detail)
    }
}

/// The Connect code `reason` always comes with: API contract, as the reasons are (ADR 0019).
const fn refusal_code(reason: RefusalReason) -> ErrorCode {
    use RefusalReason as R;
    match reason {
        R::REFUSAL_REASON_SIGN_IN_REQUIRED
        | R::REFUSAL_REASON_WRONG_EMAIL_OR_PASSWORD
        | R::REFUSAL_REASON_SERVER_NOT_LINKED
        | R::REFUSAL_REASON_PASSKEY_NOT_ACCEPTED => ErrorCode::Unauthenticated,
        R::REFUSAL_REASON_RECENT_AUTHENTICATION_REQUIRED
        | R::REFUSAL_REASON_EMAIL_NOT_PROVEN
        | R::REFUSAL_REASON_SIGN_IN_LINKS_OFF
        | R::REFUSAL_REASON_NO_USER_ON_SERVER
        | R::REFUSAL_REASON_PASSKEYS_OFF
        | R::REFUSAL_REASON_LAST_SIGN_IN_METHOD => ErrorCode::FailedPrecondition,
        R::REFUSAL_REASON_LINK_CODE_NOT_FOUND
        | R::REFUSAL_REASON_LINK_EXPIRED
        | R::REFUSAL_REASON_SIGN_IN_LINK_EXPIRED
        | R::REFUSAL_REASON_USER_INVITE_NOT_FOUND
        | R::REFUSAL_REASON_SERVER_SIGN_IN_NOT_FOUND
        | R::REFUSAL_REASON_PASSKEY_NOT_FOUND => ErrorCode::NotFound,
        R::REFUSAL_REASON_TOO_MANY_ATTEMPTS => ErrorCode::ResourceExhausted,
        R::REFUSAL_REASON_EMAIL_TAKEN
        | R::REFUSAL_REASON_USER_LINKED
        | R::REFUSAL_REASON_ACCOUNT_HAS_USER_ON_SERVER => ErrorCode::AlreadyExists,
        // `UNSPECIFIED` is never sent; a client sees it only for a reason it doesn't know.
        R::REFUSAL_REASON_EMAIL_INVALID
        | R::REFUSAL_REASON_PASSWORD_LENGTH
        | R::REFUSAL_REASON_PASSKEY_NAME_LENGTH
        | R::REFUSAL_REASON_WATCH_STATE_INVALID
        | R::REFUSAL_REASON_URL_INVALID
        | R::REFUSAL_REASON_SERVER_ADDRESS_INVALID
        | R::REFUSAL_REASON_VIEWING_EVENT_INVALID
        | R::REFUSAL_REASON_TRACK_PREFERENCE_INVALID
        | R::REFUSAL_REASON_WATCHLIST_INVALID
        | R::REFUSAL_REASON_CERTIFICATE_CHALLENGE_INVALID
        | R::REFUSAL_REASON_UNSPECIFIED => ErrorCode::InvalidArgument,
    }
}

/// Refuses a request whose session lacks Recent Authentication, whichever area found it, saying
/// how recent a sign-in must be.
fn recent_authentication_required(err: &RecentAuthenticationRequired) -> Refused {
    refuse(
        RefusalReason::REFUSAL_REASON_RECENT_AUTHENTICATION_REQUIRED,
        err.to_string(),
    )
    .param("recentMinutes", &RECENT_AUTHENTICATION_MINS)
}

/// The error a client gets for a failure on the Hub's side: logged, not sent, since it says
/// nothing the client can act on.
fn hub_failed(err: &impl std::fmt::Display) -> ConnectError {
    tracing::error!("{err}");
    ConnectError::internal("the Hub failed")
}

/// Runs a ConnectRPC call in a span named after its service and method, for Diagnostics.
async fn trace_call(request: Request, next: Next) -> Response {
    let Some((service, method)) = service_and_method(request.uri().path()) else {
        return next.run(request).await;
    };
    let span = tracing::info_span!(
        "rpc",
        otel.name = format!("{service}/{method}"),
        otel.kind = "server",
        rpc.system = "connect_rpc",
        rpc.service = service,
        rpc.method = method,
    );
    next.run(request).instrument(span).await
}

/// The service and method of an RPC's `path`, such as `/nama.hub.v1.HubService/GetHubInfo`, or
/// `None` for a path that isn't one, whose words a span mustn't carry.
fn service_and_method(path: &str) -> Option<(&str, &str)> {
    let (service, method) = path.strip_prefix('/')?.split_once('/')?;
    (service.starts_with("nama.hub.") && !method.contains('/')).then_some((service, method))
}

#[cfg(test)]
#[path = "api/api_tests.rs"]
mod tests;
