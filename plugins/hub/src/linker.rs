//! Calling the Hub as the Linked Server, whose Link the Kernel keeps (ADR 0105). Every Hub call
//! made as the Server goes through [`Linker::call`], so one rule decides what a Link the Hub dropped means.

use std::{future::Future, sync::Arc};

use nama_plugin_api::{PluginError, server_link::ServerLinker};

use crate::{
    KernelLink,
    hub::{Hub, LinkDropped, LinkedHub},
};

/// Calls the Hub as the Server, through the Kernel's Link.
pub(crate) struct Linker {
    hub: Hub,
    link: Arc<dyn KernelLink>,
}

impl Linker {
    pub(crate) fn new(hub: Hub, link: Arc<dyn KernelLink>) -> Self {
        Self { hub, link }
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
}

impl ServerLinker for Linker {}
