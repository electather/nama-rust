//! The one way the Kernel asks a Fulfiller about a title: the Hand-off of a Request and telling it
//! to stop following a Show (Requests), and telling it to stop monitoring a deleted title
//! (Scanner).

use std::{sync::Arc, time::Duration};

use nama_plugin_api::{BoxFuture, PluginError, PluginId, fulfiller::Fulfiller};

use super::{ExtensionPoints, Failure, Retryable};

/// How many times in a row the Kernel asks a Fulfiller again after it said to try again, for one
/// Request, unfollowed Show or deleted title, before it gives up (ADR 0047).
// ponytail: a fixed cap, so a Fulfiller that always says to try again can't keep a Request waiting
// or be asked forever; make it the Fulfiller's to say if a service needs longer.
pub(crate) const RETRIES: u32 = 10;

/// The enabled Plugin providing the Fulfiller of `Kind`, if any. The Registry refuses a second
/// Fulfiller for a kind (`EnableError::FulfillerTaken`), so this is the kind's one.
pub(crate) fn enabled_fulfiller<Kind: 'static>(points: &ExtensionPoints) -> Option<PluginId> {
    points.ranked::<dyn Fulfiller<Kind>>().into_iter().next()
}

/// The title a Fulfiller is asked about, as the needs check and its reason read it.
#[derive(Debug, Clone)]
pub(crate) struct AskedAbout<'a> {
    /// What a reason calls it, such as `Movie`, `Show` or `title`.
    word: &'a str,
    /// The namespaces of the ids it is known by.
    namespaces: Vec<&'a str>,
}

impl<'a> AskedAbout<'a> {
    /// The title a reason calls `word`, known by ids in `namespaces`.
    pub(crate) fn new(word: &'a str, namespaces: impl IntoIterator<Item = &'a str>) -> Self {
        Self {
            word,
            namespaces: namespaces.into_iter().collect(),
        }
    }
}

/// What a Fulfiller did when asked about a title, each with the Plugin providing it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FulfillerOutcome<R> {
    /// It acted, answering `answer`.
    Took { plugin: PluginId, answer: R },
    /// It wasn't asked, since the title has no id in any of the namespaces it `needs`; `reason`
    /// says so in words.
    LacksIds {
        plugin: PluginId,
        needs: Vec<String>,
        reason: String,
    },
    /// It said to try again after `wait`.
    TryAgain {
        plugin: PluginId,
        wait: Duration,
        message: String,
    },
    /// It failed.
    Failed { plugin: PluginId, message: String },
    /// It panicked, which disabled it, or was disabled meanwhile.
    Gone { plugin: PluginId },
}

/// A Fulfiller's method that tells its service something about `Asked`, such as
/// [`Fulfiller::unfollow`], answering `R`.
type FulfillerCall<Kind, Asked, R> =
    for<'a> fn(&'a dyn Fulfiller<Kind>, &'a Asked) -> BoxFuture<'a, Result<R, PluginError>>;

/// Asks the enabled Fulfiller of `Kind` to `call` about `asked`, unless `title` lacks every id it
/// needs. `None` when no Fulfiller of `Kind` is enabled. The answer is recorded for the Plugins
/// page as [`ExtensionPoints::ask`] records it.
pub(crate) async fn ask_fulfiller<Kind: 'static, Asked: Sync, R>(
    points: &ExtensionPoints,
    title: &AskedAbout<'_>,
    call: FulfillerCall<Kind, Asked, R>,
    asked: &Asked,
) -> Option<FulfillerOutcome<R>> {
    let plugin = enabled_fulfiller::<Kind>(points)?;
    // Inside the ask, so a Fulfiller's `needs` runs contained.
    let ask = |fulfiller: Arc<dyn Fulfiller<Kind>>| async move {
        if let Some(needs) = unmet(&*fulfiller, &title.namespaces) {
            return Ok(Err(needs));
        }
        call(&*fulfiller, asked).await.map(Ok).map_err(Retryable)
    };
    let answer = points.ask(&plugin, ask).await.map(|answer| answer.result);
    Some(match answer {
        Some(Ok(Ok(answer))) => FulfillerOutcome::Took { plugin, answer },
        Some(Ok(Err(needs))) => FulfillerOutcome::LacksIds {
            reason: missing(&plugin, title.word, &needs),
            needs,
            plugin,
        },
        Some(Err(Failure {
            kind: Some(wait),
            message,
        })) => FulfillerOutcome::TryAgain {
            plugin,
            wait,
            message,
        },
        Some(Err(Failure {
            kind: None,
            message,
        })) => FulfillerOutcome::Failed { plugin, message },
        None => FulfillerOutcome::Gone { plugin },
    })
}

