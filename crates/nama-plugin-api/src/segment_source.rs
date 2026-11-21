//! Provide: a Segment source, which says where a Version's Intro, Recap and Credits are, such as
//! from an outside service (ADR 0075).
//!
//! A Plugin serves it by registering an `Arc<dyn SegmentSource>`:
//!
//! ```ignore
//! registrar.provide::<dyn SegmentSource>(Priority::DEFAULT, Arc::new(my_source));
//! ```

use crate::{
    BoxFuture, PluginError,
    contract::{SegmentResults, SegmentSearch},
};

/// A Segment source. It speaks the contract's `SegmentSourceService` messages, as an Installed
/// Plugin's export does. It keeps to its outside service's rate limits; the Kernel doesn't count
/// calls, but waits out a [`TryAgainIn`].
///
/// [`TryAgainIn`]: crate::TryAgainIn
pub trait SegmentSource: Send + Sync + 'static {
    /// Where the Segments of the Version `search` describes are, at most one of each kind; empty
    /// if it doesn't know.
    ///
    /// # Errors
    ///
    /// [`TryAgainIn`] when it can't ask yet, such as past its service's rate limit: the Kernel
    /// asks it about no Version for that long, clamped to 1 to 300 seconds, and about this one
    /// again after. Otherwise when it couldn't ask: the Kernel asks again later.
    ///
    /// [`TryAgainIn`]: crate::TryAgainIn
    fn search<'a>(
        &'a self,
        search: &'a SegmentSearch,
    ) -> BoxFuture<'a, Result<SegmentResults, PluginError>>;
}
