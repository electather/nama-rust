//! Registering a Server, Linking its Users, and Unlinking either: `nama.hub.v1.ServerLinkService`,
//! `LinkService` and `LinkedServerService`, one for each kind of caller. `LinkedServerService` is
//! one impl here: its RPCs that aren't about Links call the part of their concept.

use std::net::IpAddr;

use connectrpc::{ConnectError, RequestContext, Response, ServiceRequest, ServiceResult};
use nama_forwarded::RequestSource;
use nama_hub_proto::nama::hub::v1::{
    self as proto, AcceptUserInviteRequest, AcceptUserInviteResponse, ApproveServerLinkRequest,
    ApproveServerLinkResponse, ApproveServerSignInRequest, ApproveServerSignInResponse,
    ApproveUserLinkRequest, ApproveUserLinkResponse, CreateServerTokenRequest,
    CreateServerTokenResponse, GetRelayTokenRequest, GetRelayTokenResponse, GetServerSignInRequest,
    GetServerSignInResponse, GetUserLinkRequest, GetUserLinkResponse, InviteUserRequest,
    InviteUserResponse, LinkService, LinkedServerService, ListLinkedServersRequest,
    ListLinkedServersResponse, ListLinkedUsersRequest, ListLinkedUsersResponse,
    ListMostWatchlistedTitlesRequest, ListMostWatchlistedTitlesResponse,
    ListRecommendedTitlesRequest, ListRecommendedTitlesResponse, ListSimilarTitlesRequest,
    ListSimilarTitlesResponse, ListTrendingTitlesRequest, ListTrendingTitlesResponse,
    ListUserInvitesRequest, ListUserInvitesResponse, PollServerLinkRequest, PollServerLinkResponse,
    PollServerSignInRequest, PollServerSignInResponse, PullTrackPreferencesRequest,
    PullTrackPreferencesResponse, PullWatchStatesRequest, PullWatchStatesResponse,
    PushTrackPreferencesRequest, PushTrackPreferencesResponse, PushWatchStatesRequest,
    PushWatchStatesResponse, RecordViewingEventsRequest, RecordViewingEventsResponse,
    RefusalReason, ServerLinkService, SetCertificateChallengeRequest,
    SetCertificateChallengeResponse, SetServerAddressesRequest, SetServerAddressesResponse,
    StartServerLinkRequest, StartServerLinkResponse, StartServerSignInRequest,
    StartServerSignInResponse, StartUserLinkRequest, StartUserLinkResponse, SyncWatchlistRequest,
    SyncWatchlistResponse, UnlinkLinkedUserRequest, UnlinkLinkedUserResponse, UnlinkServerRequest,
    UnlinkServerResponse, UnlinkUserRequest, UnlinkUserResponse,
};
use sqlx::PgPool;

