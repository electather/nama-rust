//! Provide: handing Approved Requests to an outside service, such as Radarr (ADR 0003).
//!
//! A Fulfiller only hands the work off. The Kernel keeps each Request's state, and a Request
//! becomes Available only when a Library scan Matches it (ADR 0012). A Plugin provides a
//! Fulfiller for one kind of title by registering it under that kind:
//!
//! ```ignore
//! registrar.provide::<dyn Fulfiller<Movies>>(Priority::DEFAULT, Arc::new(my_fulfiller));
//! ```
//!
//! Each kind has at most one Fulfiller: the Kernel refuses to enable a Plugin providing one for a
//! kind another enabled Plugin already provides one for. Once enabled, a Fulfiller is handed every
//! Approved Request of its kind that was waiting for one.

use std::collections::HashMap;

use nama_proto::nama::v1::Request;
pub use nama_proto::nama::v1::{Download, DownloadStatus};

use crate::{BoxFuture, PluginError};

/// The kind a [`Fulfiller`] of Movie Requests is registered under.
#[derive(Debug)]
pub enum Movies {}

/// The kind a [`Fulfiller`] of Show Requests is registered under.
#[derive(Debug)]
pub enum Shows {}

/// A Provider that hands Requests for one kind of title, [`Movies`] or [`Shows`], to an outside
/// service and reports how they're progressing.
pub trait Fulfiller<Kind>: Send + Sync + 'static {
    /// Hands `request` to the service and returns at once, without waiting for it to be
    /// fulfilled, with a reference the Kernel keeps with the Request and passes back to
    /// [`Fulfiller::progress`]. Only this Fulfiller reads the reference.
    ///
    /// # Errors
    ///
    /// When the service didn't take the Request, such as when it is unreachable or doesn't know
    /// the title. The error is the reason.
    fn fulfill<'a>(&'a self, request: &'a Request) -> BoxFuture<'a, Result<String, PluginError>>;

    /// What the service is downloading for each of `references`, by reference. A reference with
    /// nothing downloading may be left out. While any Request is Processing, the Kernel asks this
    /// every minute, with the references of all this Fulfiller's Processing Requests at once, and
    /// gives up on an answer that takes longer than that.
    ///
    /// # Errors
    ///
    /// When the service couldn't be asked.
    fn progress<'a>(
        &'a self,
        references: &'a [String],
    ) -> BoxFuture<'a, Result<HashMap<String, Vec<Download>>, PluginError>>;
}
