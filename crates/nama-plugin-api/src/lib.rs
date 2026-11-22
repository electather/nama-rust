//! The contract between the Kernel and its Plugins (ADR 0006).
//!
//! A [`Plugin`] takes part in a Server through Extension Points. Each Extension Point is a type
//! declared as one of two kinds:
//!
//! - **Observe** ([`ObservePoint`]): the Kernel tells [`Observer`]s that something happened. Delivery
//!   is asynchronous and fire-and-forget, so an Observer can't change or delay what it observes.
//! - **Provide**: a capability the Server needs, looked up by its trait (for example a title
//!   source, [`dyn TitleSource`](metadata::TitleSource)). A Plugin registers a Provider as an
//!   `Arc` of that trait object. A Server won't start without at least one
//!   [`dyn Authenticator`](auth::Authenticator).
//!
//! When the Kernel enables a Plugin it calls [`Plugin::register`] with a fresh [`Registrar`].
//! Everything registered there belongs to that Plugin, so disabling the Plugin removes all of it.

pub mod auth;
pub mod automation;
pub mod blob_store;
pub mod connector;
pub mod diagnostics_sharer;
pub mod fulfiller;
pub mod home_row;
mod ids;
pub mod job;
pub mod metadata;
pub mod notifier;
pub mod points;
pub mod recogniser;
pub mod search_source;
pub mod segment_source;
pub mod server_link;
pub mod settings;
pub mod similar_source;
pub mod subtitle_source;

/// The Plugin contract's messages (ADR 0041), which the Extension Points take and return, so a
/// Built-in Plugin sees the same types an Installed one does.
pub use nama_plugin_proto::nama::plugin::v1 as contract;

use std::{
    any::{Any, TypeId},
    borrow::Cow,
    error::Error,
    fmt,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::Duration,
};

use serde::{Deserialize, Serialize};

use crate::{
    automation::{Kernel, Permission},
    settings::{PersonalSettingsCheck, Settings, SettingsCheck, SettingsSchema},
};

/// A boxed future that can move between threads, as Extension Point handlers return.
///
/// Write a handler's body as `Box::pin(async move { ... })`.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Why a Plugin's handler or settings check failed.
pub type PluginError = Box<dyn Error + Send + Sync>;

/// The error a [`Fulfiller`](fulfiller::Fulfiller) or a
/// [`SegmentSource`](segment_source::SegmentSource) returns when its service can't act yet, such
/// as Sonarr still adding a Show or a rate limit reached: the Kernel asks again after this long.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TryAgainIn(pub Duration);

impl fmt::Display for TryAgainIn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "try again in {} seconds", self.0.as_secs())
    }
}

impl Error for TryAgainIn {}

/// A Plugin's stable, unique name, such as `log-events`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PluginId(Cow<'static, str>);

impl PluginId {
    /// A Plugin id known at compile time, as Built-in Plugins have.
    #[must_use]
    pub const fn from_static(id: &'static str) -> Self {
        Self(Cow::Borrowed(id))
    }

    /// The id as a string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for PluginId {
    /// A Plugin id read at run time, such as one stored with a Match.
    fn from(id: String) -> Self {
        Self(Cow::Owned(id))
    }
}