use super::{
    accounts::session, hub_failed, recent_authentication_required, refuse, titles,
    track_preferences, viewing, watch_states, watchlists,
};
use crate::{
    links::{
        self, ADDRESS_LENGTH, CODE_LIFETIME_MINS, InviteId, LinkId, LinkedServer, Links,
        ListedInvite, ListedServer, ListedUser, MAX_CHALLENGE_VALUES, MAX_LAN_ADDRESSES,
        POLL_INTERVAL_SECS, Polled, ReportedAddresses, SignInId,
    },
    recommendations::Recommendations,
    sync::{TrackPreferences, WatchStates, Watchlists},
    viewing::Viewing,
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
        let started = self.links.start(request.name, request.return_url);
        let started = started.await.map_err(refused)?;
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
        let (server, return_url) = self
            .links
            .approve(session(&ctx)?, request.code)
            .await
            .map_err(refused)?;
        Response::ok(ApproveServerLinkResponse {
            server: server_to_proto(server).into(),
            return_url,
            ..Default::default()
        })
    }

    async fn list_linked_servers(
        &self,
        ctx: RequestContext,
        _request: ServiceRequest<'_, ListLinkedServersRequest>,
    ) -> ServiceResult<ListLinkedServersResponse> {
        let account = session(&ctx)?.account.id;
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
            .approve_user(session(&ctx)?, request.code)
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
        let account = session(&ctx)?.account.id;
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
        let account = session(&ctx)?.account.id;
        // An id that isn't a UUID names no Link, so Unlinking it does nothing.
        if let Ok(link) = request.id.parse::<LinkId>() {
            self.links
                .unlink_linked_user(account, link)
                .await
                .map_err(|err| hub_failed(&err))?;
        }
        Response::ok(UnlinkLinkedUserResponse::default())
    }

    async fn list_user_invites(
        &self,
        ctx: RequestContext,
        _request: ServiceRequest<'_, ListUserInvitesRequest>,
    ) -> ServiceResult<ListUserInvitesResponse> {
        let account = session(&ctx)?.account.id;
        let invites = self
            .links
            .list_invites(account)
            .await
            .map_err(|err| hub_failed(&err))?;
        Response::ok(ListUserInvitesResponse {
            invites: invites.into_iter().map(invite_to_proto).collect(),
            ..Default::default()
        })
    }

    async fn accept_user_invite(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, AcceptUserInviteRequest>,
    ) -> ServiceResult<AcceptUserInviteResponse> {
        let session = session(&ctx)?;
        let user = self
            .links
            .accept_invite(session, invite_id(request.id)?)
            .await
            .map_err(refused)?;
        Response::ok(AcceptUserInviteResponse {
            user: user_to_proto(user).into(),
            ..Default::default()
        })
    }

    async fn get_server_sign_in(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, GetServerSignInRequest>,
    ) -> ServiceResult<GetServerSignInResponse> {
        let account = session(&ctx)?.account.id;
        let waiting = self
            .links
            .sign_in(account, sign_in_id(request.id)?)
            .await
            .map_err(refused)?;
        Response::ok(GetServerSignInResponse {
            server_name: waiting.server_name,
            user_name: waiting.user_name.unwrap_or_default(),
            ..Default::default()
        })
    }

    async fn approve_server_sign_in(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, ApproveServerSignInRequest>,
    ) -> ServiceResult<ApproveServerSignInResponse> {
        let account = session(&ctx)?.account.id;
        let return_url = self
            .links
            .approve_sign_in(account, sign_in_id(request.id)?)
            .await
            .map_err(refused)?;
        Response::ok(ApproveServerSignInResponse {
            return_url,
            ..Default::default()
        })
    }

    async fn create_server_token(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, CreateServerTokenRequest>,
    ) -> ServiceResult<CreateServerTokenResponse> {
        let session = session(&ctx)?;
        // An id that isn't a UUID names no Linked Server, so none the Account has a User on.
        let server = (request.linked_server_id.parse())
            .map_err(|_not_a_uuid| refused(links::Error::NoUserOnServer))?;
        let token = (self.links.server_token(session, server).await).map_err(refused)?;
        Response::ok(CreateServerTokenResponse {
            token,
            ..Default::default()
        })
    }
}

