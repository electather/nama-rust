//! Registering a Server, Linking its Users, and Unlinking either: `nama.hub.v1.ServerLinkService`,
//! `LinkService` and `LinkedServerService`, one for each kind of caller, and what a Linked Server
//! asks of the Hub's titles.

use std::net::SocketAddr;

use connectrpc::{ConnectError, RequestContext, Response, ServiceRequest, ServiceResult};
use nama_forwarded::RequestSource;
use nama_hub_proto::nama::hub::v1::{
    self as proto, ApproveServerLinkRequest, ApproveServerLinkResponse, ApproveUserLinkRequest,
    ApproveUserLinkResponse, GetUserLinkRequest, GetUserLinkResponse, LinkService,
    LinkedServerService, ListLinkedServersRequest, ListLinkedServersResponse,
    ListLinkedUsersRequest, ListLinkedUsersResponse, ListSimilarTitlesRequest,
    ListSimilarTitlesResponse, PollServerLinkRequest, PollServerLinkResponse, RefusalReason,
    ServerLinkService, SetServerAddressesRequest, SetServerAddressesResponse,
    StartServerLinkRequest, StartServerLinkResponse, StartUserLinkRequest, StartUserLinkResponse,
    UnlinkLinkedUserRequest, UnlinkLinkedUserResponse, UnlinkServerRequest, UnlinkServerResponse,
    UnlinkUserRequest, UnlinkUserResponse,
};
use sqlx::PgPool;

use super::{accounts::signed_in, hub_failed, refuse};
use crate::{
    links::{
        self, ADDRESS_LENGTH, CODE_LIFETIME_MINS, LinkId, LinkedServer, Links, ListedServer,
        ListedUser, MAX_LAN_ADDRESSES, POLL_INTERVAL_SECS, Polled,
    },
    titles::{self, ExternalId, SimilarTitle, TitleKind},
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

    async fn approve_user_link(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, ApproveUserLinkRequest>,
    ) -> ServiceResult<ApproveUserLinkResponse> {
        let user = self
            .links
            .approve_user(signed_in(&ctx)?, request.code)
            .await
            .map_err(refused)?;
        Response::ok(ApproveUserLinkResponse {
            user: user_to_proto(user).into(),
            ..Default::default()
        })
    }

    async fn list_linked_users(
        &self,
        ctx: RequestContext,
        _request: ServiceRequest<'_, ListLinkedUsersRequest>,
    ) -> ServiceResult<ListLinkedUsersResponse> {
        let account = signed_in(&ctx)?.account.id;
        let users = self
            .links
            .list_users(account)
            .await
            .map_err(|err| hub_failed(&err))?;
        Response::ok(ListLinkedUsersResponse {
            users: users.into_iter().map(user_to_proto).collect(),
            ..Default::default()
        })
    }

    async fn unlink_linked_user(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, UnlinkLinkedUserRequest>,
    ) -> ServiceResult<UnlinkLinkedUserResponse> {
        let account = signed_in(&ctx)?.account.id;
        // An id that isn't a UUID names no Link, so Unlinking it does nothing.
        if let Ok(link) = request.id.parse::<LinkId>() {
            self.links
                .unlink_linked_user(account, link)
                .await
                .map_err(|err| hub_failed(&err))?;
        }
        Response::ok(UnlinkLinkedUserResponse::default())
    }
}

