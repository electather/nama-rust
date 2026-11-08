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
//! The Kernel uses one Fulfiller per kind: the first in the order
//! [`Registrar::provide`](crate::Registrar::provide) describes.

use std::collections::HashMap;

use nama_proto::nama::v1::Request;

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
    /// nothing downloading may be left out.
    ///
    /// # Errors
    ///
    /// When the service couldn't be asked.
    fn progress<'a>(
        &'a self,
        references: &'a [String],
    ) -> BoxFuture<'a, Result<HashMap<String, Vec<Download>>, PluginError>>;
}

/// One download a Fulfiller's service is making for a Request.
#[derive(Debug, Clone, PartialEq)]
pub struct Download {
    /// What is downloading, as the service names it.
    pub title: String,
    /// How much is done, from 0 to 100.
    pub percent: f32,
}