/// `nama.hub.v1.LinkedServerService`: a Linked Server, by its credential.
pub(super) struct LinkedServerServiceHandler {
    pub(super) links: Links,
    pub(super) watch_states: WatchStates,
    pub(super) track_preferences: TrackPreferences,
    pub(super) watchlists: Watchlists,
    pub(super) db: PgPool,
    pub(super) viewing: Viewing,
    pub(super) recommendations: Recommendations,
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
            .map_err(refused)?;
        Response::ok(UnlinkServerResponse::default())
    }

    async fn list_similar_titles(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, ListSimilarTitlesRequest>,
    ) -> ServiceResult<ListSimilarTitlesResponse> {
        titles::list_similar_titles(self, request.to_owned_message()).await
    }

    async fn list_recommended_titles(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, ListRecommendedTitlesRequest>,
    ) -> ServiceResult<ListRecommendedTitlesResponse> {
        titles::list_recommended_titles(self, &ctx, request.to_owned_message()).await
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

    async fn push_watch_states(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, PushWatchStatesRequest>,
    ) -> ServiceResult<PushWatchStatesResponse> {
        watch_states::push_watch_states(self, &ctx, request.to_owned_message()).await
    }

    async fn pull_watch_states(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, PullWatchStatesRequest>,
    ) -> ServiceResult<PullWatchStatesResponse> {
        watch_states::pull_watch_states(self, &ctx, request.to_owned_message()).await
    }

    async fn push_track_preferences(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, PushTrackPreferencesRequest>,
    ) -> ServiceResult<PushTrackPreferencesResponse> {
        track_preferences::push_track_preferences(self, &ctx, request.to_owned_message()).await
    }

    async fn pull_track_preferences(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, PullTrackPreferencesRequest>,
    ) -> ServiceResult<PullTrackPreferencesResponse> {
        track_preferences::pull_track_preferences(self, &ctx, request.to_owned_message()).await
    }

    async fn sync_watchlist(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, SyncWatchlistRequest>,
    ) -> ServiceResult<SyncWatchlistResponse> {
        watchlists::sync_watchlist(self, &ctx, request.to_owned_message()).await
    }

    async fn invite_user(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, InviteUserRequest>,
    ) -> ServiceResult<InviteUserResponse> {
        self.links
            .invite_user(
                linked_server(&ctx)?,
                request.user_id,
                request.name,
                request.email,
            )
            .await
            .map_err(|err| hub_failed(&err))?;
        Response::ok(InviteUserResponse::default())
    }

    async fn start_server_sign_in(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, StartServerSignInRequest>,
    ) -> ServiceResult<StartServerSignInResponse> {
        let started = self
            .links
            .start_sign_in(linked_server(&ctx)?, request.return_url)
            .await
            .map_err(refused)?;
        Response::ok(StartServerSignInResponse {
            device_code: started.device_code,
            id: started.id.to_string(),
            expires_in_seconds: CODE_LIFETIME_MINS * 60,
            ..Default::default()
        })
    }

    async fn poll_server_sign_in(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, PollServerSignInRequest>,
    ) -> ServiceResult<PollServerSignInResponse> {
        let token = self
            .links
            .poll_sign_in(linked_server(&ctx)?, request.device_code, request.code)
            .await
            .map_err(refused)?;
        Response::ok(PollServerSignInResponse {
            token: token.unwrap_or_default(),
            ..Default::default()
        })
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
        let reported = ReportedAddresses {
            url: request.url,
            lan_addresses: &lan_addresses,
            public_port: request.public_port,
            relay: request.relay,
            request_ip: request_ip(&ctx)?,
        };
        let server = linked_server(&ctx)?;
        let checked = self
            .links
            .set_addresses(server, reported)
            .await
            .map_err(refused)?;
        let probed = checked.public.unwrap_or_default();
        Response::ok(SetServerAddressesResponse {
            public_address: probed.address,
            reachable: probed.reachable,
            names: checked.names,
            server_id: server.id.to_string(),
            ..Default::default()
        })
    }

    async fn set_certificate_challenge(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, SetCertificateChallengeRequest>,
    ) -> ServiceResult<SetCertificateChallengeResponse> {
        let values = request.to_owned_message().values;
        (self.links.set_challenge(linked_server(&ctx)?, values))
            .await
            .map_err(refused)?;
        Response::ok(SetCertificateChallengeResponse::default())
    }

    #[expect(
        clippy::unused_async_trait_impl,
        reason = "the token is signed in memory, but the generated trait's methods are async"
    )]
    async fn get_relay_token(
        &self,
        ctx: RequestContext,
        _request: ServiceRequest<'_, GetRelayTokenRequest>,
    ) -> ServiceResult<GetRelayTokenResponse> {
        let token = (self.links.relay_token(linked_server(&ctx)?)).map_err(refused)?;
        Response::ok(GetRelayTokenResponse {
            token,
            ..Default::default()
        })
    }

    async fn record_viewing_events(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, RecordViewingEventsRequest>,
    ) -> ServiceResult<RecordViewingEventsResponse> {
        viewing::record_viewing_events(self, &ctx, request.to_owned_message()).await
    }

    async fn list_trending_titles(
        &self,
        _ctx: RequestContext,
        _request: ServiceRequest<'_, ListTrendingTitlesRequest>,
    ) -> ServiceResult<ListTrendingTitlesResponse> {
        titles::list_trending_titles(self).await
    }

    async fn list_most_watchlisted_titles(
        &self,
        _ctx: RequestContext,
        _request: ServiceRequest<'_, ListMostWatchlistedTitlesRequest>,
    ) -> ServiceResult<ListMostWatchlistedTitlesResponse> {
        watchlists::list_most_watchlisted_titles(self).await
    }
}

