//! Linking the Server, and its Users, through the enabled Plugin that links it, such as the Hub
//! Plugin (ADR 0011, ADR 0094): `ServerService`'s `GetServerLink`, `StartServerLink` and
//! `UnlinkServer`, `UserService`'s `GetUserLink`, `StartUserLink`, `UnlinkUser` and
//! `CreateUserFromAccount`, which Users `ListUsers` says can't sign in, and `SignInService`'s
//! sign-in with an Account (ADR 0092). A Link found Linked is a trigger for `Syncing`.

use std::{future::Future, net::SocketAddr, sync::Arc};

use connectrpc::{ConnectError, Response, ServiceResult};
use futures_util::future::join_all;
use nama_plugin_api::{
    PluginError,
    server_link::{Link, ServerLinker},
};
use nama_proto::nama::v1::{
    self as proto, CreateUserFromAccountResponse, GetServerLinkResponse, GetUserLinkResponse,
    RefusalReason, StartAccountSignInResponse, StartServerLinkResponse, StartUserLinkResponse,
    UnlinkServerResponse, UnlinkUserResponse,
};
use tracing::Instrument;

use super::{database_failed, refuse};
use crate::{
    diagnostics,
    plugins::ExtensionPoints,
    server_addresses,
    server_settings::ServerUrl,
    sync::Syncing,
    users::{User, Users},
};

/// What `unavailable` says when the Plugin that links the Server fails.
const LINKING_FAILED: &str = "the Plugin that links the Server failed; its page says why";

/// What `unavailable` says when a sign-in with an Account fails, such as while the Hub is down. A
/// Device already signed in never needs the Hub (ADR 0092).
/// No password advice: a User whose only Sign-in Method is their Account has none.
const ACCOUNT_SIGN_IN_FAILED: &str =
    "Signing in with an Account needs the Hub, which can't be reached right now. Try again later.";

