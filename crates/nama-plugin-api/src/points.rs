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

/// Observe: a User's Subtitle or Audio Preference changed (ADR 0035). Only Built-in Plugins
/// observe it, as the Hub Plugin does to sync it (ADR 0092). Each change carries when it was made,
/// since two may reach an Observer in either order.
#[derive(Debug)]
pub struct TrackPreferenceChanged;

impl ObservePoint for TrackPreferenceChanged {
    const NAME: &'static str = "track_preference_changed";
    type Event = TrackPreferenceChange;
}

/// What [`TrackPreferenceChanged`] Observers are told: one of a User's Preferences, as the change
/// left it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackPreferenceChange {
    /// The User's id.
    pub user_id: i64,
    /// The Preference.
    pub preference: TrackPreference,
}

/// A Subtitle or Audio Preference: what describes the Track a User wants, never which Track it
/// was, so any Server matches it against its own (ADR 0035).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrackPreference {
    /// An Audio Preference rather than a Subtitle Preference.
    pub audio: bool,
    /// What it is kept for.
    pub kept_for: TrackPreferenceFor,
    /// What describes the Track wanted. `None` for subtitles turned off, or for the User's own
    /// Audio Preference left to each file's own audio.
    pub like: Option<TrackDescription>,
    /// Full subtitles only over audio in another language, which only the User's own Subtitle
    /// Preference can ask.
    pub foreign_audio_only: bool,
    /// When it changed, in milliseconds since the Unix epoch.
    pub changed_at_ms: i64,
}

/// What describes a Track a User picked, as its file tagged it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrackDescription {
    /// Its language, such as `eng`.
    pub language: Option<String>,
    /// A forced-only subtitle Track.
    pub forced: bool,
    /// A subtitle Track for the deaf and hard of hearing.
    pub sdh: bool,
    /// Its title.
    pub title: Option<String>,
}

/// What a [`TrackPreference`] is kept for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum TrackPreferenceFor {
    /// Everything the User plays: their own, set in Settings.
    #[default]
    User,
    /// The Movie with this External ID, such as `tmdb-348`.
    Movie(String),
    /// The Show with this External ID.
    Show(String),
}
