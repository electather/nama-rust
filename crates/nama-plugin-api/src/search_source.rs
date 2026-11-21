//! Provide: a Search source, which adds titles to a User's search, such as by what they are about
//! (ADR 0083).
//!
//! A Plugin serves it by registering an `Arc<dyn SearchSource>`:
//!
//! ```ignore
//! registrar.provide::<dyn SearchSource>(Priority::DEFAULT, Arc::new(my_source));
//! ```

use crate::{
    BoxFuture, PluginError,
    contract::{SearchSourceQuery, SearchSourceResults},
};

/// A Search source. It speaks the contract's `SearchSourceService` messages, as an Installed
/// Plugin's export does. It is asked for every search that isn't blank, and never told who is
/// searching.
pub trait SearchSource: Send + Sync + 'static {
    /// The titles that best answer `query`, best first; empty if none fit.
    ///
    /// # Errors
    ///
    /// When it couldn't answer: the search names it as failed and goes on without it.
    fn search<'a>(
        &'a self,
        query: &'a SearchSourceQuery,
    ) -> BoxFuture<'a, Result<SearchSourceResults, PluginError>>;
}
