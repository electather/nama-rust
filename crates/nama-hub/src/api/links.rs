//! Registering a Server and Unlinking it: `nama.hub.v1.ServerLinkService`, `LinkService` and
//! `LinkedServerService`, one for each kind of caller.

use connectrpc::{ConnectError, RequestContext, Response, ServiceRequest, ServiceResult};
use nama_hub_proto::nama::hub::v1::{
    self as proto, ApproveServerLinkRequest, ApproveServerLinkResponse, LinkService,
    LinkedServerService, ListLinkedServersRequest, ListLinkedServersResponse,
    PollServerLinkRequest, PollServerLinkResponse, RefusalReason, ServerLinkService,
    StartServerLinkRequest, StartServerLinkResponse, UnlinkServerRequest, UnlinkServerResponse,
};

use super::{accounts::signed_in, hub_failed, refuse};
use crate::links::{
    self, CODE_LIFETIME_MINS, LinkedServer, Links, ListedServer, POLL_INTERVAL_SECS, Polled,
};

/// `nama.hub.v1.ServerLinkService`: a Server not yet Linked, so anyone.
pub(super) struct ServerLinkServiceHandler {
    pub(super) links: Links,
}

#[expect(
    refining_impl_trait,
    reason = "plain `async fn` returning the owned response is the form the generated trait \
              documents; the refined signature isn't visible outside this crate"
)]
impl ServerLinkService for ServerLinkServiceHandler {
    async fn start_server_link(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, StartServerLinkRequest>,
    ) -> ServiceResult<StartServerLinkResponse> {
        let started = self.links.start(request.name).await.map_err(refused)?;
        Response::ok(StartServerLinkResponse {
            device_code: started.device_code,
            user_code: started.user_code,
            expires_in_seconds: CODE_LIFETIME_MINS * 60,
            interval_seconds: POLL_INTERVAL_SECS,
            ..Default::default()
        })
    }

    async fn poll_server_link(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, PollServerLinkRequest>,
    ) -> ServiceResult<PollServerLinkResponse> {
        let polled = self
            .links
            .poll(request.device_code)
            .await
            .map_err(refused)?;
        Response::ok(match polled {
            Polled::Waiting => PollServerLinkResponse::default(),
            Polled::Linked { credential, token } => PollServerLinkResponse {
                linked: true,
                credential,
                token,
                ..Default::default()
            },
        })
    }
}

/// `nama.hub.v1.LinkService`: a signed-in Account.
pub(super) struct LinkServiceHandler {
    pub(super) links: Links,
}

#[expect(
    refining_impl_trait,
    reason = "plain `async fn` returning the owned response is the form the generated trait \
              documents; the refined signature isn't visible outside this crate"
)]
impl LinkService for LinkServiceHandler {
    async fn approve_server_link(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, ApproveServerLinkRequest>,
    ) -> ServiceResult<ApproveServerLinkResponse> {
        let server = self
            .links
            .approve(signed_in(&ctx)?, request.code)
            .await
            .map_err(refused)?;
        Response::ok(ApproveServerLinkResponse {
            server: server_to_proto(server).into(),
            ..Default::default()
        })
    }

    async fn list_linked_servers(
        &self,
        ctx: RequestContext,
        _request: ServiceRequest<'_, ListLinkedServersRequest>,
    ) -> ServiceResult<ListLinkedServersResponse> {
        let account = signed_in(&ctx)?.account.id;
        let servers = self
            .links
            .list(account)
            .await
            .map_err(|err| hub_failed(&err))?;
        Response::ok(ListLinkedServersResponse {
            servers: servers.into_iter().map(server_to_proto).collect(),
            ..Default::default()
        })
    }
}

/// `nama.hub.v1.LinkedServerService`: a Linked Server, by its credential.
pub(super) struct LinkedServerServiceHandler {
    pub(super) links: Links,
}

#[expect(
    refining_impl_trait,
    reason = "plain `async fn` returning the owned response is the form the generated trait \
              documents; the refined signature isn't visible outside this crate"
)]
impl LinkedServerService for LinkedServerServiceHandler {
    async fn unlink_server(
        &self,
        ctx: RequestContext,
        _request: ServiceRequest<'_, UnlinkServerRequest>,
    ) -> ServiceResult<UnlinkServerResponse> {
        let server = ctx.extensions().get::<LinkedServer>().ok_or_else(|| {
            ConnectError::from(refuse(
                RefusalReason::REFUSAL_REASON_SERVER_NOT_LINKED,
                "send a Linked Server's credential",
            ))
        })?;
        self.links
            .unlink(server)
            .await
            .map_err(|err| hub_failed(&err))?;
        Response::ok(UnlinkServerResponse::default())
    }
}

fn server_to_proto(server: ListedServer) -> proto::LinkedServer {
    proto::LinkedServer {
        id: server.id.to_string(),
        name: server.name,
        linked_at: server.linked_at,
        ..Default::default()
    }
}

/// The Connect error a client gets for a Link that couldn't be made or polled.
#[expect(
    clippy::needless_pass_by_value,
    reason = "by value, so callers can write `.map_err(refused)`"
)]
fn refused(err: links::Error) -> ConnectError {
    use RefusalReason as R;
    use links::Error as E;
    let message = err.to_string();
    let refused = match err {
        E::RecentAuthenticationRequired => {
            refuse(R::REFUSAL_REASON_RECENT_AUTHENTICATION_REQUIRED, message)
        }
        E::EmailNotProven => refuse(R::REFUSAL_REASON_EMAIL_NOT_PROVEN, message),
        E::CodeNotFound => refuse(R::REFUSAL_REASON_LINK_CODE_NOT_FOUND, message).field("code"),
        E::Expired => refuse(R::REFUSAL_REASON_LINK_EXPIRED, message).field("device_code"),
        E::Random(_) | E::Database(_) => return hub_failed(&err),
    };
    refused.into()
}