impl fmt::Display for PluginId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// What a Plugin says about itself before it is enabled, as an Installed Plugin's package does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    /// The Plugin's unique id.
    pub id: PluginId,
    /// What admins know the Plugin by, such as `TMDB`. Empty if it gives none: its id stands in.
    pub name: String,
    /// What the Plugin does, in one sentence. Empty if it gives none.
    pub description: String,
    /// The Plugin's version, for the log and the admin.
    pub version: String,
    /// The settings the Plugin takes, so an admin UI can render a form for them. The Kernel
    /// enables the Plugin only with settings that fit it.
    pub settings: SettingsSchema,
    /// The Personal settings the Plugin takes: settings each User sets for themself, such as a
    /// topic to be notified on (ADR 0046). The Kernel hands one User's values to the Plugin only
    /// in a call for that User, such as a [`notifier::Recipient::User`].
    pub personal_settings: SettingsSchema,
    /// The Extension Points the Plugin takes part at, such as
    /// [`PointKey::provide::<dyn Notifier>()`](notifier::Notifier), so an admin sees what it does
    /// before it is enabled. The Kernel refuses to enable a Plugin whose
    /// [`Plugin::register`] registers at one it doesn't name.
    pub extension_points: Vec<PointKey>,
    /// The Plugin's logo, an SVG document's text such as `include_str!("../logo.svg")`. The
    /// Kernel drops one over 64 KiB, with a DOCTYPE, or that isn't SVG, logging why; the Plugin
    /// is otherwise as it would be.
    pub logo_svg: Option<String>,
    /// How to set the Plugin up, such as where to get an API key, as short Markdown that admins
    /// see as text, never HTML. Empty if it gives none. The Kernel drops one over 4 KiB, logging
    /// why.
    pub setup_markdown: String,
    /// The Plugin's homepage or docs. Empty if it gives none. The Kernel drops one that isn't an
    /// http or https URL of at most 2 KiB, logging why.
    pub homepage_url: String,
    /// Where the Plugin's source lives, as [`Self::homepage_url`] is.
    pub source_url: String,
    /// The Plugin's licence as an SPDX expression, such as `MIT OR Apache-2.0`. Empty if it gives
    /// none. The Kernel drops one over 128 characters or with characters besides ASCII letters,
    /// digits, spaces and `.-+()`, logging why.
    pub license: String,
    /// The Plugin Permissions it requests, which the Admin grants some or all of when enabling
    /// it; each [`Kernel`] function needs one.
    pub permissions: Vec<Permission>,
    /// The Jobs its [`JobRunner`](job::JobRunner) runs. The Kernel refuses a Plugin naming that
    /// point with none, or with one whose id or schedule `job.proto` doesn't allow.
    pub jobs: Vec<job::Job>,
    /// What its [`TitleSource`](metadata::TitleSource) keeps titles under and whether it searches,
    /// if it provides one. A Library's title source order may add only a namespace some Plugin
    /// keeps or looks titles up by here, enabled or not.
    pub title_source: Option<metadata::SourceManifest>,
}

impl Manifest {
    /// A Manifest for the Plugin `id` that says nothing else about it. Fill in what it does say
    /// with struct update syntax.
    #[must_use]
    pub fn new(id: PluginId) -> Self {
        Self {
            id,
            name: String::new(),
            description: String::new(),
            version: String::new(),
            settings: SettingsSchema::default(),
            personal_settings: SettingsSchema::default(),
            extension_points: Vec::new(),
            logo_svg: None,
            setup_markdown: String::new(),
            homepage_url: String::new(),
            source_url: String::new(),
            license: String::new(),
            permissions: Vec::new(),
            jobs: Vec::new(),
            title_source: None,
        }
    }
}

/// A unit of trusted functionality that plugs into a Server through Extension Points.
///
/// The Kernel contains a panic in any of these methods or in a handler: it logs it and disables
/// the Plugin. The exception is an [`auth::Authenticator`] hashing or checking a password, which
/// runs on a blocking thread: a panic there fails that one sign-in or password change and the
/// Plugin stays enabled, since disabling the only Authenticator on a password anyone can choose
/// would lock every User out. A Plugin and its handlers must not panic when dropped, because the
/// last reference to a handler may be dropped wherever the Kernel was using it.
pub trait Plugin: Send + Sync + 'static {
    /// What the Plugin says about itself, which the Kernel reads before enabling it. Its id must
    /// not change while the Plugin is enabled.
    fn manifest(&self) -> Manifest;

    /// The Plugin's check of its settings, if it has one, which offers the choices for its
    /// [`SettingKind::Choice`](settings::SettingKind::Choice) settings, and for its
    /// [`SettingKind::Choices`](settings::SettingKind::Choices) settings in place of their own.
    /// The Kernel runs it on a fresh instance that isn't enabled.
    fn settings_check(&self) -> Option<&dyn SettingsCheck> {
        None
    }

    /// The Plugin's check of a User's Personal settings, if it has one. The Kernel runs it on a
    /// fresh instance that isn't enabled when a User saves them, and refuses the save if it fails.
    fn personal_settings_check(&self) -> Option<&dyn PersonalSettingsCheck> {
        None
    }

    /// Registers the Plugin's Observers and Providers.
    ///
    /// Called each time the Plugin is enabled, with the Plugin's settings in
    /// [`Registrar::settings`]. Everything registered here is removed again when the Plugin is
    /// disabled.
    ///
    /// # Errors
    ///
    /// When the Plugin can't start with these settings. The Kernel then doesn't enable it, drops
    /// what it registered so far, and shows the error's message to the admin without the
    /// Plugin's secret settings.
    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError>;
}

/// Where a handler runs relative to the other handlers at the same Extension Point.
///
/// Lower runs first. Handlers with equal priority run in order of Plugin id, then in the order
/// one Plugin registered them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Priority(pub i32);

