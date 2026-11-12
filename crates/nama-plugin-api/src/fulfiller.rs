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

pub use nama_proto::nama::plugin::v1::{Fulfilled, StopMonitoring, stop_monitoring};
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
    /// [`Fulfiller::progress`], and the quality profile the service used. Only this Fulfiller
    /// reads the reference.
    ///
    /// A Season Request naming a Followed Show in `followed_show_id` also asks it to follow the
    /// Show: to fetch the Seasons that come out later too, until [`Fulfiller::unfollow`]. The
    /// Kernel names one only while it is Approved, so a single-Season ask never asks to follow.
    ///
    /// # Errors
    ///
    /// When the service didn't take the Request, such as when it is unreachable or doesn't know
    /// the title or the quality profile `request` names, or the Fulfiller can't follow a Show it
    /// was asked to. The error is the reason.
    fn fulfill<'a>(&'a self, request: &'a Request)
    -> BoxFuture<'a, Result<Fulfilled, PluginError>>;

    /// Stops following the Show `request` is a Season of, as a hand-off asked it to, since its
    /// Followed Show ended. A Fulfiller that never follows, such as one for Movies, leaves it be.
    ///
    /// # Errors
    ///
    /// When the service couldn't be told. The Kernel only logs it.
    fn unfollow<'a>(&'a self, _request: &'a Request) -> BoxFuture<'a, Result<(), PluginError>> {
        Box::pin(async { Ok(()) })
    }

    /// Stops the service monitoring `title`, which is being deleted, so it doesn't download it
    /// again. The service keeps the title, and one it doesn't have means nothing to do. Separate
    /// from [`Fulfiller::unfollow`]: a Show's new Seasons still come. A Fulfiller without it isn't
    /// asked.
    ///
    /// # Errors
    ///
    /// When the service couldn't be told. The delete goes ahead and names this Fulfiller, and the
    /// Kernel doesn't try it again.
    fn stop_monitoring<'a>(
        &'a self,
        _title: &'a StopMonitoring,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        Box::pin(async { Ok(()) })
    }

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
