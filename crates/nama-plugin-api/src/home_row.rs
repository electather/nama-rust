//! Provide: a Home row source, which gives headed rows of titles for the home page as data the
//! Kernel draws (ADR 0058).
//!
//! A Plugin serves it by registering an `Arc<dyn HomeRowSource>`:
//!
//! ```ignore
//! registrar.provide::<dyn HomeRowSource>(Priority::DEFAULT, Arc::new(my_rows));
//! ```

use crate::{
    BoxFuture, PluginError,
    contract::{HomeRows, SharedRowsAsked},
};

/// A Home row source. It speaks the contract's `HomeRowService` messages, as an Installed Plugin's
/// exports do. The Kernel asks it ahead of time and stores the answer, so home never waits on it.
pub trait HomeRowSource: Send + Sync + 'static {
    /// The rows every User sees, in the order home shows them.
    ///
    /// # Errors
    ///
    /// When it couldn't answer, which keeps the rows stored before.
    fn shared_rows<'a>(
        &'a self,
        asked: &'a SharedRowsAsked,
    ) -> BoxFuture<'a, Result<HomeRows, PluginError>>;
}
