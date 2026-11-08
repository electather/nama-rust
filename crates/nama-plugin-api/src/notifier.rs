//! Provide: delivering notifications about Requests through an outside service, such as Telegram
//! (ADR 0021).
//!
//! The Kernel decides who is told what; a Notifier only delivers. Several Notifiers may be enabled
//! at once, and each is handed every notification. A Plugin provides one by registering it:
//!
//! ```ignore
//! registrar.provide::<dyn Notifier>(Priority::DEFAULT, Arc::new(my_notifier));
//! ```

use std::time::Duration;

pub use nama_proto::nama::plugin::v1::Notification;

use crate::{BoxFuture, PluginError};

/// How long one [`Notifier::deliver`] may take before the Kernel gives up on it as failed.
pub const DELIVER_TIMEOUT: Duration = Duration::from_secs(30);

/// Whom a [`Notification`] is for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recipient {
    /// The channel set in the Notifier's settings, which hears about every Request event. A
    /// Notifier with no channel set does nothing with these.
    Channel,
    /// The Plugin Connection of one User the Kernel tells, as the Plugin's Connector found it,
    /// such as their Telegram chat id.
    Connection(String),
}

/// A Provider that delivers notifications about Requests through an outside service.
pub trait Notifier: Send + Sync + 'static {
    /// Formats `notification` and sends it to `recipient`. The Kernel doesn't wait for this, so a
    /// slow service holds up nothing else, and gives up after [`DELIVER_TIMEOUT`].
    ///
    /// # Errors
    ///
    /// When the service didn't take the notification. The Kernel logs the reason, or the
    /// timeout, and doesn't retry.
    fn deliver<'a>(
        &'a self,
        notification: &'a Notification,
        recipient: &'a Recipient,
    ) -> BoxFuture<'a, Result<(), PluginError>>;
}
