//! The Extension Points the Kernel declares.

pub use nama_plugin_proto::nama::plugin::v1::ObservedEvents;

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

/// Observe: things happened on the Server. Observers are told the kinds of [`Event`] the Plugin
/// contract has, and never a batch with none of them.
///
/// Events arrive in batches. Those about a Library's files come once the database has stored the
/// changes they describe, one batch per write, so a scan doesn't call Observers once per file.
/// Within a batch, events are in the order they happened, but, as at any Observe point, two batches
/// may reach an Observer in either order.
///
/// [`Event`]: nama_plugin_proto::nama::plugin::v1::Event
#[derive(Debug)]
pub struct Events;

impl ObservePoint for Events {
    const NAME: &'static str = "events";
    type Event = ObservedEvents;
}
