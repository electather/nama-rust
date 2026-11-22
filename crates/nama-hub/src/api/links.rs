//! Registering a Server, Linking its Users, and Unlinking either: `nama.hub.v1.ServerLinkService`,
//! `LinkService` and `LinkedServerService`, one for each kind of caller, and what a Linked Server
//! asks of the Hub's titles.

use connectrpc::{ConnectError, RequestContext, Response, ServiceRequest, ServiceResult};
use nama_hub_proto::nama::hub::v1::{
    self as proto, AcceptUserInviteRequest, AcceptUserInviteResponse, ApproveServerLinkRequest,
    ApproveServerLinkResponse, ApproveServerSignInRequest, ApproveServerSignInResponse,
    ApproveUserLinkRequest, ApproveUserLinkResponse, GetServerSignInRequest,
    GetServerSignInResponse, GetUserLinkRequest, GetUserLinkResponse, InviteUserRequest,
    InviteUserResponse, LinkService, LinkedServerService, ListLinkedServersRequest,
    ListLinkedServersResponse, ListLinkedUsersRequest, ListLinkedUsersResponse,
    ListSimilarTitlesRequest, ListSimilarTitlesResponse, ListUserInvitesRequest,
    ListUserInvitesResponse, PollServerLinkRequest, PollServerLinkResponse,
    PollServerSignInRequest, PollServerSignInResponse, PushWatchStatesRequest,
    PushWatchStatesResponse, RecordViewingEventsRequest, RecordViewingEventsResponse,
    RefusalReason, ServerLinkService, SetServerAddressesRequest, SetServerAddressesResponse,
    StartServerLinkRequest, StartServerLinkResponse, StartServerSignInRequest,
    StartServerSignInResponse, StartUserLinkRequest, StartUserLinkResponse,
    UnlinkLinkedUserRequest, UnlinkLinkedUserResponse, UnlinkServerRequest, UnlinkServerResponse,
    UnlinkUserRequest, UnlinkUserResponse,
};
use sqlx::PgPool;