/// `nama.hub.v1.LinkedServerService`: a Linked Server, by its credential.
pub(super) struct LinkedServerServiceHandler {
    pub(super) links: Links,
    pub(super) db: PgPool,
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
        self.links
            .unlink(linked_server(&ctx)?)
            .await
            .map_err(|err| hub_failed(&err))?;
        Response::ok(UnlinkServerResponse::default())
    }

    async fn list_similar_titles(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, ListSimilarTitlesRequest>,
    ) -> ServiceResult<ListSimilarTitlesResponse> {
        linked_server(&ctx)?;
        let kind = match request.kind.as_known() {
            Some(proto::TitleKind::TITLE_KIND_MOVIE) => TitleKind::Movie,
            Some(proto::TitleKind::TITLE_KIND_SHOW) => TitleKind::Show,
            Some(proto::TitleKind::TITLE_KIND_UNSPECIFIED) | None => {
                return Response::ok(ListSimilarTitlesResponse::default());
            }
        };
        let ids: Vec<ExternalId> = request
            .ids
            .iter()
            .map(|id| ExternalId {
                namespace: id.namespace.to_owned(),
                value: id.value.to_owned(),
            })
            .collect();
        let similar = titles::similar(&self.db, kind, &ids)
            .await
            .map_err(|err| hub_failed(&err))?;
        Response::ok(ListSimilarTitlesResponse {
            titles: similar.into_iter().map(similar_to_proto).collect(),
            ..Default::default()
        })
    }

    async fn start_user_link(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, StartUserLinkRequest>,
    ) -> ServiceResult<StartUserLinkResponse> {
        let user_code = self
            .links
            .start_user(linked_server(&ctx)?, request.user_id, request.name)
            .await
            .map_err(refused)?;
        Response::ok(StartUserLinkResponse {
            user_code,
            expires_in_seconds: CODE_LIFETIME_MINS * 60,
            ..Default::default()
        })
    }

    async fn get_user_link(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, GetUserLinkRequest>,
    ) -> ServiceResult<GetUserLinkResponse> {
        let account = self
            .links
            .user_account(linked_server(&ctx)?, request.user_id)
            .await
            .map_err(|err| hub_failed(&err))?;
        Response::ok(GetUserLinkResponse {
            account: account.unwrap_or_default(),
            ..Default::default()
        })
    }

    async fn unlink_user(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, UnlinkUserRequest>,
    ) -> ServiceResult<UnlinkUserResponse> {
        self.links
            .unlink_user(linked_server(&ctx)?, request.user_id)
            .await
            .map_err(|err| hub_failed(&err))?;
        Response::ok(UnlinkUserResponse::default())
    }

    async fn set_server_addresses(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, SetServerAddressesRequest>,
    ) -> ServiceResult<SetServerAddressesResponse> {
        let lan_addresses: Vec<String> = request
            .lan_addresses
            .iter()
            .map(|&address| address.to_owned())
            .collect();
        let public = public_address(&ctx, request.public_port)?;
        let checked = self
            .links
            .set_addresses(linked_server(&ctx)?, request.url, &lan_addresses, public)
            .await
            .map_err(refused)?;
        Response::ok(
            checked.map_or_else(SetServerAddressesResponse::default, |checked| {
                SetServerAddressesResponse {
                    public_address: checked.address,
                    reachable: checked.reachable,
                    ..Default::default()
                }
            }),
        )
    }
}

/// The address to check the calling Server's public `port` at: the IP address its request `ctx`
/// came from (ADR 0018), never one it names, so a Server can't have the Hub call anyone else.
/// `None` for port 0.
fn public_address(ctx: &RequestContext, port: u32) -> Result<Option<SocketAddr>, ConnectError> {
    if port == 0 {
        return Ok(None);
    }
    let port = u16::try_from(port).map_err(|_too_big| refused(links::Error::AddressInvalid))?;
    let source = ctx
        .extensions()
        .get::<RequestSource>()
        .ok_or_else(|| hub_failed(&"a request came without its peer's address"))?;
    Ok(Some(SocketAddr::new(source.client_ip.to_canonical(), port)))
}

/// The Linked Server whose credential the request carried, which the auth layer found.
fn linked_server(ctx: &RequestContext) -> Result<&LinkedServer, ConnectError> {
    ctx.extensions().get::<LinkedServer>().ok_or_else(|| {
        refuse(
            RefusalReason::REFUSAL_REASON_SERVER_NOT_LINKED,
            "send a Linked Server's credential",
        )
        .into()
    })
}

fn similar_to_proto(similar: SimilarTitle) -> proto::SimilarTitle {
    proto::SimilarTitle {
        id: proto::ExternalId {
            namespace: similar.id.namespace,
            value: similar.id.value,
            ..Default::default()
        }
        .into(),
        weight: similar.weight,
        ..Default::default()
    }
}

fn server_to_proto(server: ListedServer) -> proto::LinkedServer {
    proto::LinkedServer {
        id: server.id.to_string(),
        name: server.name,
        linked_at: server.linked_at,
        url: server.url,
        addresses: server.addresses,
        ..Default::default()
    }
}

fn user_to_proto(user: ListedUser) -> proto::LinkedUser {
    proto::LinkedUser {
        id: user.id.to_string(),
        name: user.name,
        server_name: user.server_name,
        linked_at: user.linked_at,
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
        E::UserLinked => refuse(R::REFUSAL_REASON_USER_LINKED, message),
        E::AddressInvalid => refuse(R::REFUSAL_REASON_SERVER_ADDRESS_INVALID, message)
            .param("maxLength", &ADDRESS_LENGTH)
            .param("maxLanAddresses", &MAX_LAN_ADDRESSES)
            .param("maxPort", &u16::MAX),
        E::Random(_) | E::Database(_) => return hub_failed(&err),
    };
    refused.into()
}
