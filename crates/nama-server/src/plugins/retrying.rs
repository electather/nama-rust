//! Asking a Fulfiller again after it said to try again: a Hand-off or telling it to stop following
//! a Show (Requests), and telling it to stop monitoring a deleted title (Scanner).

use std::{sync::Arc, time::Duration};

use nama_plugin_api::{
    BoxFuture, PluginError, PluginId,
    fulfiller::{Fulfiller, TryAgainIn},
};

use super::{ExtensionPoints, Failure, FailureKind, ProviderError};

/// How many times in a row the Kernel asks a Fulfiller again after it said to try again, for one
/// Request or deleted title, before it gives up (ADR 0047).
// ponytail: a fixed cap, so a Fulfiller that always says to try again can't keep a Request waiting
// or be asked forever; make it the Fulfiller's to say if a service needs longer.
pub(crate) const RETRIES: u32 = 10;

/// Tells the Plugin `plugin`'s Fulfiller of `Kind` `asked` again with `call` after `wait`, as it
/// said to try again, and again each time it says so, up to [`RETRIES`] times. The waits run in the
/// Plugin's scope, so disabling it ends them at once (ADR 0047). Hands `gave_up` why, once, when
/// the Fulfiller fails, panics or still says to try again after the last; the caller logs it as it
/// logs a first ask that failed.
#[expect(
    clippy::too_many_arguments,
    reason = "the Plugin, its wait, what to tell it and how to log giving up are each the caller's"
)]
pub(crate) fn ask_again<Kind: 'static, Asked: Send + Sync + 'static>(
    plugins: &ExtensionPoints,
    plugin: PluginId,
    mut wait: Duration,
    asked: Asked,
    call: FulfillerCall<Kind, Asked>,
    gave_up: impl FnOnce(&PluginId, String) + Send + 'static,
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
                None => "it panicked or was disabled".to_owned(),
            };
            gave_up(&plugin, format!("{reason}, asked again {retried} times"));
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
