//! The Extension Points the Kernel declares.

use crate::ObservePoint;

/// Observe: the Server has started and is listening for clients.
#[derive(Debug)]
pub struct ServerStarted;

impl ObservePoint for ServerStarted {
    const NAME: &'static str = "server_started";
    type Event = ServerStartedEvent;
}

/// What [`ServerStarted`] Observers are told.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerStartedEvent {
    /// The Server's display name.
    pub name: String,
    /// The Server's version.
    pub version: String,
}
