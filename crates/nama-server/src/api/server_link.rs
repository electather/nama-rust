//! Linking the Server through the enabled Plugin that links it, such as the Hub Plugin (ADR 0011,
//! ADR 0094): `ServerService`'s `GetServerLink`, `StartServerLink` and `UnlinkServer`.

use std::future::Future;

use connectrpc::{ConnectError, Response, ServiceResult};
use nama_plugin_api::{
    PluginError,
    server_link::{ServerLink, ServerLinker},
};
use nama_proto::nama::v1::{
    self as proto, GetServerLinkResponse, RefusalReason, StartServerLinkResponse,
    UnlinkServerResponse,
};

use super::refuse;
use crate::plugins::ExtensionPoints;

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
        tracing::warn!(plugin = %answer.plugin, "Linking the Server failed: {}", failure.message);
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

fn link_to_proto(link: ServerLink) -> proto::ServerLink {
    match link {
        ServerLink::NotLinked => proto::ServerLink::default(),
        ServerLink::Waiting { code, url } => proto::ServerLink {
            code,
            url,
            ..Default::default()
        },
        ServerLink::Linked { account } => proto::ServerLink {
            account,
            ..Default::default()
        },
    }
}