/// Tells the enabled Fulfiller of `Kind` to `call` about `asked`, for a caller that only logs what
/// came of it, as [`ask_fulfiller`] asks. One that says to try again is asked again later, in its
/// scope, as `ask_again` says. Hands `not_told` the Plugin and why it wasn't told, once: at once
/// when it lacks ids, fails or panics, or when it is given up on after [`RETRIES`]. Returns the
/// Plugin when it wasn't told now; a retry still waiting returns `None`.
pub(crate) async fn tell_fulfiller<Kind: 'static, Asked: Clone + Send + Sync + 'static>(
    points: &ExtensionPoints,
    title: &AskedAbout<'_>,
    call: FulfillerCall<Kind, Asked, ()>,
    asked: &Asked,
    not_told: impl FnOnce(&PluginId, &str) + Send + 'static,
) -> Option<PluginId> {
    let (plugin, reason) = match ask_fulfiller(points, title, call, asked).await? {
        FulfillerOutcome::Took { .. } => return None,
        FulfillerOutcome::TryAgain { plugin, wait, .. } => {
            ask_again(points, plugin, wait, asked.clone(), call, not_told);
            return None;
        }
        FulfillerOutcome::LacksIds { plugin, reason, .. } => (plugin, reason),
        FulfillerOutcome::Failed { plugin, message } => (plugin, message),
        FulfillerOutcome::Gone { plugin } => (plugin, GONE.to_owned()),
    };
    not_told(&plugin, &reason);
    Some(plugin)
}

/// Why a Fulfiller that answered nothing wasn't told.
const GONE: &str = "it panicked or was disabled";

/// Asks the Plugin `plugin`'s Fulfiller again, after `wait`, to `call` about `asked`, as it said to
/// try again, and again each time it says so, up to [`RETRIES`] times. The waits run in the
/// Plugin's scope, so disabling it ends them at once (ADR 0047). Hands `gave_up` the Plugin and
/// why, once, when the Fulfiller fails, panics or still says to try again after the last.
#[expect(
    clippy::too_many_arguments,
    reason = "the Plugin, its wait, what to tell it and how to log giving up are each the caller's"
)]
fn ask_again<Kind: 'static, Asked: Send + Sync + 'static>(
    plugins: &ExtensionPoints,
    plugin: PluginId,
    mut wait: Duration,
    asked: Asked,
    call: FulfillerCall<Kind, Asked, ()>,
    gave_up: impl FnOnce(&PluginId, &str) + Send + 'static,
) {
    let points = plugins.clone();
    let scope = plugin.clone();
    // Not spawned if the Plugin was disabled meanwhile, which leaves nothing to tell.
    plugins.spawn_in(&scope, async move {
        let asked = &asked;
        let ask = |fulfiller: Arc<dyn Fulfiller<Kind>>| async move {
            call(&*fulfiller, asked).await.map_err(Retryable)
        };
        for retried in 1..=RETRIES {
            tokio::time::sleep(wait).await;
            let reason = match points.ask(&plugin, ask).await.map(|answer| answer.result) {
                Some(Ok(())) => return,
                Some(Err(Failure {
                    kind: Some(again), ..
                })) if retried < RETRIES => {
                    wait = again;
                    continue;
                }
                Some(Err(failure)) => failure.message,
                None => GONE.to_owned(),
            };
            gave_up(&plugin, &format!("{reason}, asked again {retried} times"));
            return;
        }
    });
}

/// The namespaces `fulfiller` needs when a title, known by ids in `namespaces`, has an id in none
/// of them, so the Kernel doesn't ask it about the title.
///
/// ponytail: read inside the `ask`, not with `read_provider`, so an unmet need clears the
/// Fulfiller's recorded failure though its service wasn't called (`plugins/AGENTS.md`).
fn unmet<Kind: 'static>(
    fulfiller: &dyn Fulfiller<Kind>,
    namespaces: &[&str],
) -> Option<Vec<String>> {
    let needs = fulfiller.needs();
    let met = needs.is_empty()
        || namespaces
            .iter()
            .any(|namespace| needs.iter().any(|n| n == namespace));
    (!met).then_some(needs)
}

/// Why the Plugin `plugin`'s Fulfiller wasn't asked about a title its reason calls `word`, which
/// has no id in any of the namespaces it `needs`.
fn missing(plugin: &PluginId, word: &str, needs: &[String]) -> String {
    let ids = match needs {
        [one] => format!("the {word}'s {one} id"),
        [first @ .., last] => format!("one of the {word}'s {} or {last} ids", first.join(", ")),
        [] => format!("the {word}'s id"),
    };
    format!("{plugin} needs {ids}, and none is known for it")
}

#[cfg(test)]
#[path = "fulfilling_tests.rs"]
mod tests;