impl Priority {
    /// Runs before handlers at [`Priority::DEFAULT`].
    pub const EARLY: Self = Self(-100);
    /// The priority to use when order doesn't matter.
    pub const DEFAULT: Self = Self(0);
    /// Runs after handlers at [`Priority::DEFAULT`].
    pub const LATE: Self = Self(100);
}

/// An Observe Extension Point: something the Kernel tells Observers has happened.
pub trait ObservePoint: 'static {
    /// The Extension Point's name, for the log.
    const NAME: &'static str;

    /// What Observers are told.
    type Event: Send + Sync + 'static;
}

/// A Plugin's handler that is told something happened and can't change it.
pub trait Observer<P: ObservePoint>: Send + Sync + 'static {
    /// Handles one event. The Kernel doesn't wait for this to finish; an error shows on the
    /// Plugin's page until a later event is handled.
    fn observe<'a>(&'a self, event: &'a P::Event) -> BoxFuture<'a, Result<(), PluginError>>;
}

/// The kind of an Extension Point. Part of [`PointKey`], so one type can't be confused across
/// kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Kind {
    Observe,
    Provide,
}

/// Identifies one Extension Point: its kind and the type that declares it.
#[derive(Debug, Clone, Copy)]
pub struct PointKey {
    kind: Kind,
    type_id: TypeId,
    name: &'static str,
}

impl PointKey {
    /// The key of the Observe Extension Point `P`.
    #[must_use]
    pub fn observe<P: ObservePoint>() -> Self {
        Self {
            kind: Kind::Observe,
            type_id: TypeId::of::<P>(),
            name: P::NAME,
        }
    }

    /// The key of the capability `T`, usually a `dyn Trait`.
    #[must_use]
    pub fn provide<T: ?Sized + 'static>() -> Self {
        Self {
            kind: Kind::Provide,
            type_id: TypeId::of::<T>(),
            name: std::any::type_name::<T>(),
        }
    }

    /// The Extension Point's name, for the log.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }
}

impl PartialEq for PointKey {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind && self.type_id == other.type_id
    }
}

impl Eq for PointKey {}

impl std::hash::Hash for PointKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.kind.hash(state);
        self.type_id.hash(state);
    }
}

/// One handler a Plugin registered at one Extension Point.
///
/// The Kernel reads these out of a [`Registrar`]; Plugins don't need them.
#[derive(Clone)]
pub struct Registration {
    point: PointKey,
    priority: Priority,
    /// An `Arc<dyn Observer<P>>` or `Arc<T>`, by `point`'s kind.
    handler: Arc<dyn Any + Send + Sync>,
}

impl Registration {
    /// The Extension Point the handler is registered at.
    #[must_use]
    pub const fn point(&self) -> PointKey {
        self.point
    }

    /// Where the handler runs among the others at its Extension Point.
    #[must_use]
    pub const fn priority(&self) -> Priority {
        self.priority
    }

    /// The handler, if it is an Observer at `P`.
    #[must_use]
    pub fn observer<P: ObservePoint>(&self) -> Option<Arc<dyn Observer<P>>> {
        self.handler_at(PointKey::observe::<P>())
    }

    /// The handler, if it is a Provider of `T`.
    #[must_use]
    pub fn provider<T: ?Sized + Send + Sync + 'static>(&self) -> Option<Arc<T>> {
        self.handler_at(PointKey::provide::<T>())
    }

    fn handler_at<H: Clone + 'static>(&self, point: PointKey) -> Option<H> {
        if self.point == point {
            self.handler.downcast_ref::<H>().cloned()
        } else {
            None
        }
    }
}

impl fmt::Debug for Registration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Registration")
            .field("point", &self.point)
            .field("priority", &self.priority)
            .finish_non_exhaustive()
    }
}

/// The signal that one enablement of a Plugin is ending: it is disabled, enabled again with
/// other settings or grants, or the Server is stopping.
///
/// The Kernel fires it when it cancels the enablement's scope, before it waits for the tasks in
/// that scope (ADR 0047), so a Plugin can interrupt work the Kernel isn't awaiting, such as a
/// call running on a thread of its own. A Built-in Plugin whose work is all futures the Kernel
/// awaits needs none: the Kernel stops awaiting them. Cloning gives another handle to the same
/// signal.
#[derive(Clone, Default)]
pub struct Stop(Arc<Mutex<Stopping>>);

