//! The Kernel's functions, which an Installed Plugin calls as Extism imports in the namespace
//! `nama`: one row per function, each answering a `KernelAnswer` (`automation.proto`).

use std::sync::{Arc, PoisonError};

use buffa::Message;
use extism::{CurrentPlugin, UserData, Val};
use nama_plugin_api::{
    BoxFuture,
    automation::{Kernel, KernelRefused},
};
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

/// One Kernel function: its import name and how it answers an encoded request.
pub(super) struct Function {
    pub(super) name: &'static str,
    answer: fn(&Handle, &dyn Kernel, &[u8]) -> kernel_answer::Result,
}

/// Every Kernel function, as `automation.proto` lists them.
pub(super) const FUNCTIONS: &[Function] = &[
    Function {
        name: "store_get",
        answer: |runtime, kernel, request| answer(runtime, kernel, request, Kernel::store_get),
    },
    Function {
        name: "store_put",
        answer: |runtime, kernel, request| answer(runtime, kernel, request, Kernel::store_put),
    },
    Function {
        name: "store_delete",
        answer: |runtime, kernel, request| answer(runtime, kernel, request, Kernel::store_delete),
    },
    Function {
        name: "list_libraries",
        answer: |runtime, kernel, request| answer(runtime, kernel, request, Kernel::list_libraries),
    },
    Function {
        name: "list_titles",
        answer: |runtime, kernel, request| answer(runtime, kernel, request, Kernel::list_titles),
    },
    Function {
        name: "list_watch_states",
        answer: |runtime, kernel, request| {
            answer(runtime, kernel, request, Kernel::list_watch_states)
        },
    },
    Function {
        name: "list_users",
        answer: |runtime, kernel, request| answer(runtime, kernel, request, Kernel::list_users),
    },
    Function {
        name: "list_requesters",
        answer: |runtime, kernel, request| {
            answer(runtime, kernel, request, Kernel::list_requesters)
        },
    },
    Function {
        name: "delete_media",
        answer: |runtime, kernel, request| answer(runtime, kernel, request, Kernel::delete_media),
    },
];

/// Decodes `request`, asks `call` of `kernel` on `runtime` and encodes its response.
fn answer<Req: Message, Resp: Message>(
    runtime: &Handle,
    kernel: &dyn Kernel,
    request: &[u8],
    call: for<'a> fn(&'a dyn Kernel, &'a Req) -> BoxFuture<'a, Result<Resp, KernelRefused>>,
) -> kernel_answer::Result {
    let Ok(request) = Req::decode_from_slice(request) else {
        return refused("the Kernel couldn't read the request");
    };
    // An import runs on the blocking thread its call runs on, so blocking here is allowed.
    match runtime.block_on(call(kernel, &request)) {
        Ok(response) => kernel_answer::Result::Answer(response.encode_to_vec()),
        Err(KernelRefused(words)) => kernel_answer::Result::Refused(words),
    }
}

fn refused(words: &str) -> kernel_answer::Result {
    kernel_answer::Result::Refused(words.to_owned())
}

/// The Extism host function for `function`: reads the request block the Plugin passes, and
/// returns a fresh block holding the encoded `KernelAnswer`. A request it can't read is refused,
/// never a trap.
pub(super) fn import(
    function: &'static Function,
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
                (function.answer)(&runtime, kernel.as_ref(), &request)
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
