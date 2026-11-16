//! Asking a Fulfiller again after it said to try again: a Hand-off or telling it to stop following
//! a Show (Requests), and telling it to stop monitoring a deleted title (Scanner).

use std::time::Duration;

use nama_plugin_api::{PluginError, fulfiller::TryAgainIn};

use super::{Failure, ProviderError};

/// How many times in a row the Kernel asks a Fulfiller again after it said to try again, for one
/// Request or deleted title, before it gives up (ADR 0047).
// ponytail: a fixed cap, so a Fulfiller that always says to try again can't keep a Request waiting
// or be asked forever; make it the Fulfiller's to say if a service needs longer.
pub(crate) const RETRIES: u32 = 10;

/// A Fulfiller's error, whose kind is how long it said to wait when it said to try again.
pub(crate) struct Retryable(pub(crate) PluginError);

impl ProviderError<Option<Duration>> for Retryable {
    fn failure(self) -> Failure<Option<Duration>> {
        Failure {
            kind: self.0.downcast_ref::<TryAgainIn>().map(|wait| wait.0),
            message: self.0.to_string(),
        }
    }
}
