//! The Hub API: the ConnectRPC services in `nama.hub.v1`. Proto types stop here: areas take and
//! return their own, and this converts (ADR 0099).

mod accounts;

use std::sync::Arc;

use axum::{
    extract::{Request, State},
    http::{StatusCode, header},
    middleware,
    middleware::Next,
    response::{IntoResponse as _, Response},
};
use buffa::MessageName as _;
use connectrpc::{
    ConnectError, ErrorCode, ErrorDetail, RequestContext, ServiceRequest, ServiceResult,
};
use nama_forwarded::{IpRange, identify_source};
use nama_hub_proto::nama::hub::v1::{
    GetHubInfoRequest, GetHubInfoResponse, HubService, Refusal, RefusalReason,
};
use sqlx::PgPool;
use tracing::Instrument as _;

use crate::{accounts::Accounts, db, mail::Mail};

use self::accounts::{AccountServiceHandler, COOKIE_NAME};

/// The Hub's routes: every service, each call in a span of its own, knowing where it came from
/// through `trusted_proxies` (ADR 0018) and which Account, if any, sent it. Sign-in Links go out
/// through `mail`, if it is set up.
pub(crate) fn router(db: PgPool, trusted_proxies: &[IpRange], mail: Option<Mail>) -> axum::Router {
    let accounts = Accounts::new(db.clone(), mail);
    let rpcs = connectrpc::Router::new()
        .add_service(Arc::new(HubServiceHandler {
            db,
            sign_in_links: accounts.sends_sign_in_links(),
        }))
        .add_service(Arc::new(AccountServiceHandler {
            accounts: accounts.clone(),
        }))
        .into_axum_service();
    axum::Router::new()
        .fallback_service(rpcs)
        .layer(middleware::from_fn_with_state(accounts, identify_caller))
        .layer(middleware::from_fn_with_state(
            Arc::from(trusted_proxies),
            identify_source,
        ))
        .layer(middleware::from_fn(trace_call))
}

/// `nama.hub.v1.HubService`: facts about the Hub, for anyone.
struct HubServiceHandler {
    db: PgPool,
    sign_in_links: bool,
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
            sign_in_links: self.sign_in_links,
            ..Default::default()
        })
    }
}

/// The Hub's one auth layer: adds the `AccountSession` the request's session cookie names to its
/// extensions. A request without one is anonymous, and gets none.
async fn identify_caller(
    State(accounts): State<Accounts>,
    mut request: Request,
    next: Next,
) -> Response {
    let token = request
        .headers()
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|cookies| cookies.split(';'))
        .find_map(|cookie| cookie.trim().strip_prefix(COOKIE_NAME)?.strip_prefix('='));
    let Some(token) = token else {
        return next.run(request).await;
    };
    match accounts.session(token).await {
        Ok(Some(session)) => {
            request.extensions_mut().insert(session);
            next.run(request).await
        }
        Ok(None) => next.run(request).await,
        Err(err) => {
            tracing::error!("could not look up a session: {err}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
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
        R::REFUSAL_REASON_SIGN_IN_REQUIRED | R::REFUSAL_REASON_WRONG_EMAIL_OR_PASSWORD => {
            ErrorCode::Unauthenticated
        }
        R::REFUSAL_REASON_TOO_MANY_ATTEMPTS => ErrorCode::ResourceExhausted,
        R::REFUSAL_REASON_EMAIL_TAKEN => ErrorCode::AlreadyExists,
        R::REFUSAL_REASON_EMAIL_INVALID | R::REFUSAL_REASON_PASSWORD_LENGTH => {
            ErrorCode::InvalidArgument
        }
        R::REFUSAL_REASON_SIGN_IN_LINK_EXPIRED => ErrorCode::NotFound,
        R::REFUSAL_REASON_SIGN_IN_LINKS_OFF => ErrorCode::FailedPrecondition,
        // Never sent; a client sees it only for a reason it doesn't know.
        R::REFUSAL_REASON_UNSPECIFIED => ErrorCode::InvalidArgument,
    }
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
