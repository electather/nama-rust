//! Provide: delivering notifications about Requests through an outside service, such as Telegram
//! (ADR 0021).
//!
//! The Kernel decides who is told what; a Notifier only delivers. Several Notifiers may be enabled
//! at once, and each is handed every notification. A Plugin provides one by registering it:
//!
//! ```ignore
//! registrar.provide::<dyn Notifier>(Priority::DEFAULT, Arc::new(my_notifier));
//! ```

use std::{collections::BTreeMap, error::Error, fmt, time::Duration};

pub use nama_plugin_proto::nama::plugin::v1::Notification;

use crate::{BoxFuture, PluginError};

/// How long one [`Notifier::deliver`] may take before the Kernel gives up on it as failed.
pub const DELIVER_TIMEOUT: Duration = Duration::from_secs(30);

/// Whom a [`Notification`] is for.
#[derive(Clone, PartialEq, Eq)]
pub enum Recipient {
    /// The channel set in the Notifier's settings, which hears about every Request event. A
    /// Notifier with no channel set does nothing with these.
    Channel,
    /// The Plugin Connection of one User the Kernel tells, as the Plugin's Connector found it,
    /// such as their Telegram chat id.
    Connection(String),
    /// A User the Kernel tells who set this Plugin's Personal settings, with their values under
    /// each key as text, as config values are. The Kernel sends it only to a Plugin whose
    /// [`Manifest`](crate::Manifest) declares Personal settings.
    User(BTreeMap<String, String>),
}

// Hand-written so a Personal setting's value never reaches a log: only the keys show.
impl fmt::Debug for Recipient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Channel => f.write_str("Channel"),
            Self::Connection(connection) => f.debug_tuple("Connection").field(connection).finish(),
            Self::User(settings) => f
                .debug_tuple("User")
                .field(&settings.keys().collect::<Vec<_>>())
                .finish(),
        }
    }
}

/// The error a [`Notifier`] returns when the service says a [`Recipient::Connection`] is gone,
/// such as a User who blocked the bot. The Kernel then deletes that Plugin Connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecipientGone;

impl fmt::Display for RecipientGone {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the recipient is gone")
    }
}

impl Error for RecipientGone {}

/// A Provider that delivers notifications about Requests through an outside service.
pub trait Notifier: Send + Sync + 'static {
    /// Formats `notification` and sends it to `recipient`. The Kernel doesn't wait for this, so a
    /// slow service holds up nothing else, and gives up after [`DELIVER_TIMEOUT`].
    ///
    /// # Errors
    ///
    /// [`RecipientGone`] when the service says a [`Recipient::Connection`] is gone. Otherwise when
    /// the service didn't take the notification: the Kernel logs the reason, or the timeout, and
    /// doesn't retry.
    fn deliver<'a>(
        &'a self,
        notification: &'a Notification,
        recipient: &'a Recipient,
    ) -> BoxFuture<'a, Result<(), PluginError>>;
}

#[cfg(test)]
#[path = "notifier_tests.rs"]
mod tests;