use super::{accounts::signed_in, hub_failed, refuse, viewing};
use crate::{
    links::{
        self, ADDRESS_LENGTH, CODE_LIFETIME_MINS, InviteId, LinkId, LinkedServer, Links,
        ListedInvite, ListedServer, ListedUser, MAX_LAN_ADDRESSES, POLL_INTERVAL_SECS, Polled,
        SignInId,
    },
    sync::{MAX_CHANGED_AT_MS, MAX_CHANGES, MAX_ID_LENGTH, MAX_IDS, WatchStateChange, WatchStates},
    titles::{self, ExternalId, SimilarTitle, TitleKind},
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

    async fn list_user_invites(
        &self,
        ctx: RequestContext,
        _request: ServiceRequest<'_, ListUserInvitesRequest>,
    ) -> ServiceResult<ListUserInvitesResponse> {
        let account = signed_in(&ctx)?.account.id;
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
        let session = signed_in(&ctx)?;
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
        let account = signed_in(&ctx)?.account.id;
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
        let account = signed_in(&ctx)?.account.id;
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
}

/// `nama.hub.v1.LinkedServerService`: a Linked Server, by its credential.
pub(super) struct LinkedServerServiceHandler {
    pub(super) links: Links,
    pub(super) watch_states: WatchStates,
    pub(super) db: PgPool,
    pub(super) viewing: Viewing,
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
            Some(
                proto::TitleKind::TITLE_KIND_UNSPECIFIED | proto::TitleKind::TITLE_KIND_EPISODE,
            )
            | None => {
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

    async fn push_watch_states(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, PushWatchStatesRequest>,
    ) -> ServiceResult<PushWatchStatesResponse> {
        let server = linked_server(&ctx)?;
        let changes = changes_from_proto(request.to_owned_message().changes)?;
        self.watch_states
            .push(server, changes)
            .await
            .map_err(|err| hub_failed(&err))?;
        Response::ok(PushWatchStatesResponse::default())
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
            .poll_sign_in(linked_server(&ctx)?, request.device_code)
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
        self.links
            .set_addresses(linked_server(&ctx)?, request.url, &lan_addresses)
            .await
            .map_err(refused)?;
        Response::ok(SetServerAddressesResponse::default())
    }

    async fn record_viewing_events(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, RecordViewingEventsRequest>,
    ) -> ServiceResult<RecordViewingEventsResponse> {
        let server = linked_server(&ctx)?;
        let events = viewing::recorded_from_proto(request.to_owned_message().events)?;
        (self.viewing.record(server, events))
            .await
            .map_err(|err| viewing::refused(&err))?;
        Response::ok(RecordViewingEventsResponse::default())
    }
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
        E::RecentAuthenticationRequired => {
            refuse(R::REFUSAL_REASON_RECENT_AUTHENTICATION_REQUIRED, message)
        }
        E::EmailNotProven => refuse(R::REFUSAL_REASON_EMAIL_NOT_PROVEN, message),
        E::CodeNotFound => refuse(R::REFUSAL_REASON_LINK_CODE_NOT_FOUND, message).field("code"),
        E::Expired => refuse(R::REFUSAL_REASON_LINK_EXPIRED, message).field("device_code"),
        E::UserLinked => refuse(R::REFUSAL_REASON_USER_LINKED, message),
        E::InviteNotFound => refuse(R::REFUSAL_REASON_USER_INVITE_NOT_FOUND, message).field("id"),
        E::SignInNotFound => {
            refuse(R::REFUSAL_REASON_SERVER_SIGN_IN_NOT_FOUND, message).field("id")
        }
        E::NoUserOnServer => refuse(R::REFUSAL_REASON_NO_USER_ON_SERVER, message),
        E::UrlInvalid => refuse(R::REFUSAL_REASON_URL_INVALID, message).field("return_url"),
        E::AddressInvalid => refuse(R::REFUSAL_REASON_SERVER_ADDRESS_INVALID, message)
            .param("maxLength", &ADDRESS_LENGTH)
            .param("maxLanAddresses", &MAX_LAN_ADDRESSES),
        E::Random(_) | E::Database(_) => return hub_failed(&err),
    };
    refused.into()
}

/// `changes` as `sync` takes them, or the refusal of the first limit they break.
fn changes_from_proto(
    changes: Vec<proto::WatchStateChange>,
) -> Result<Vec<WatchStateChange>, ConnectError> {
    if changes.len() > MAX_CHANGES {
        return Err(invalid_change(
            "changes",
            format!("a push holds up to {MAX_CHANGES} changes"),
        ));
    }
    (changes.into_iter().enumerate())
        .map(|(nth, change)| change_from_proto(nth, change))
        .collect()
}

/// The `nth` change of a push, as `sync` takes it.
fn change_from_proto(
    nth: usize,
    change: proto::WatchStateChange,
) -> Result<WatchStateChange, ConnectError> {
    let field = |name: &str| format!("changes[{nth}].{name}");
    let id_fits = |id: &String| !id.is_empty() && id.chars().count() <= MAX_ID_LENGTH;
    if !(1..=MAX_IDS).contains(&change.ids.len()) || !change.ids.iter().all(id_fits) {
        return Err(invalid_change(
            &field("ids"),
            format!(
                "a change names 1 to {MAX_IDS} External IDs of 1 to {MAX_ID_LENGTH} characters"
            ),
        ));
    }
    let duration_ms = i64::try_from(change.duration_ms)
        .map_err(|_too_long| invalid_change(&field("duration_ms"), "the duration is too long"))?;
    let position_ms = (i64::try_from(change.position_ms).ok())
        .filter(|&position_ms| position_ms <= duration_ms)
        .ok_or_else(|| {
            invalid_change(&field("position_ms"), "the position is past the duration")
        })?;
    if !(0..=MAX_CHANGED_AT_MS).contains(&change.changed_at_ms) {
        return Err(invalid_change(
            &field("changed_at_ms"),
            "a change is dated from 1970 to the end of 9999",
        ));
    }
    Ok(WatchStateChange {
        user_id: change.user_id,
        episode: change.episode,
        ids: change.ids,
        played: change.played,
        position_ms,
        duration_ms,
        changed_at_ms: change.changed_at_ms,
    })
}

/// Refuses a push for a change at `field` that breaks a limit, which `message` names.
fn invalid_change(field: &str, message: impl Into<String>) -> ConnectError {
    refuse(RefusalReason::REFUSAL_REASON_WATCH_STATE_INVALID, message)
        .field(field)
        .param("maxChanges", &MAX_CHANGES)
        .param("maxIds", &MAX_IDS)
        .param("maxIdLength", &MAX_ID_LENGTH)
        .into()
}
