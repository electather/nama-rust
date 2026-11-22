//! Linking the Server, and its Users, through the enabled Plugin that links it, such as the Hub
//! Plugin (ADR 0011, ADR 0094): `ServerService`'s `GetServerLink`, `StartServerLink` and
//! `UnlinkServer`, `UserService`'s `GetUserLink`, `StartUserLink`, `UnlinkUser` and
//! `CreateUserFromAccount`, and `SignInService`'s sign-in with an Account (ADR 0092). Through it
//! too, a User's Watch State on their other Servers comes back (ADR 0008).

use std::{future::Future, time::Duration};

use connectrpc::{ConnectError, Response, ServiceResult};
use nama_plugin_api::{
    PluginError,
    server_link::{Link, ServerLinker},
};
use nama_proto::nama::v1::{
    self as proto, CreateUserFromAccountResponse, Event, GetServerLinkResponse,
    GetUserLinkResponse, RefusalReason, StartAccountSignInResponse, StartServerLinkResponse,
    StartUserLinkResponse, UnlinkServerResponse, UnlinkUserResponse, event,
};
use tokio::{sync::broadcast::error::RecvError, task::JoinHandle};
use tracing::Instrument;

use super::refuse;
use crate::{
    browse::Browse,
    diagnostics,
    events::EventBus,
    plugins::ExtensionPoints,
    users::{User, Users},
};

/// How long a pull of a User's Watch State may take.
const PULL_TIMEOUT: Duration = Duration::from_mins(1);

/// What `unavailable` says when the Plugin that links the Server fails.
const LINKING_FAILED: &str = "the Plugin that links the Server failed; its page says why";

/// What `unavailable` says when a sign-in with an Account fails, such as while the Hub is down. A
/// Device already signed in never needs the Hub (ADR 0092).
/// No password advice: a User whose only Sign-in Method is their Account has none.
const ACCOUNT_SIGN_IN_FAILED: &str =
    "Signing in with an Account needs the Hub, which can't be reached right now. Try again later.";

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
/// the Server, or before the Server is Linked. Once they are Linked, pulls their Watch State too,
/// as it can't tell the look that finds a Link new from a later one.
pub(super) async fn get_user(
    points: &ExtensionPoints,
    browse: &Browse,
    user: &User,
) -> ServiceResult<GetUserLinkResponse> {
    let link = ask(points, async |linker| linker.user_link(user.id).await).await?;
    if let Some(Some(Link::Linked { .. })) = &link {
        pull_watch_states_soon(points, browse, user.id);
    }
    Response::ok(GetUserLinkResponse {
        link: link.flatten().map(user_link_to_proto).into(),
        ..Default::default()
    })
}

/// Starts Linking `user`, whom the service lists by their name, or pulls their Watch State if
/// they turn out to be Linked already.
pub(super) async fn start_user(
    points: &ExtensionPoints,
    browse: &Browse,
    user: &User,
) -> ServiceResult<StartUserLinkResponse> {
    let link = ask(points, async |linker| {
        linker.start_user_link(user.id, &user.name).await
    })
    .await?
    .ok_or_else(no_linker)?;
    if let Link::Linked { .. } = link {
        pull_watch_states_soon(points, browse, user.id);
    }
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

/// Starts signing in with an Account, which the service sends back to `return_url`.
pub(super) async fn start_account_sign_in(
    points: &ExtensionPoints,
    return_url: &str,
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

/// The id of the User the sign-in with an Account `handle` stands for, once it was approved.
pub(super) async fn account_signing_in(
    points: &ExtensionPoints,
    handle: &str,
) -> Result<i64, ConnectError> {
    ask_or(points, ACCOUNT_SIGN_IN_FAILED, async |linker| {
        linker.finish_account_sign_in(handle).await
    })
    .await?
    .ok_or_else(no_linker)?
    .ok_or_else(|| {
        refuse(
            RefusalReason::REFUSAL_REASON_ACCOUNT_SIGN_IN_NOT_FOUND,
            "the sign-in wasn't approved on the Hub, or it expired; start again",
        )
        .field("handle")
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

/// Brings the User `user_id`'s Watch State up to date from the enabled Plugin that links the
/// Server, as when they sign in or Link (ADR 0008), without waiting for it, so a slow or
/// unreachable service holds up nothing. Must be called within a Tokio runtime.
pub(super) fn pull_watch_states_soon(points: &ExtensionPoints, browse: &Browse, user_id: i64) {
    let (points, browse) = (points.clone(), browse.clone());
    let span =
        diagnostics::following_current(tracing::info_span!(parent: None, "pull_watch_states"));
    tokio::spawn(
        async move { pull_watch_states(&points, &browse, user_id).await }.instrument(span),
    );
}

/// Pulls every User's Watch State after each scan's Matching, so a title a scan added shows what
/// they did with it on their other Servers (ADR 0008), until the task is aborted.
pub(crate) fn pull_watch_states_after_scans(
    events: &EventBus,
    points: ExtensionPoints,
    browse: Browse,
    users: Users,
) -> JoinHandle<()> {
    let mut followed = events.follow();
    tokio::spawn(async move {
        loop {
            match followed.recv().await {
                Ok(Event {
                    event: Some(event::Event::MatchingFinished(_)),
                    ..
                })
                // The events it missed may have held a scan's.
                | Err(RecvError::Lagged(_)) => {}
                Ok(_) => continue,
                Err(RecvError::Closed) => return,
            }
            let listed = match users.list().await {
                Ok(listed) => listed,
                Err(err) => {
                    tracing::error!("listing Users to pull their Watch State failed: {err}");
                    continue;
                }
            };
            // One at a time, as each reads every Library.
            for user in listed {
                pull_watch_states(&points, &browse, user.id).await;
            }
        }
    })
}

/// Takes the User `user_id`'s Watch State from the enabled Plugin that links the Server, where it
/// is newer than the Server's own. A failure is logged: the next sign-in or scan pulls again.
async fn pull_watch_states(points: &ExtensionPoints, browse: &Browse, user_id: i64) {
    let Some(answer) = points
        .ask_first::<dyn ServerLinker, _, _, _, _>(async |linker| {
            // Bounded, so a Plugin that never answers can't hold up every later scan's pulls.
            tokio::time::timeout(PULL_TIMEOUT, linker.watch_states(user_id))
                .await
                .unwrap_or_else(|_elapsed| Err("it didn't answer in time".into()))
        })
        .await
    else {
        return;
    };
    let Ok(pulled) = answer.result else {
        // Ids only: `ask_first` already puts the failure's message on the Plugin's page.
        let plugin = &answer.plugin;
        tracing::warn!(%plugin, user = user_id, "could not pull a User's Watch State");
        return;
    };
    if let Err(err) = browse.take_pulled(user_id, pulled).await {
        tracing::error!(
            user = user_id,
            "taking a User's pulled Watch State failed: {err}"
        );
    }
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
    ask_or(points, LINKING_FAILED, call).await
}

/// [`ask`], answering a failure `unavailable` with `unavailable` as its message.
async fn ask_or<R, F>(
    points: &ExtensionPoints,
    unavailable: &'static str,
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
