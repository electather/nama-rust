//! The Kernel's functions, which an Installed Plugin calls as Extism imports in the namespace
//! `nama`: each passes its name and bytes to the enablement's Kernel and answers a `KernelAnswer`
//! (`automation.proto`).

use std::sync::{Arc, PoisonError};

use buffa::Message;
use extism::{CurrentPlugin, UserData, Val};
use nama_plugin_api::automation::{Kernel, KernelRefused};
use nama_plugin_proto::nama::plugin::v1::{KernelAnswer, kernel_answer};
use tokio::runtime::Handle;

use super::calls::Calls;

/// What a compiled Plugin's imports reach the Kernel through.
#[derive(Clone, Default)]
pub(super) struct Reach {
    /// The Kernel's functions for this enablement; `None` when compiling for no enablement, such
    /// as to read a Manifest or check settings.
    pub(super) kernel: Option<Arc<dyn Kernel>>,
    /// The runtime the async Kernel runs on, captured when compiling.
    pub(super) runtime: Option<Handle>,
    /// The calls running on the compiled Plugin, whose clock an import pauses while the Kernel
    /// answers.
    pub(super) calls: Calls,
}

/// The name of each Kernel function, as `automation.proto` lists them.
pub(crate) const KERNEL_IMPORTS: &[&str] = &[
    "store_get",
    "store_put",
    "store_delete",
    "list_libraries",
    "list_titles",
    "list_watch_states",
    "list_users",
    "list_requesters",
    "list_watchlist",
    "delete_media",
    "forget_home_rows",
];

fn refused(words: &str) -> kernel_answer::Result {
    kernel_answer::Result::Refused(words.to_owned())
}

/// The Extism host function for `function`: reads the request block the Plugin passes, hands it
/// to the enablement's Kernel, and returns a fresh block holding the encoded `KernelAnswer`. A
/// request it can't find is refused, never a trap.
pub(super) fn import(
    function: &'static str,
) -> impl Fn(&mut CurrentPlugin, &[Val], &mut [Val], UserData<Reach>) -> Result<(), extism::Error>
+ Send
+ Sync
+ 'static {
    move |plugin, inputs, outputs, reach| {
        let reach = reach
            .get()?
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        let request = inputs
            .first()
            .and_then(Val::i64)
            .and_then(|offset| plugin.memory_handle(offset.cast_unsigned()));
        let result = match (reach.kernel, reach.runtime, request) {
            (None, _, _) => refused("the Kernel's functions answer only an enabled Plugin"),
            (_, None, _) => refused("the Kernel's functions answer only on a running Server"),
            (_, _, None) => refused("the Kernel couldn't find the request in the Plugin's memory"),
            (Some(kernel), Some(runtime), Some(request)) => {
                let request = plugin.memory_bytes(request)?.to_vec();
                // The call is found by its instance's id rather than through Extism's host
                // context, since a call made with one skips the check of the export's return code.
                let _paused = reach.calls.pause(plugin.id().as_u128());
                // An import runs on the blocking thread its call runs on, so blocking is allowed.
                match runtime.block_on(kernel.call(function, request)) {
                    Ok(response) => kernel_answer::Result::Answer(response),
                    Err(KernelRefused(words)) => kernel_answer::Result::Refused(words),
                }
            }
        };
        let answer = KernelAnswer {
            result: Some(result),
            ..KernelAnswer::default()
        };
        let block = plugin.memory_new(answer.encode_to_vec())?;
        outputs.fill(plugin.memory_to_val(block));
        Ok(())
    }
}

#[cfg(test)]
#[path = "automation_tests.rs"]
mod tests;
