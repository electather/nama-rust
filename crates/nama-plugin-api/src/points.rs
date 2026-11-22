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

/// Observe: one write changed a User's Watch State. Only Built-in Plugins observe it, as the Hub
/// Plugin does to sync it (ADR 0008). Each change carries when it was made, since two writes may
/// reach an Observer in either order.
#[derive(Debug)]
pub struct WatchStatesChanged;

impl ObservePoint for WatchStatesChanged {
    const NAME: &'static str = "watch_states_changed";
    type Event = WatchStateChanges;
}

/// What [`WatchStatesChanged`] Observers are told: one write's changes to one User's Watch State.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchStateChanges {
    /// The User's id.
    pub user_id: i64,
    /// One per Movie or Episode the write changed.
    pub changes: Vec<WatchStateChange>,
}

/// A Movie's or Episode's Watch State as a write left it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchStateChange {
    /// Whether it is an Episode rather than a Movie, whose TMDB ids are a separate id space.
    pub episode: bool,
    /// Every External ID it is known by, such as `tmdb-348`, the one it is kept under first (ADR
    /// 0013). An Unmatched Item has no Watch State, so it is never told.
    pub ids: Vec<String>,
    /// Whether it was played. Marking it unplayed leaves it unplayed at 0 of 0 milliseconds.
    pub played: bool,
    /// How far in, in milliseconds.
    pub position_ms: u64,
    /// How long it is, in milliseconds; 0 if no player said.
    pub duration_ms: u64,
    /// When it changed, in milliseconds since the Unix epoch.
    pub changed_at_ms: i64,
}
