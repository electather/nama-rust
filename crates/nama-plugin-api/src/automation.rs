//! The Kernel's functions, which any Plugin may call through the [`Kernel`] it finds in
//! [`Registrar::kernel`](crate::Registrar::kernel) (ADR 0048, ADR 0071).

use std::{error::Error, fmt, future, sync::Arc};

use buffa::Message;

pub use nama_plugin_proto::{FUNCTIONS, KernelFunction, functions, nama::plugin::v1::Permission};

use crate::BoxFuture;

/// Why the [`Kernel`] refused a function, in plain words, such as a Permission not granted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelRefused(pub String);

impl fmt::Display for KernelRefused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for KernelRefused {}

/// What any Plugin may ask the Kernel: one of its functions by name, with the request
/// `automation.proto` names for it encoded. Each function needs the Plugin [`Permission`]
/// `automation.proto` names beside it, which the Kernel checks. A closure is one, so a test's fake
/// Kernel is a closure.
pub trait Kernel: Send + Sync + 'static {
    /// Asks the Kernel's function `function`, and answers its encoded response.
    ///
    /// # Errors
    ///
    /// When the Kernel refused, in plain words: such as a function it doesn't have, a Plugin
    /// Permission not granted, or a request it can't read.
    fn call(
        &self,
        function: &str,
        request: Vec<u8>,
    ) -> BoxFuture<'_, Result<Vec<u8>, KernelRefused>>;
}

impl<F> Kernel for F
where
    F: Fn(&str, Vec<u8>) -> BoxFuture<'static, Result<Vec<u8>, KernelRefused>>
        + Send
        + Sync
        + 'static,
{
    fn call(
        &self,
        function: &str,
        request: Vec<u8>,
    ) -> BoxFuture<'_, Result<Vec<u8>, KernelRefused>> {
        self(function, request)
    }
}

impl dyn Kernel {
    /// Asks the Kernel's function `F` `request`, and answers its response.
    ///
    /// # Errors
    ///
    /// When the Kernel refused, as [`Kernel::call`] says, or answered something that isn't
    /// `F`'s response.
    pub async fn ask<F: KernelFunction>(
        &self,
        request: F::Request,
    ) -> Result<F::Response, KernelRefused> {
        let answer = self.call(F::NAME, request.encode_to_vec()).await?;
        F::Response::decode_from_slice(&answer).map_err(|_unreadable| {
            KernelRefused(format!("the Kernel's answer to {} can't be read", F::NAME))
        })
    }
}

/// A Kernel answering the function `F` alone, with `answer`, as a test's fake: it refuses a request
/// it can't read and any other function, as the Kernel does.
pub fn answering<F: KernelFunction>(
    answer: impl Fn(F::Request) -> Result<F::Response, KernelRefused> + Send + Sync + 'static,
) -> Arc<dyn Kernel> {
    Arc::new(
        move |function: &str, request: Vec<u8>| -> BoxFuture<'static, _> {
            let answered = if function == F::NAME {
                (F::Request::decode_from_slice(&request))
                    .map_err(|_unreadable| {
                        KernelRefused("the Kernel couldn't read the request".to_owned())
                    })
                    .and_then(&answer)
                    .map(|response| response.encode_to_vec())
            } else {
                Err(KernelRefused(format!(
                    "the Kernel has no function {function}"
                )))
            };
            Box::pin(future::ready(answered))
        },
    )
}

#[cfg(test)]
#[path = "automation_tests.rs"]
mod tests;
