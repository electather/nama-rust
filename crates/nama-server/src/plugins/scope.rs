//! A Plugin enablement's scope: the Kernel tasks started for it, cancelled and joined before the
//! Plugin is dropped (ADR 0047).

use std::future::Future;

use nama_plugin_api::Stop;
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use tracing::error;

use super::contain_sync;

/// The tasks started for one enablement of a Plugin, the token that stops them, and the
/// enablement's [`Stop`], which cancelling fires.
///
/// Cloning gives another handle to the same scope.
#[derive(Clone, Default)]
pub(super) struct Scope {
    cancel: CancellationToken,
    tasks: TaskTracker,
    stop: Stop,
}

impl Scope {
    /// Runs `task` until it ends or the scope is cancelled, whichever comes first. Must be called
    /// within a Tokio runtime.
    pub(super) fn spawn(&self, task: impl Future<Output = ()> + Send + 'static) {
        self.tasks
            .spawn(self.cancel.clone().run_until_cancelled_owned(task));
    }

    /// The scope, whose cancel fires `stop`, the signal its Plugin was handed at enable.
    pub(super) fn with_stop(self, stop: Stop) -> Self {
        Self { stop, ..self }
    }

    /// Cancels every task in the scope, without waiting for them, and fires its [`Stop`], whose
    /// callbacks interrupt what the Plugin runs outside the scope, such as an Installed Plugin's
    /// WASM calls.
    pub(super) fn cancel(&self) {
        self.cancel.cancel();
        // A callback is Plugin code; one that panics leaves the rest unrun, which only the Plugin
        // itself loses.
        if let Err(message) = contain_sync(|| self.stop.stop()) {
            error!(message, "plugin panicked while being stopped");
        }
    }

    /// Cancels every task in the scope and waits until each has ended.
    pub(super) async fn close(self) {
        self.cancel();
        self.tasks.close();
        self.tasks.wait().await;
    }
}

#[cfg(test)]
#[path = "scope_tests.rs"]
mod tests;