/// The IP address the request `ctx` came from (ADR 0018). Every request has one, so its absence
/// is a Hub bug and never a Refusal.
fn request_ip(ctx: &RequestContext) -> Result<IpAddr, ConnectError> {
    ctx.extensions()
        .get::<RequestSource>()
        .map(|source| source.client_ip)
        .ok_or_else(|| hub_failed(&"a request came without its peer's address"))
}

/// The invite `id` names. One that isn't a UUID names none.
fn invite_id(id: &str) -> Result<InviteId, ConnectError> {
    id.parse()
        .map_err(|_not_a_uuid| refused(links::Error::InviteNotFound))
}

/// The sign-in `id` names. One that isn't a UUID names none.
fn sign_in_id(id: &str) -> Result<SignInId, ConnectError> {
    id.parse()
        .map_err(|_not_a_uuid| refused(links::Error::SignInNotFound))
}

/// The Linked Server that sent the request `ctx` is for. The auth layer refuses every
/// `LinkedServerService` call without one, so its absence is a Hub bug and never a Refusal.
pub(super) fn linked_server(ctx: &RequestContext) -> Result<&LinkedServer, ConnectError> {
    ctx.extensions()
        .get::<LinkedServer>()
        .ok_or_else(|| hub_failed(&"a LinkedServerService call came without its Linked Server"))
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

fn invite_to_proto(invite: ListedInvite) -> proto::UserInvite {
    proto::UserInvite {
        id: invite.id.to_string(),
        name: invite.name,
        server_name: invite.server_name,
        invited_at: invite.invited_at,
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
        E::RecentAuthenticationRequired(ref err) => recent_authentication_required(err),
        E::EmailNotProven => refuse(R::REFUSAL_REASON_EMAIL_NOT_PROVEN, message),
        E::SignInLinksOff => refuse(R::REFUSAL_REASON_SIGN_IN_LINKS_OFF, message),
        E::CodeNotFound => refuse(R::REFUSAL_REASON_LINK_CODE_NOT_FOUND, message).field("code"),
        E::Expired => refuse(R::REFUSAL_REASON_LINK_EXPIRED, message).field("device_code"),
        E::UserLinked => refuse(R::REFUSAL_REASON_USER_LINKED, message),
        E::AccountHasUserOnServer => refuse(R::REFUSAL_REASON_ACCOUNT_HAS_USER_ON_SERVER, message),
        E::InviteNotFound => refuse(R::REFUSAL_REASON_USER_INVITE_NOT_FOUND, message).field("id"),
        E::SignInNotFound => {
            refuse(R::REFUSAL_REASON_SERVER_SIGN_IN_NOT_FOUND, message).field("id")
        }
        E::NoUserOnServer => refuse(R::REFUSAL_REASON_NO_USER_ON_SERVER, message),
        E::UrlInvalid => refuse(R::REFUSAL_REASON_URL_INVALID, message).field("return_url"),
        E::AddressInvalid => refuse(R::REFUSAL_REASON_SERVER_ADDRESS_INVALID, message)
            .param("maxLength", &ADDRESS_LENGTH)
            .param("maxLanAddresses", &MAX_LAN_ADDRESSES)
            .param("maxPort", &u16::MAX),
        E::ChallengeInvalid => refuse(R::REFUSAL_REASON_CERTIFICATE_CHALLENGE_INVALID, message)
            .field("values")
            .param("maxValues", &MAX_CHALLENGE_VALUES),
        // ADR 0019: no Refusal, as it is the Hub's own setup.
        E::Random(_) | E::Database(_) | E::Names(_) | E::NamesOff => return hub_failed(&err),
    };
    refused.into()
}

#[cfg(test)]
#[path = "links_tests.rs"]
mod tests;
