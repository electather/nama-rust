//! The fake Hub's answers to the Server's calls about its Users' Links.

use axum::http::HeaderMap;
use connectrpc::ConnectError;
use nama_hub_proto::nama::hub::v1::{
    GetUserLinkRequest, GetUserLinkResponse, InviteUserRequest, InviteUserResponse,
    StartUserLinkRequest, StartUserLinkResponse, UnlinkUserRequest, UnlinkUserResponse,
};
use ring::signature::Ed25519KeyPair;

use super::{EMAIL, State, USER_CODE, caller};

pub(super) fn start_user(
    state: &mut State,
    headers: &HeaderMap,
    request: &StartUserLinkRequest,
    _: &Ed25519KeyPair,
) -> Result<StartUserLinkResponse, ConnectError> {
    caller(state, headers)?;
    if user_account(state, request.user_id).is_some() {
        return Err(ConnectError::already_exists("the User is Linked"));
    }
    state
        .user_codes
        .insert(request.user_id, request.name.clone());
    Ok(StartUserLinkResponse {
        user_code: USER_CODE.to_owned(),
        expires_in_seconds: 600,
        ..Default::default()
    })
}

/// The email of the Account `user_id` is Linked to, if they are.
fn user_account(state: &State, user_id: i64) -> Option<String> {
    (state.user_accounts.get(&user_id).cloned()).or_else(|| {
        (state.everyone_linked && !state.unlinked_on_hub.contains(&user_id))
            .then(|| EMAIL.to_owned())
    })
}

pub(super) fn user_link(
    state: &mut State,
    headers: &HeaderMap,
    request: &GetUserLinkRequest,
    _: &Ed25519KeyPair,
) -> Result<GetUserLinkResponse, ConnectError> {
    caller(state, headers)?;
    Ok(GetUserLinkResponse {
        account: user_account(state, request.user_id).unwrap_or_default(),
        ..Default::default()
    })
}

pub(super) fn unlink_user(
    state: &mut State,
    headers: &HeaderMap,
    request: &UnlinkUserRequest,
    _: &Ed25519KeyPair,
) -> Result<UnlinkUserResponse, ConnectError> {
    caller(state, headers)?;
    state.unlinked_users.push(request.user_id);
    if state.unlinks_fail {
        return Err(ConnectError::internal("the Hub failed"));
    }
    state.user_codes.remove(&request.user_id);
    state.user_accounts.remove(&request.user_id);
    Ok(UnlinkUserResponse::default())
}

pub(super) fn invite(
    state: &mut State,
    headers: &HeaderMap,
    request: &InviteUserRequest,
    _: &Ed25519KeyPair,
) -> Result<InviteUserResponse, ConnectError> {
    caller(state, headers)?;
    state.invited = Some((request.user_id, request.name.clone(), request.email.clone()));
    Ok(InviteUserResponse::default())
}
