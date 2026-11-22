//! Linking the Server, and its Users, through the enabled Plugin that links it, such as the Hub
//! Plugin (ADR 0011, ADR 0094): `ServerService`'s `GetServerLink`, `StartServerLink` and
//! `UnlinkServer`, and `UserService`'s `GetUserLink`, `StartUserLink` and `UnlinkUser`.

use std::future::Future;

use connectrpc::{ConnectError, Response, ServiceResult};
use nama_plugin_api::{
    PluginError,
    server_link::{Link, ServerLinker},
};
use nama_proto::nama::v1::{
    self as proto, GetServerLinkResponse, GetUserLinkResponse, RefusalReason,
    StartServerLinkResponse, StartUserLinkResponse, UnlinkServerResponse, UnlinkUserResponse,
};
use tracing::Instrument;

use super::refuse;
use crate::{diagnostics, plugins::ExtensionPoints, users::User};

/// Where Linking stands, unset without an enabled Plugin that links the Server.
pub(super) async fn get(points: &ExtensionPoints) -> ServiceResult<GetServerLinkResponse> {
    let link = ask(points, async |linker| linker.link().await).await?;
    Response::ok(GetServerLinkResponse {
        link: link.map(link_to_proto).into(),
        ..Default::default()
    })
}

/// Starts Linking the Server, which the service lists as `name`.
pub(super) async fn start(
    points: &ExtensionPoints,
    name: &str,
) -> ServiceResult<StartServerLinkResponse> {
    let link = ask(points, async |linker| linker.start_link(name).await)
        .await?
        .ok_or_else(no_linker)?;
    Response::ok(StartServerLinkResponse {
        link: link_to_proto(link).into(),
        ..Default::default()
    })
}

/// Unlinks the Server.
pub(super) async fn unlink(points: &ExtensionPoints) -> ServiceResult<UnlinkServerResponse> {
    ask(points, async |linker| linker.unlink().await)
        .await?
        .ok_or_else(no_linker)?;
    Response::ok(UnlinkServerResponse::default())
}

/// Where Linking `user` stands, unset while they can't Link: without an enabled Plugin that links
/// the Server, or before the Server is Linked.
pub(super) async fn get_user(
    points: &ExtensionPoints,
    user: &User,
) -> ServiceResult<GetUserLinkResponse> {
    let link = ask(points, async |linker| linker.user_link(user.id).await).await?;
    Response::ok(GetUserLinkResponse {
        link: link.flatten().map(user_link_to_proto).into(),
        ..Default::default()
    })
}

/// Starts Linking `user`, whom the service lists by their name.
pub(super) async fn start_user(
    points: &ExtensionPoints,
    user: &User,
) -> ServiceResult<StartUserLinkResponse> {
    let link = ask(points, async |linker| {
        linker.start_user_link(user.id, &user.name).await
    })
    .await?
    .ok_or_else(no_linker)?;
    Response::ok(StartUserLinkResponse {
        link: user_link_to_proto(link).into(),
        ..Default::default()
    })
}

/// Unlinks `user`.
pub(super) async fn unlink_user(
    points: &ExtensionPoints,
    user: &User,
) -> ServiceResult<UnlinkUserResponse> {
    ask(points, async |linker| linker.unlink_user(user.id).await)
        .await?
        .ok_or_else(no_linker)?;
    Response::ok(UnlinkUserResponse::default())
}

/// Unlinks the User `user_id`, just deleted, so the service forgets their Link or waiting code
/// too, without waiting for it. A failure is logged, not answered: the User is gone either way,
/// and the service being unreachable mustn't fail or hold up their delete (#1919). Must be called
/// within a Tokio runtime.
pub(super) fn unlink_deleted_user(points: &ExtensionPoints, user_id: i64) {
    let points = points.clone();
    let unlinking = async move {
        let unlinked = points
            .ask_first::<dyn ServerLinker, _, _, _, _>(async |linker| {
                linker.unlink_user(user_id).await
            })
            .await;
        if let Some(answer) = unlinked
            && answer.result.is_err()
        {
            // Ids only: `ask_first` already puts the failure's message on the Plugin's page. The
            // User may never have Linked, so this says only that the service wasn't told.
            tracing::warn!(
                plugin = %answer.plugin,
                user = user_id,
                "could not tell the service a User was deleted"
            );
        }
    };
    let span =
        diagnostics::following_current(tracing::info_span!(parent: None, "unlink_deleted_user"));
    tokio::spawn(unlinking.instrument(span));
}

/// What `call` gets from the enabled Plugin that links the Server, or `None` without one. Its
/// failure, such as the service being unreachable, is logged and answered `unavailable`.
async fn ask<R, F>(
    points: &ExtensionPoints,
    call: impl Fn(std::sync::Arc<dyn ServerLinker>) -> F,
) -> Result<Option<R>, ConnectError>
where
    F: Future<Output = Result<R, PluginError>>,
{
    let Some(answer) = points.ask_first::<dyn ServerLinker, _, _, _, _>(call).await else {
        return Ok(None);
    };
    answer.result.map(Some).map_err(|failure| {
        tracing::warn!(plugin = %answer.plugin, "Linking failed: {}", failure.message);
        ConnectError::unavailable("the Plugin that links the Server failed; its page says why")
    })
}

fn no_linker() -> ConnectError {
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
