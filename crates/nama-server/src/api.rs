//! The Server's ConnectRPC API (ADR 0007), served on axum.

use std::{future, sync::Arc};

use connectrpc::{Encodable, RequestContext, Response, ServiceRequest, ServiceResult};
use nama_proto::nama::v1::{GetServerInfoRequest, GetServerInfoResponse, ServerService};

use crate::config::Config;

/// Routes every RPC the Server answers.
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

impl ServerService for ServerServiceHandler {
    fn get_server_info<'a>(
        &'a self,
        _ctx: RequestContext,
        _request: ServiceRequest<'_, GetServerInfoRequest>,
    ) -> impl Future<Output = ServiceResult<impl Encodable<GetServerInfoResponse> + Send + use<'a>>> + Send
    {
        future::ready(Response::ok(GetServerInfoResponse {
            name: self.name.clone(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            ..Default::default()
        }))
    }
}
