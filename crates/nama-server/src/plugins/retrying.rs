//! Asking a Fulfiller again after it said to try again: a Hand-off or telling it to stop following
//! a Show (Requests), and telling it to stop monitoring a deleted title (Scanner).

use std::{sync::Arc, time::Duration};

use nama_plugin_api::{
    BoxFuture, PluginError, PluginId,
    fulfiller::{Fulfiller, TryAgainIn},
};
use tracing::warn;

use super::{ExtensionPoints, Failure, FailureKind, ProviderError};

/// How many times in a row the Kernel asks a Fulfiller again after it said to try again, for one
/// Request or deleted title, before it gives up (ADR 0047).
// ponytail: a fixed cap, so a Fulfiller that always says to try again can't keep a Request waiting
// or be asked forever; make it the Fulfiller's to say if a service needs longer.
pub(crate) const RETRIES: u32 = 10;

/// Asks the Plugin `plugin`'s Fulfiller of `Kind` again after `wait`, as it said to, to `call`
/// with `asked`, and again each time it says to try again, up to [`RETRIES`] times. The waits run
/// in the Plugin's scope, so disabling it ends them at once (ADR 0047). Logs once that the
/// Fulfiller didn't do `what` when it gives up, fails or panics. The Hand-off retries its own way
/// (`plugins/AGENTS.md`).
#[expect(
    clippy::too_many_arguments,
    reason = "the Plugin, its wait, what to ask it and what the log calls it are each the caller's"
)]
pub(crate) fn ask_again<Kind: 'static, Asked: Send + Sync + 'static>(
    plugins: &ExtensionPoints,
    plugin: PluginId,
    mut wait: Duration,
    asked: Asked,
    call: FulfillerCall<Kind, Asked>,
    what: String,
) {
    let points = plugins.clone();
    // Not spawned if the Plugin was disabled meanwhile, which leaves nothing to tell.
    plugins.spawn_in(&plugin.clone(), async move {
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
                None => "it panicked or was disabled".to_owned(),
            };
            warn!(plugin = %plugin, retried, reason, "a Fulfiller didn't {what}");
            return;
        }
    });
}

/// A Fulfiller's method that tells its service something about `Asked`, such as
/// [`Fulfiller::unfollow`].
type FulfillerCall<Kind, Asked> =
    for<'a> fn(&'a dyn Fulfiller<Kind>, &'a Asked) -> BoxFuture<'a, Result<(), PluginError>>;

/// A Fulfiller's error, whose kind is how long it said to wait when it said to try again.
pub(crate) struct Retryable(pub(crate) PluginError);

/// A failure unless the Fulfiller said to try again, since the service can't act yet.
impl FailureKind for Option<Duration> {
    fn is_failure(&self) -> bool {
        self.is_none()
    }
}

impl ProviderError<Option<Duration>> for Retryable {
    fn failure(self) -> Failure<Option<Duration>> {
        Failure {
            kind: self.0.downcast_ref::<TryAgainIn>().map(|wait| wait.0),
            message: self.0.to_string(),
        }
    }
}

#[cfg(test)]
#[path = "retrying_tests.rs"]
mod tests;