#[derive(Default)]
struct Stopping {
    stopped: bool,
    callbacks: Vec<Box<dyn FnOnce() + Send>>,
}

impl Stop {
    /// A signal that hasn't fired.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> MutexGuard<'_, Stopping> {
        // A callback runs outside the lock, so a poisoned lock's state is still whole.
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Runs `callback` once when the signal fires, or now if it has. It runs on the thread that
    /// fires it, so it must return quickly; the Kernel contains a panic in it.
    pub fn on_stop(&self, callback: impl FnOnce() + Send + 'static) {
        let mut stopping = self.lock();
        if stopping.stopped {
            drop(stopping);
            callback();
        } else {
            stopping.callbacks.push(Box::new(callback));
        }
    }

    /// Whether the signal has fired.
    #[must_use]
    pub fn is_stopped(&self) -> bool {
        self.lock().stopped
    }

    /// Fires the signal: runs each callback [`Self::on_stop`] took, once. Only the Kernel calls
    /// it; a second call runs nothing.
    pub fn stop(&self) {
        let callbacks = {
            let mut stopping = self.lock();
            stopping.stopped = true;
            std::mem::take(&mut stopping.callbacks)
        };
        for callback in callbacks {
            callback();
        }
    }
}

impl fmt::Debug for Stop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Stop")
            .field("stopped", &self.is_stopped())
            .finish_non_exhaustive()
    }
}

/// Collects what one Plugin registers while it is being enabled.
#[derive(Default)]
pub struct Registrar {
    settings: Settings,
    registrations: Vec<Registration>,
    kernel: Option<Arc<dyn Kernel>>,
    stop: Stop,
}

impl fmt::Debug for Registrar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Registrar")
            .field("settings", &self.settings)
            .field("registrations", &self.registrations)
            .field("kernel", &self.kernel.is_some())
            .field("stop", &self.stop)
            .finish()
    }
}

impl Registrar {
    /// An empty Registrar with no settings. The Kernel makes one for each Plugin it enables.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// An empty Registrar handing the Plugin `settings`, which fit its schema.
    #[must_use]
    pub fn with_settings(settings: Settings) -> Self {
        Self {
            settings,
            ..Self::default()
        }
    }

    /// Hands the Plugin `kernel`, the functions any Plugin may call. Only the Kernel
    /// calls it, when enabling a Plugin.
    #[must_use]
    pub fn with_kernel(self, kernel: Arc<dyn Kernel>) -> Self {
        Self {
            kernel: Some(kernel),
            ..self
        }
    }

    /// Hands the Plugin `stop`, which fires when this enablement ends. Only the Kernel calls it,
    /// when enabling a Plugin.
    #[must_use]
    pub fn with_stop(self, stop: Stop) -> Self {
        Self { stop, ..self }
    }

    /// The signal that this enablement of the Plugin is ending. A Registrar the Kernel didn't hand
    /// one has a signal that never fires.
    #[must_use]
    pub fn stop(&self) -> Stop {
        self.stop.clone()
    }

    /// The functions the Plugin may call on the Kernel, if it is handed them.
    #[must_use]
    pub fn kernel(&self) -> Option<Arc<dyn Kernel>> {
        self.kernel.clone()
    }

    /// The settings the Plugin is being enabled with, checked against its
    /// [`Manifest::settings`] and with its defaults filled in.
    #[must_use]
    pub const fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Registers an Observer at the Observe Extension Point `P`.
    pub fn observe<P: ObservePoint>(&mut self, observer: impl Observer<P>) {
        let observer: Arc<dyn Observer<P>> = Arc::new(observer);
        self.push(PointKey::observe::<P>(), Priority::DEFAULT, observer);
    }

    /// Registers a Provider of the capability `T`, usually a `dyn Trait`.
    ///
    /// Where there are several Providers of `T`, the Kernel uses them in the order the admin set,
    /// then in `priority` order.
    pub fn provide<T: ?Sized + Send + Sync + 'static>(
        &mut self,
        priority: Priority,
        provider: Arc<T>,
    ) {
        self.push(PointKey::provide::<T>(), priority, provider);
    }

    /// Everything registered, in the order it was registered.
    #[must_use]
    pub fn into_registrations(self) -> Vec<Registration> {
        self.registrations
    }

    fn push<H: Send + Sync + 'static>(&mut self, point: PointKey, priority: Priority, handler: H) {
        self.registrations.push(Registration {
            point,
            priority,
            handler: Arc::new(handler),
        });
    }
}
