//! The Server's ConnectRPC API (ADR 0007), served on axum.

use std::{future, sync::Arc};

use connectrpc::{RequestContext, Response, ServiceRequest, ServiceResult};
use nama_proto::nama::v1::{GetServerInfoRequest, GetServerInfoResponse, ServerService};

use crate::config::Config;

/// Routes every Connect RPC the Server answers.
pub(crate) fn router(config: &Config) -> axum::Router {
    connectrpc::Router::new()
        .add_service(Arc::new(ServerServiceHandler {
            name: config.name.clone(),
        }))
        .into_axum_router()
}

/// `nama.v1.ServerService`: facts about this Server that any client can read.
struct ServerServiceHandler {
    name: String,
}

#[expect(
    refining_impl_trait_internal,
    reason = "the trait returns `impl Encodable<_>` so handlers can answer with borrowed views; \
              ours return the owned message, which is narrower and fine for a private type"
)]
impl ServerService for ServerServiceHandler {
    fn get_server_info(
        &self,
        _ctx: RequestContext,
        _request: ServiceRequest<'_, GetServerInfoRequest>,
    ) -> impl Future<Output = ServiceResult<GetServerInfoResponse>> + Send {
        future::ready(Response::ok(GetServerInfoResponse {
            name: self.name.clone(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            ..Default::default()
        }))
    }
}
