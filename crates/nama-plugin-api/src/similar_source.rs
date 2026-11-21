//! Provide: a Similar source, which says which titles are like a title, the same for every User
//! (ADR 0084).
//!
//! A Plugin serves it by registering an `Arc<dyn SimilarSource>`:
//!
//! ```ignore
//! registrar.provide::<dyn SimilarSource>(Priority::DEFAULT, Arc::new(my_source));
//! ```

use crate::{
    BoxFuture, PluginError,
    contract::{SimilarAsked, SimilarTitles},
};

/// A Similar source. It speaks the contract's `SimilarSourceService` messages, as an Installed
/// Plugin's export does. The Kernel asks it when it refreshes a title's Matches, and live only
/// for a title not on the Server, never saying who the User is.
pub trait SimilarSource: Send + Sync + 'static {
    /// The titles like the one `asked` names, of its kind, each with a weight from 0.0 to 1.0;
    /// empty if it knows none.
    ///
    /// # Errors
    ///
    /// When it couldn't ask: the Kernel keeps the answer it stored last.
    fn similar<'a>(
        &'a self,
        asked: &'a SimilarAsked,
    ) -> BoxFuture<'a, Result<SimilarTitles, PluginError>>;
}
