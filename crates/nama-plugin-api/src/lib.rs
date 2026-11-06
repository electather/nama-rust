//! The contract between the Kernel and its Plugins (ADR 0006).
//!
//! A [`Plugin`] takes part in a Server through Extension Points. Each Extension Point is a type
//! declared as one of three kinds:
//!
//! - **Observe** ([`ObservePoint`]): the Kernel tells [`Observer`]s that something happened. Delivery
//!   is asynchronous and fire-and-forget, so an Observer can't change or delay what it observes.
//! - **Intercept** ([`InterceptPoint`]): before an action takes effect, its [`Interceptor`]s run one
//!   after another in [`Priority`] order. Each returns a [`Verdict`]: let the action continue,
//!   modify it, or veto it. Each Intercept Extension Point declares, as its [`OnFailure`], whether
//!   an Interceptor that fails lets the action through or blocks it.
//! - **Provide**: a capability the Server needs, looked up by its trait (for example
//!   [`dyn MetadataProvider`](metadata::MetadataProvider)). A Plugin registers a Provider as an
//!   `Arc` of that trait object.
//!
//! When the Kernel enables a Plugin it calls [`Plugin::register`] with a fresh [`Registrar`].
//! Everything registered there belongs to that Plugin, so disabling the Plugin removes all of it.
//!
//! Built-in Plugins depend on this crate (and `nama-proto`), never on the Server itself, so the
//! compiler keeps them out of Kernel internals.

pub mod metadata;
pub mod points;
pub mod settings;

use std::{
    any::{Any, TypeId},
    borrow::Cow,
    error::Error,
    fmt,
    future::Future,
    pin::Pin,
    sync::Arc,
    time::Duration,
};

use crate::settings::{Settings, SettingsSchema};

/// A boxed future that can move between threads, as Extension Point handlers return.
///
/// Write a handler's body as `Box::pin(async move { ... })`.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Why an Interceptor failed, for the log.
pub type PluginError = Box<dyn Error + Send + Sync>;

/// A Plugin's stable, unique name, such as `log-events`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

impl fmt::Display for PluginId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A unit of trusted functionality that plugs into a Server through Extension Points.
///
/// The Kernel contains a panic in any of these methods or in a handler: it logs it and disables
/// the Plugin. A Plugin and its handlers must not panic when dropped, because the last reference
/// to a handler may be dropped wherever the Kernel was using it.
pub trait Plugin: Send + Sync + 'static {
    /// The Plugin's unique id. It must not change while the Plugin is enabled.
    fn id(&self) -> PluginId;

    /// The Plugin's version, for the log and the admin.
    fn version(&self) -> &str;

    /// The settings the Plugin takes, so an admin UI can render a form for them. The Kernel
    /// enables the Plugin only with settings that fit it.
    fn settings_schema(&self) -> SettingsSchema {
        SettingsSchema::default()
    }

    /// Registers the Plugin's Observers, Interceptors and Providers.
    ///
    /// Called each time the Plugin is enabled, with the Plugin's settings in
    /// [`Registrar::settings`]. Everything registered here is removed again when the Plugin is
    /// disabled.
    fn register(&self, registrar: &mut Registrar);
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
    /// Handles one event. The Kernel doesn't wait for this to finish.
    fn observe<'a>(&'a self, event: &'a P::Event) -> BoxFuture<'a, ()>;
}

/// What an Intercept Extension Point does with the action when one of its Interceptors fails
/// (returns an error or times out).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OnFailure {
    /// Fail-open: skip the failed Interceptor and let the action through.
    Allow,
    /// Fail-closed: block the action.
    Block,
}

/// An Intercept Extension Point: an action that Interceptors may let continue, modify or veto
/// before it takes effect.
pub trait InterceptPoint: 'static {
    /// The Extension Point's name, for the log.
    const NAME: &'static str;

    /// Whether a failing Interceptor lets the action through or blocks it.
    const ON_FAILURE: OnFailure;

    /// How long one Interceptor may take before it counts as failed.
    const TIMEOUT: Duration = Duration::from_secs(5);

    /// The action being intercepted.
    type Action: Send + Sync + 'static;
}

/// What an Interceptor decides about an action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict<A> {
    /// Let the action continue unchanged.
    Continue,
    /// Replace the action. Later Interceptors see the replacement.
    Modify(A),
    /// Stop the action, for the given reason. Later Interceptors don't run.
    Veto(String),
}

/// A Plugin's handler that can let an action continue, modify it, or veto it before it takes
/// effect.
pub trait Interceptor<P: InterceptPoint>: Send + Sync + 'static {
    /// Decides about one action.
    ///
    /// # Errors
    ///
    /// When the Interceptor can't decide. The Extension Point's [`OnFailure`] says what happens
    /// to the action then.
    fn intercept<'a>(
        &'a self,
        action: &'a P::Action,
    ) -> BoxFuture<'a, Result<Verdict<P::Action>, PluginError>>;
}

/// The kind of an Extension Point. Part of [`PointKey`], so one type can't be confused across
/// kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Kind {
    Observe,
    Intercept,
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

    /// The key of the Intercept Extension Point `P`.
    #[must_use]
    pub fn intercept<P: InterceptPoint>() -> Self {
        Self {
            kind: Kind::Intercept,
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
    /// An `Arc<dyn Observer<P>>`, `Arc<dyn Interceptor<P>>` or `Arc<T>`, by `point`'s kind.
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

    /// The handler, if it is an Interceptor at `P`.
    #[must_use]
    pub fn interceptor<P: InterceptPoint>(&self) -> Option<Arc<dyn Interceptor<P>>> {
        self.handler_at(PointKey::intercept::<P>())
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

/// Collects what one Plugin registers while it is being enabled.
#[derive(Debug, Default)]
pub struct Registrar {
    settings: Settings,
    registrations: Vec<Registration>,
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
            registrations: Vec::new(),
        }
    }

    /// The settings the Plugin is being enabled with, checked against its
    /// [`Plugin::settings_schema`] and with its defaults filled in.
    #[must_use]
    pub const fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Registers an Observer at the Observe Extension Point `P`.
    pub fn observe<P: ObservePoint>(&mut self, observer: impl Observer<P>) {
        let observer: Arc<dyn Observer<P>> = Arc::new(observer);
        self.push(PointKey::observe::<P>(), Priority::DEFAULT, observer);
    }

    /// Registers an Interceptor at the Intercept Extension Point `P`.
    pub fn intercept<P: InterceptPoint>(
        &mut self,
        priority: Priority,
        interceptor: impl Interceptor<P>,
    ) {
        let interceptor: Arc<dyn Interceptor<P>> = Arc::new(interceptor);
        self.push(PointKey::intercept::<P>(), priority, interceptor);
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
