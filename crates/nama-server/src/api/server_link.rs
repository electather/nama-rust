//! Linking the Server, and its Users, through the enabled Plugin that links it, such as the Hub
//! Plugin (ADR 0011, ADR 0094), which `Linking` asks: `ServerService`'s `GetServerLink`,
//! `StartServerLink` and `UnlinkServer`, `UserService`'s `GetUserLink` and `StartUserLink`, and
//! `SignInService`'s `StartAccountSignIn` (ADR 0092), with the Refusals of `Linking`'s errors.

use std::net::SocketAddr;

use connectrpc::{ConnectError, Response, ServiceResult};
use nama_proto::nama::v1::{
    self as proto, GetServerLinkResponse, GetUserLinkResponse, RefusalReason,
    StartAccountSignInResponse, StartServerLinkResponse, StartUserLinkResponse,
    UnlinkServerResponse,
};

use super::refuse;
use crate::{
    link::{self, Link, Linking},
    server_addresses,
    server_settings::ServerUrl,
    users::User,
};

/// What `unavailable` says when the Plugin that links the Server fails.
pub(super) const LINKING_FAILED: &str =
    "the Plugin that links the Server failed; its page says why";

/// What `unavailable` says when a sign-in with an Account fails, such as while the Hub is down. A
/// Device already signed in never needs the Hub (ADR 0092).
/// No password advice: a User whose only Sign-in Method is their Account has none.
const ACCOUNT_SIGN_IN_FAILED: &str =
    "Signing in with an Account needs the Hub, which can't be reached right now. Try again later.";

/// Where Linking stands, unset without an enabled Plugin that links the Server.
pub(super) async fn get(linking: &Linking) -> ServiceResult<GetServerLinkResponse> {
    let link = unless_no_linking_plugin(linking.server_link().await)?;
    Response::ok(GetServerLinkResponse {
        link: link.map(link_to_proto).into(),
        ..Default::default()
    })
}

/// Starts Linking the Server, which the service lists as `name`.
pub(super) async fn start(linking: &Linking, name: &str) -> ServiceResult<StartServerLinkResponse> {
    let link = linking.start_server_link(name).await?;
    Response::ok(StartServerLinkResponse {
        link: link_to_proto(link).into(),
        ..Default::default()
    })
}

/// Unlinks the Server.
pub(super) async fn unlink(linking: &Linking) -> ServiceResult<UnlinkServerResponse> {
    linking.unlink_server().await?;
    Response::ok(UnlinkServerResponse::default())
}

/// Where Linking `user` stands, unset while they can't Link: without an enabled Plugin that links
/// the Server, or before the Server is Linked.
pub(super) async fn get_user(linking: &Linking, user: &User) -> ServiceResult<GetUserLinkResponse> {
    let link = unless_no_linking_plugin(linking.user_link(user.id).await)?;
    Response::ok(GetUserLinkResponse {
        link: link.flatten().map(user_link_to_proto).into(),
        ..Default::default()
    })
}

/// Starts Linking `user`, whom the service lists by their name.
pub(super) async fn start_user(
    linking: &Linking,
    user: &User,
) -> ServiceResult<StartUserLinkResponse> {
    let link = linking.start_user_link(user.id, &user.name).await?;
    Response::ok(StartUserLinkResponse {
        link: user_link_to_proto(link).into(),
        ..Default::default()
    })
}

/// Starts signing in with an Account, which the service sends back to `return_url`, one of the
/// Server Addresses the Server reports given its Server URL `server_url` and `listening`.
pub(super) async fn start_account_sign_in(
    linking: &Linking,
    return_url: &str,
    server_url: &ServerUrl,
    listening: SocketAddr,
) -> ServiceResult<StartAccountSignInResponse> {
    // The Hub refuses any other, but its refusal would reach the client as the Hub failing.
    if !is_web_address(return_url) {
        return Err(refuse(
            RefusalReason::REFUSAL_REASON_NOT_A_URL,
            "the return URL must be an http or https URL",
        )
        .field("return_url")
        .into());
    }
    // As above, and the client can say which address to open, as the Hub's refusal can't.
    if !server_addresses::is_reported(return_url, server_url, listening) {
        return Err(refuse(
            RefusalReason::REFUSAL_REASON_NOT_A_SERVER_ADDRESS,
            "open the Server at its Server URL to sign in with an Account",
        )
        .field("return_url")
        .param("serverUrl", &server_url.url())
        .into());
    }
    let started =
        (linking.start_account_sign_in(return_url).await).map_err(account_sign_in_failed)?;
    Response::ok(StartAccountSignInResponse {
        url: started.url,
        handle: started.handle,
        ..Default::default()
    })
}

/// `answer`, `None` when no Plugin links the Server.
fn unless_no_linking_plugin<T>(answer: Result<T, link::Error>) -> Result<Option<T>, ConnectError> {
    match answer {
        Err(link::Error::NoLinkingPlugin) => Ok(None),
        answer => Ok(Some(answer?)),
    }
}

/// The Connect error a client gets for a call to the Plugin that links the Server that failed.
impl From<link::Error> for ConnectError {
    fn from(err: link::Error) -> Self {
        linking_failed(err, LINKING_FAILED)
    }
}

/// [`link::Error`]'s Connect error, a sign-in with an Account's.
pub(super) fn account_sign_in_failed(err: link::Error) -> ConnectError {
    linking_failed(err, ACCOUNT_SIGN_IN_FAILED)
}

/// [`link::Error`]'s Connect error, saying `unavailable` when the Plugin couldn't tell.
fn linking_failed(err: link::Error, unavailable: &'static str) -> ConnectError {
    match err {
        link::Error::NoLinkingPlugin => no_linking_plugin(),
        link::Error::Failed => ConnectError::unavailable(unavailable),
    }
}

/// Whether `url` is an `http` or `https` URL with a host.
fn is_web_address(url: &str) -> bool {
    url::Url::parse(url).is_ok_and(|url| matches!(url.scheme(), "http" | "https") && url.has_host())
}

pub(super) fn no_linking_plugin() -> ConnectError {
    refuse(
        RefusalReason::REFUSAL_REASON_NO_SERVER_LINKER,
        "no enabled Plugin links the Server",
    )
    .into()
}

fn link_to_proto(link: Link) -> proto::ServerLink {
    match link {
        Link::NotLinked => proto::ServerLink::default(),
        Link::Waiting { code, url } => proto::ServerLink {
            code,
            url,
            ..Default::default()
        },
        Link::Linked {
            account,
            reachability,
        } => {
            let reachability = reachability.unwrap_or_default();
            proto::ServerLink {
                account,
                public_address: reachability.address,
                reachable: reachability.reachable,
                ..Default::default()
            }
        }
    }
}

/// A User's Link as the Server's shows it, but for where the service reached the Server.
fn user_link_to_proto(link: Link) -> proto::UserLink {
    let proto::ServerLink {
        code, url, account, ..
    } = link_to_proto(link);
    proto::UserLink {
        code,
        url,
        account,
        ..Default::default()
    }
}
