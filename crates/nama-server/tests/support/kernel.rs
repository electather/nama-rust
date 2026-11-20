//! Calling the Kernel's functions as a Plugin does: by name, with an encoded request.

use std::sync::Arc;

use buffa::Message;
use nama_plugin_api::automation::{Kernel, KernelRefused};

/// What `kernel`'s function `function` answers `request`, decoded, or why it refused.
pub(crate) async fn ask<Resp: Message>(
    kernel: &Arc<dyn Kernel>,
    function: &str,
    request: &impl Message,
) -> Result<Resp, KernelRefused> {
    let response = Resp::decode_from_slice(&kernel.call(function, request.encode_to_vec()).await?);
    // An assert rather than an expect: a unit test includes this file too, and clippy allows an
    // expect there but not in a test target's support file, so no one lint expectation fits both.
    let unread = response.as_ref().err();
    assert!(
        unread.is_none(),
        "the Kernel answered another message: {unread:?}"
    );
    Ok(response.unwrap_or_default())
}
