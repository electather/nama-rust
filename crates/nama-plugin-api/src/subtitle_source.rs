//! Provide: a subtitle source, which searches an outside service for a Version's subtitles and
//! downloads one a User picks (ADR 0067).
//!
//! A Plugin serves it by registering an `Arc<dyn SubtitleSource>`:
//!
//! ```ignore
//! registrar.provide::<dyn SubtitleSource>(Priority::DEFAULT, Arc::new(my_source));
//! ```

use crate::{
    BoxFuture, PluginError,
    contract::{SubtitleDownload, SubtitleFile, SubtitleResults, SubtitleSearch},
};

/// A subtitle source. It speaks the contract's `SubtitleSourceService` messages, as an Installed
/// Plugin's exports do. It keeps to its outside service's rate limits; the Kernel doesn't count
/// calls.
pub trait SubtitleSource: Send + Sync + 'static {
    /// The source's subtitles for the Version `search` describes, best first; empty if none fit.
    ///
    /// # Errors
    ///
    /// When it couldn't search, such as past its service's rate limit.
    fn search<'a>(
        &'a self,
        search: &'a SubtitleSearch,
    ) -> BoxFuture<'a, Result<SubtitleResults, PluginError>>;

    /// The file of the subtitle a [`Self::search`] answered with `download`'s id.
    ///
    /// # Errors
    ///
    /// When it couldn't download it, such as past its service's daily quota.
    fn download<'a>(
        &'a self,
        download: &'a SubtitleDownload,
    ) -> BoxFuture<'a, Result<SubtitleFile, PluginError>>;
}
