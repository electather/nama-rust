//! Provide: Watch sync, which follows each User's Watch State to an outside tracker and brings the
//! changes made there back.
//!
//! A Plugin serves it by registering an `Arc<dyn WatchSync>`:
//!
//! ```ignore
//! registrar.provide::<dyn WatchSync>(Priority::DEFAULT, Arc::new(my_sync));
//! ```

use crate::{
    BoxFuture, PluginError,
    contract::{
        ImportAsked, Imported, LinkPolled, LinkStarted, PollLinkAsked, ScrobbleAsked, Scrobbled,
        StartLinkAsked,
    },
};

/// Watch sync. It speaks the contract's `WatchSyncService` messages, as an Installed Plugin's
/// exports do. The Kernel asks it only for a User who set any of its Personal settings, with
/// theirs.
pub trait WatchSync: Send + Sync + 'static {
    /// Takes one write's changes to the User's Watch State to the outside tracker.
    ///
    /// # Errors
    ///
    /// When it couldn't: the Kernel logs it and doesn't ask again.
    fn scrobble<'a>(
        &'a self,
        asked: &'a ScrobbleAsked,
    ) -> BoxFuture<'a, Result<Scrobbled, PluginError>>;

    /// The changes to the User's Watch State on the outside tracker since `asked.since`, with
    /// where the next ask continues from.
    ///
    /// # Errors
    ///
    /// When it couldn't ask: the Kernel keeps the cursor and asks again next time.
    fn import<'a>(&'a self, asked: &'a ImportAsked)
    -> BoxFuture<'a, Result<Imported, PluginError>>;

    /// Starts linking a User's account at the outside tracker by device code.
    ///
    /// # Errors
    ///
    /// When the tracker couldn't be asked for a code.
    fn start_link<'a>(
        &'a self,
        asked: &'a StartLinkAsked,
    ) -> BoxFuture<'a, Result<LinkStarted, PluginError>>;

    /// Whether the User entered the code `asked.device_code` came with, and the tracker's tokens
    /// once they did.
    ///
    /// # Errors
    ///
    /// When the tracker couldn't be asked.
    fn poll_link<'a>(
        &'a self,
        asked: &'a PollLinkAsked,
    ) -> BoxFuture<'a, Result<LinkPolled, PluginError>>;
}
