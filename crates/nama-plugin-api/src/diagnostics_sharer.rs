//! Provide: taking the Diagnostics and Usage Analytics an Admin shares with Nama's developers to
//! them, such as through the Hub (ADR 0054). The Kernel hands over only what the Admin shares, as
//! OTLP/HTTP protobuf export requests, and the Plugin sends them on unchanged. Only a Built-in
//! Plugin provides it, by registering it:
//!
//! ```ignore
//! registrar.provide::<dyn DiagnosticsSharer>(Priority::DEFAULT, Arc::new(my_sharer));
//! ```

use crate::{BoxFuture, PluginError};

/// Which OTLP signal a batch holds, as its OTLP/HTTP path names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    /// Spans.
    Traces,
    /// Log records: the allow-listed log lines and Usage Analytics.
    Logs,
}

impl Signal {
    /// Its path under an OTLP/HTTP endpoint.
    #[must_use]
    pub const fn path(self) -> &'static str {
        match self {
            Self::Traces => "v1/traces",
            Self::Logs => "v1/logs",
        }
    }
}

/// A Provider that takes shared Diagnostics and Usage Analytics to Nama's developers.
pub trait DiagnosticsSharer: Send + Sync + 'static {
    /// Sends `batch`, an OTLP/HTTP protobuf export request of `signal`, on unchanged. A Plugin with
    /// nowhere to send it yet, such as a Server that isn't Linked, drops it.
    ///
    /// # Errors
    ///
    /// When it couldn't be sent: the batch is dropped, never retried.
    fn share(&self, signal: Signal, batch: Vec<u8>) -> BoxFuture<'_, Result<(), PluginError>>;
}