/// Where Linking stands, unset without an enabled Plugin that links the Server. Once it is Linked,
/// tells `syncing`, as it can't tell the look that finds the Link new from a later one.
pub(super) async fn get(
    points: &ExtensionPoints,
    syncing: &Syncing,
) -> ServiceResult<GetServerLinkResponse> {
    let link = ask(points, async |linker| linker.link().await).await?;
    if let Some(Link::Linked { .. }) = &link {
        syncing.server_linked();
    }
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
/// the Server, or before the Server is Linked. Once they are Linked, tells `syncing`, as it can't
/// tell the look that finds a Link new from a later one.
pub(super) async fn get_user(
    points: &ExtensionPoints,
    syncing: &Syncing,
    user: &User,
) -> ServiceResult<GetUserLinkResponse> {
    let link = ask(points, async |linker| linker.user_link(user.id).await).await?;
    if let Some(Some(Link::Linked { .. })) = &link {
        syncing.user_linked(user.id);
    }
    Response::ok(GetUserLinkResponse {
        link: link.flatten().map(user_link_to_proto).into(),
        ..Default::default()
    })
}

/// Which of `user_ids` aren't Linked to an Account, asking the enabled Plugin that links the
/// Server about each at once, which is how it learns of an Unlink made on the service, such as by
/// deleting the Account (ADR 0099). One it fails to answer about is left out, as it may be Linked.
// ponytail: one call per User with no Sign-in Method, all at once, though the Plugin asks the
// Hub about each at most once a minute; cap them if Servers come to have many such Users.
pub(super) async fn unlinked(points: &ExtensionPoints, user_ids: Vec<i64>) -> Vec<i64> {
    let asked = user_ids.into_iter().map(|user_id| async move {
        let link = ask(points, async |linker| linker.user_link(user_id).await).await;
        let maybe_linked = matches!(link, Ok(Some(Some(Link::Linked { .. }))) | Err(_));
        (!maybe_linked).then_some(user_id)
    });
    join_all(asked).await.into_iter().flatten().collect()
}

/// Starts Linking `user`, whom the service lists by their name, or tells `syncing` if they turn out
/// to be Linked already.
pub(super) async fn start_user(
    points: &ExtensionPoints,
    syncing: &Syncing,
    user: &User,
) -> ServiceResult<StartUserLinkResponse> {
    let link = ask(points, async |linker| {
        linker.start_user_link(user.id, &user.name).await
    })
    .await?
    .ok_or_else(no_linker)?;
    if let Link::Linked { .. } = link {
        syncing.user_linked(user.id);
    }
    Response::ok(StartUserLinkResponse {
        link: user_link_to_proto(link).into(),
        ..Default::default()
    })
}

/// Unlinks `user`, or stops showing their code, unless they're Linked and their Account is their
/// last Sign-in Method, as for a User created from one. The service may still Unlink them (ADR
/// 0099), and an Admin's `ResetPassword` gets them back in.
pub(super) async fn unlink_user(
    points: &ExtensionPoints,
    users: &Users,
    user: &User,
) -> ServiceResult<UnlinkUserResponse> {
    if signs_in_only_through_link(points, users, user.id).await? {
        return Err(refuse(
            RefusalReason::REFUSAL_REASON_LAST_SIGN_IN_METHOD,
            "you sign in only with your Account, so Unlinking it would leave you no way in; \
             ask an Admin to give you a password first",
        )
        .into());
    }
    // ponytail: a code the Hub approves between the check and this Unlink is cancelled, and so
    // Unlinked, unchecked; the window is one Hub round trip.
    ask(points, async |linker| linker.unlink_user(user.id).await)
        .await?
        .ok_or_else(no_linker)?;
    Response::ok(UnlinkUserResponse::default())
}

/// Whether the User `user_id` is Linked and has no Sign-in Method the Server keeps, so their Link
/// is their only way in, which ADR 0014 says a User always keeps.
async fn signs_in_only_through_link(
    points: &ExtensionPoints,
    users: &Users,
    user_id: i64,
) -> Result<bool, ConnectError> {
    if users
        .has_sign_in_method(user_id)
        .await
        .map_err(database_failed)?
    {
        return Ok(false);
    }
    let link = ask(points, async |linker| linker.user_link(user_id).await).await?;
    Ok(matches!(link, Some(Some(Link::Linked { .. }))))
}

/// Creates a User named `name` from the Account with `email`, as `by` asks: the User, then asking
/// the Account to accept them. A User the service couldn't be asked about is deleted again.
pub(super) async fn create_user_from_account(
    points: &ExtensionPoints,
    users: &Users,
    by: &User,
    name: &str,
    email: &str,
) -> ServiceResult<CreateUserFromAccountResponse> {
    let (points, users, by) = (points.clone(), users.clone(), by.clone());
    let (name, email) = (name.to_owned(), email.to_owned());
    // Spawned, so a client that leaves mid-way can't drop it between creating the User and
    // deleting them again, which would leave a User with no way to sign in.
    let created = tokio::spawn(async move {
        let id = users.create_from_account(&by, &name).await?;
        let invited = ask_or(
            &points,
            "the Server couldn't ask the Hub, so no User was created; try again later",
            async |linker| linker.invite_user(id, name.trim(), &email).await,
        )
        .await
        .and_then(|invited| invited.ok_or_else(no_linker));
        if let Err(err) = invited {
            if let Err(not_deleted) = users.delete(&by, id).await {
                tracing::error!(
                    user = id,
                    "could not delete a User the Hub wasn't told of: {not_deleted}"
                );
            }
            return Err(err);
        }
        Ok::<_, ConnectError>(id)
    })
    .await
    .map_err(|panicked| {
        tracing::error!("creating a User from an Account panicked: {panicked}");
        ConnectError::internal("the Server failed to create the User")
    })??;
    Response::ok(CreateUserFromAccountResponse {
        id: created,
        ..Default::default()
    })
}

/// Whether the Server is Linked, so its Users can sign in with their Accounts. A Plugin that fails
/// to say counts as not Linked, and the sign-in page offers passwords alone.
pub(super) async fn account_sign_in(points: &ExtensionPoints) -> bool {
    let link = ask(points, async |linker| linker.link().await).await;
    matches!(link, Ok(Some(Link::Linked { .. })))
}

/// Starts signing in with an Account, which the service sends back to `return_url`, one of the
/// Server Addresses the Server reports given its Server URL `server_url` and `listening`.
pub(super) async fn start_account_sign_in(
    points: &ExtensionPoints,
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
    let started = ask_or(points, ACCOUNT_SIGN_IN_FAILED, async |linker| {
        linker.start_account_sign_in(return_url).await
    })
    .await?
    .ok_or_else(no_linker)?;
    Response::ok(StartAccountSignInResponse {
        url: started.url,
        handle: started.handle,
        ..Default::default()
    })
}

/// The id of the User the sign-in with an Account `handle` stands for, once it was approved and
/// finished with the one-time `code` the service sent the person back with. A refusal names
/// `field`, the request's field `handle` came in.
pub(super) async fn account_signing_in(
    points: &ExtensionPoints,
    handle: &str,
    code: &str,
    field: &str,
) -> Result<i64, ConnectError> {
    ask_or(points, ACCOUNT_SIGN_IN_FAILED, async |linker| {
        linker.finish_account_sign_in(handle, code).await
    })
    .await?
    .ok_or_else(no_linker)?
    .ok_or_else(|| {
        refuse(
            RefusalReason::REFUSAL_REASON_ACCOUNT_SIGN_IN_NOT_FOUND,
            "the sign-in wasn't approved on the Hub, or it expired; start again",
        )
        .field(field)
        .into()
    })
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
    call: impl Fn(Arc<dyn ServerLinker>) -> F,
) -> Result<Option<R>, ConnectError>
where
    F: Future<Output = Result<R, PluginError>>,
{
    ask_or(points, LINKING_FAILED, call).await
}

/// [`ask`], answering a failure `unavailable` with `unavailable` as its message.
async fn ask_or<R, F>(
    points: &ExtensionPoints,
    unavailable: &'static str,
    call: impl Fn(Arc<dyn ServerLinker>) -> F,
) -> Result<Option<R>, ConnectError>
where
    F: Future<Output = Result<R, PluginError>>,
{
    let Some(answer) = points.ask_first::<dyn ServerLinker, _, _, _, _>(call).await else {
        return Ok(None);
    };
    answer.result.map(Some).map_err(|failure| {
        tracing::warn!(plugin = %answer.plugin, "Linking failed: {}", failure.message);
        ConnectError::unavailable(unavailable)
    })
}

/// Whether `url` is an `http` or `https` URL with a host.
fn is_web_address(url: &str) -> bool {
    url::Url::parse(url).is_ok_and(|url| matches!(url.scheme(), "http" | "https") && url.has_host())
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
                names: reachability.names,
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
