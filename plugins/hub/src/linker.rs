//! Calling the Hub as the Linked Server, whose Link the Kernel keeps (ADR 0105). Every Hub call
//! made as the Server goes through [`Linker::call`], so one rule decides what a Link the Hub dropped means.
//! Their plays as Viewing Events are in `viewing.rs`, and wait for the Hub in a queue, in `queue.rs`.

pub(crate) mod queue;
mod viewing;

use std::{future::Future, sync::Arc};

use nama_plugin_api::{
    BoxFuture, PluginError,
    automation::Kernel,
    contract::StoreKey,
    job::{JobRunner, RunJob},
    server_link::ServerLinker,
};

use crate::{
    KernelLink,
    hub::{Hub, LinkDropped, LinkedHub},
};

pub(crate) use self::viewing::{ViewingEvents, send_job};

/// Runs the Hub Plugin's Jobs. One runner serves them all, as the Kernel asks only the first
/// `JobRunner` a Plugin provides.
pub(crate) struct Jobs {
    pub(crate) viewing: ViewingEvents,
}

impl JobRunner for Jobs {
    fn run<'a>(&'a self, run: &'a RunJob) -> BoxFuture<'a, Result<(), PluginError>> {
        Box::pin(async move {
            match run.job_id.as_str() {
                viewing::SEND_JOB => self.viewing.send().await,
                other => Err(format!("the Hub Plugin has no Job {other}").into()),
            }
        })
    }
}

/// Calls the Hub as the Server, through the Kernel's Link.
pub(crate) struct Linker {
    hub: Hub,
    link: Arc<dyn KernelLink>,
    kernel: Arc<dyn Kernel>,
}

impl Linker {
    pub(crate) fn new(hub: Hub, link: Arc<dyn KernelLink>, kernel: Arc<dyn Kernel>) -> Self {
        Self { hub, link, kernel }
    }

    /// Makes `call` to the Hub as the Linked Server: `None` while the Server isn't Linked, and
    /// once the Hub answers it doesn't know the Link any more, which forgets the Link. A Hub that
    /// is unreachable or fails is a failure, and keeps the Link.
    pub(crate) async fn call<T, Called>(
        &self,
        call: impl FnOnce(LinkedHub) -> Called,
    ) -> Result<Option<T>, PluginError>
    where
        Called: Future<Output = Result<T, PluginError>>,
    {
        let Some(credential) = self.link.credential().await? else {
            return Ok(None);
        };
        match call(self.hub.linked(&credential)).await {
            Err(err) if err.is::<LinkDropped>() => {
                self.link.forget(&credential).await?;
                Ok(None)
            }
            called => called.map(Some),
        }
    }

    /// Whether `user_id` is Linked, as the Kernel keeps it.
    pub(crate) async fn user_linked(&self, user_id: i64) -> Result<bool, PluginError> {
        self.link.user_linked(user_id).await
    }
}

impl ServerLinker for Linker {}

fn store_key(key: &str) -> StoreKey {
    StoreKey {
        key: key.to_owned(),
        ..Default::default()
    }
}
