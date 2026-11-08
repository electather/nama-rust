//! Provide: connecting a User to an outside service, such as their Telegram chat, so a Plugin can
//! act on it for them (ADR 0021).
//!
//! Plugins have no storage, so the Kernel keeps what a Connector finds as the User's Plugin
//! Connection, one per User and Plugin. A Plugin provides one by registering it:
//!
//! ```ignore
//! registrar.provide::<dyn Connector>(Priority::DEFAULT, Arc::new(my_connector));
//! ```

pub use nama_proto::nama::plugin::v1::ConnectionStarted;

use crate::{BoxFuture, PluginError};

/// A Provider that connects Users to an outside service through a link they open.
pub trait Connector: Send + Sync + 'static {
    /// The link a User opens to connect, made with `code`, a fresh random one-time code, and what
    /// the Kernel passes back to [`Connector::poll_connection`] while it waits.
    ///
    /// # Errors
    ///
    /// When the service couldn't be asked, such as for the bot's name.
    fn start_connection<'a>(
        &'a self,
        code: &'a str,
    ) -> BoxFuture<'a, Result<ConnectionStarted, PluginError>>;

    /// The User's identity at the service once they finished with the link `pending` came from,
    /// or `None` while they haven't. The Kernel asks every few seconds for up to 10 minutes, then
    /// gives up.
    ///
    /// # Errors
    ///
    /// When the service couldn't be asked. The Kernel logs it and asks again.
    fn poll_connection<'a>(
        &'a self,
        pending: &'a str,
    ) -> BoxFuture<'a, Result<Option<String>, PluginError>>;
}
