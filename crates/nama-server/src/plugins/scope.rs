//! A Plugin enablement's scope: the Kernel tasks started for it, cancelled and joined before the
//! Plugin is dropped (ADR 0047).

use std::future::Future;

use tokio_util::{sync::CancellationToken, task::TaskTracker};

/// The tasks started for one enablement of a Plugin, and the token that stops them.
///
/// Cloning gives another handle to the same scope.
#[derive(Clone, Default)]
pub(super) struct Scope {
    cancel: CancellationToken,
    tasks: TaskTracker,
}

impl Scope {
    /// Runs `task` until it ends or the scope is cancelled, whichever comes first. Must be called
    /// within a Tokio runtime.
    pub(super) fn spawn(&self, task: impl Future<Output = ()> + Send + 'static) {
        self.tasks
            .spawn(self.cancel.clone().run_until_cancelled_owned(task));
    }

    /// Cancels every task in the scope, without waiting for them.
    pub(super) fn cancel(&self) {
        self.cancel.cancel();
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
