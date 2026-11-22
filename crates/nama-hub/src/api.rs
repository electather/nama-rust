//! The Hub API: the ConnectRPC services in `nama.hub.v1`. Proto types stop here: areas take and
//! return their own, and this converts (ADR 0099).

use std::sync::Arc;

use axum::{extract::Request, middleware, middleware::Next, response::Response};
use connectrpc::{ConnectError, RequestContext, ServiceRequest, ServiceResult};
use nama_hub_proto::nama::hub::v1::{GetHubInfoRequest, GetHubInfoResponse, HubService};
use sqlx::PgPool;
use tracing::Instrument as _;

use crate::db;

/// The Hub's routes: every service, each call in a span of its own.
pub(crate) fn router(db: PgPool) -> axum::Router {
    let rpcs = connectrpc::Router::new()
        .add_service(Arc::new(HubServiceHandler { db }))
        .into_axum_service();
    axum::Router::new()
        .fallback_service(rpcs)
        .layer(middleware::from_fn(trace_call))
}

/// `nama.hub.v1.HubService`: facts about the Hub, for anyone.
struct HubServiceHandler {
    db: PgPool,
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
            ..Default::default()
        })
    }
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
#[path = "api_tests.rs"]
mod tests;
