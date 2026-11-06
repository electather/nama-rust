//! The plugin registry (ADR 0006): which Plugins are enabled, what each registered, and how the
//! Kernel calls them at Extension Points.

mod builtin;

use std::{
    any::Any,
    collections::{BTreeMap, HashMap},
    fmt,
    future::Future,
    panic::{self, AssertUnwindSafe},
    sync::{Arc, PoisonError, RwLock},
};

use futures_util::FutureExt;
use nama_plugin_api::{
    InterceptPoint, ObservePoint, Observer, OnFailure, Plugin, PluginId, PointKey, Priority,
    Registrar, Registration, Verdict,
};
use thiserror::Error;
use tracing::{error, info, warn};

/// Every Plugin enabled on this Server and everything they registered.
///
/// Cloning a `Registry` gives another handle to the same one.
#[derive(Clone, Default)]
pub struct Registry {
    state: Arc<RwLock<State>>,
}

#[derive(Default)]
struct State {
    plugins: BTreeMap<PluginId, Arc<dyn Plugin>>,
    /// Each Extension Point's handlers, in the order they run.
    points: HashMap<PointKey, Vec<Entry>>,
    /// The admin's ranking of Providers, by capability. It outlasts Plugins being disabled.
    precedence: HashMap<PointKey, Vec<PluginId>>,
}

/// One handler and the Plugin that registered it.
#[derive(Clone)]
struct Entry {
    plugin: PluginId,
    /// Position among the Plugin's own registrations, which breaks the last tie.
    index: usize,
    registration: Registration,
}

impl Entry {
    fn order(&self) -> (Priority, &PluginId, usize) {
        (self.registration.priority(), &self.plugin, self.index)
    }
}

/// What became of an intercepted action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome<A> {
    /// Go ahead with this action, which Interceptors may have modified.
    Proceed(A),
    /// An Interceptor vetoed the action.
    Vetoed {
        /// The Plugin whose Interceptor vetoed it.
        by: PluginId,
        /// The reason it gave.
        reason: String,
    },
    /// An Interceptor failed at a fail-closed Extension Point, which blocks the action.
    Failed {
        /// The Plugin whose Interceptor failed.
        by: PluginId,
        /// What went wrong.
        error: String,
    },
}

/// Why a Plugin couldn't be enabled.
#[derive(Debug, Error)]
pub enum EnableError {
    /// A Plugin with the same id is already enabled.
    #[error("plugin {0} is already enabled")]
    AlreadyEnabled(PluginId),
    /// The Plugin panicked in [`Plugin::register`].
    #[error("plugin {plugin} panicked while registering: {message}")]
    Panicked {
        /// The Plugin.
        plugin: PluginId,
        /// The panic's message.
        message: String,
    },
}

impl Registry {
    /// A registry with no Plugins enabled.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Enables `plugin`: calls its [`Plugin::register`] and records everything it registers
    /// under its id.
    ///
    /// # Errors
    ///
    /// If a Plugin with the same id is already enabled.
    pub fn enable(&self, plugin: Arc<dyn Plugin>) -> Result<(), EnableError> {
        let id = plugin.id();
        if self.read(|state| state.plugins.contains_key(&id)) {
            return Err(EnableError::AlreadyEnabled(id));
        }
        let registrations = panic::catch_unwind(AssertUnwindSafe(|| {
            let mut registrar = Registrar::new();
            plugin.register(&mut registrar);
            registrar.into_registrations()
        }))
        .map_err(|payload| EnableError::Panicked {
            plugin: id.clone(),
            message: panic_message(&*payload),
        })?;

        let count = registrations.len();
        let version = plugin.version().to_owned();
        self.write(|state| {
            if state.plugins.contains_key(&id) {
                return Err(EnableError::AlreadyEnabled(id.clone()));
            }
            state.plugins.insert(id.clone(), plugin);
            for (index, registration) in registrations.into_iter().enumerate() {
                let entries = state.points.entry(registration.point()).or_default();
                entries.push(Entry {
                    plugin: id.clone(),
                    index,
                    registration,
                });
                entries.sort_by(|a, b| a.order().cmp(&b.order()));
            }
            Ok(())
        })?;
        info!(plugin = %id, %version, registrations = count, "enabled plugin");
        Ok(())
    }

    /// Disables the Plugin `id` and removes everything it registered. Returns whether it was
    /// enabled.
    pub fn disable(&self, id: &PluginId) -> bool {
        let was_enabled = self.write(|state| {
            let was_enabled = state.plugins.remove(id).is_some();
            for entries in state.points.values_mut() {
                entries.retain(|entry| entry.plugin != *id);
            }
            state.points.retain(|_, entries| !entries.is_empty());
            was_enabled
        });
        if was_enabled {
            info!(plugin = %id, "disabled plugin");
        }
        was_enabled
    }

    /// The ids of the enabled Plugins, sorted.
    #[must_use]
    pub fn enabled(&self) -> Vec<PluginId> {
        self.read(|state| state.plugins.keys().cloned().collect())
    }

    /// Tells every Observer at `P` about `event`, without waiting for them.
    ///
    /// Each Observer runs as its own task, so a slow one doesn't hold up the others, and two
    /// events may reach one Observer in either order. Must be called within a Tokio runtime.
    pub fn notify<P: ObservePoint>(&self, event: P::Event) {
        let observers = self.entries(PointKey::observe::<P>());
        if observers.is_empty() {
            return;
        }
        let event = Arc::new(event);
        for entry in observers {
            let Some(observer) = entry.registration.observer::<P>() else {
                continue;
            };
            tokio::spawn(
                self.clone()
                    .deliver(entry.plugin, observer, Arc::clone(&event)),
            );
        }
    }

    /// Tells one Observer about one event.
    async fn deliver<P: ObservePoint>(
        self,
        plugin: PluginId,
        observer: Arc<dyn Observer<P>>,
        event: Arc<P::Event>,
    ) {
        if let Err(message) = contain(|| observer.observe(&event)).await {
            self.panicked(&plugin, P::NAME, &message);
        }
    }

    /// Runs the Interceptors at `P` on `action`, in order, and says whether and how it may go
    /// ahead.
    pub async fn intercept<P: InterceptPoint>(&self, action: P::Action) -> Outcome<P::Action> {
        let mut action = action;
        for entry in self.entries(PointKey::intercept::<P>()) {
            let Some(interceptor) = entry.registration.interceptor::<P>() else {
                continue;
            };
            // Skip Plugins disabled since the snapshot, such as one that panicked earlier in it.
            if !self.is_enabled(&entry.plugin) {
                continue;
            }
            let called =
                tokio::time::timeout(P::TIMEOUT, contain(|| interceptor.intercept(&action))).await;
            let verdict = match called {
                Ok(Ok(verdict)) => verdict,
                Ok(Err(message)) => {
                    self.panicked(&entry.plugin, P::NAME, &message);
                    continue;
                }
                Err(elapsed) => Err(format!("timed out after {:?} ({elapsed})", P::TIMEOUT).into()),
            };
            match verdict {
                Ok(Verdict::Continue) => {}
                Ok(Verdict::Modify(modified)) => action = modified,
                Ok(Verdict::Veto(reason)) => {
                    return Outcome::Vetoed {
                        by: entry.plugin,
                        reason,
                    };
                }
                Err(error) => match P::ON_FAILURE {
                    OnFailure::Allow => {
                        warn!(
                            plugin = %entry.plugin,
                            point = P::NAME,
                            %error,
                            "interceptor failed; letting the action through"
                        );
                    }
                    OnFailure::Block => {
                        warn!(
                            plugin = %entry.plugin,
                            point = P::NAME,
                            %error,
                            "interceptor failed; blocking the action"
                        );
                        return Outcome::Failed {
                            by: entry.plugin,
                            error: error.to_string(),
                        };
                    }
                },
            }
        }
        Outcome::Proceed(action)
    }

    /// Calls `call` with the Provider of `T` that comes first: those the admin ranked with
    /// [`Registry::set_provider_precedence`] in that order, then the rest in [`Priority`] order.
    ///
    /// A Provider that panics is disabled and the next one is called instead. Returns `None` when
    /// no Provider of `T` is left.
    pub async fn call_provider<T, R, F>(&self, call: impl Fn(Arc<T>) -> F) -> Option<R>
    where
        T: ?Sized + Send + Sync + 'static,
        F: Future<Output = R>,
    {
        let point = PointKey::provide::<T>();
        let mut entries = self.entries(point);
        let precedence =
            self.read(|state| state.precedence.get(&point).cloned().unwrap_or_default());
        // Stable, so Providers the admin didn't rank keep their priority order.
        entries.sort_by_key(|entry| {
            precedence
                .iter()
                .position(|ranked| *ranked == entry.plugin)
                .unwrap_or(usize::MAX)
        });
        for entry in entries {
            let Some(provider) = entry.registration.provider::<T>() else {
                continue;
            };
            if !self.is_enabled(&entry.plugin) {
                continue;
            }
            match contain(|| call(provider)).await {
                Ok(result) => return Some(result),
                Err(message) => self.panicked(&entry.plugin, point.name(), &message),
            }
        }
        None
    }

    /// Sets which Plugins' Providers of `T` come first, and in what order, overriding their
    /// priorities. Plugins not in `plugins` follow in priority order. The ranking is kept while a
    /// Plugin in it is disabled or not yet enabled.
    pub fn set_provider_precedence<T: ?Sized + 'static>(&self, plugins: Vec<PluginId>) {
        self.write(|state| state.precedence.insert(PointKey::provide::<T>(), plugins));
    }

    fn is_enabled(&self, id: &PluginId) -> bool {
        self.read(|state| state.plugins.contains_key(id))
    }

    /// Logs that a Plugin panicked at `point` and disables it.
    fn panicked(&self, plugin: &PluginId, point: &str, message: &str) {
        error!(%plugin, point, message, "plugin panicked; disabling it");
        self.disable(plugin);
    }

    /// A snapshot of the handlers at `point`, in the order they run.
    fn entries(&self, point: PointKey) -> Vec<Entry> {
        self.read(|state| state.points.get(&point).cloned().unwrap_or_default())
    }

    // No Plugin code runs while the lock is held, so a poisoned lock's state is still whole.
    fn read<T>(&self, f: impl FnOnce(&State) -> T) -> T {
        f(&self.state.read().unwrap_or_else(PoisonError::into_inner))
    }

    fn write<T>(&self, f: impl FnOnce(&mut State) -> T) -> T {
        f(&mut self.state.write().unwrap_or_else(PoisonError::into_inner))
    }
}

/// Calls a Plugin's handler and awaits it, catching a panic either in the call or while awaiting.
async fn contain<F: Future>(call: impl FnOnce() -> F) -> Result<F::Output, String> {
    let future =
        panic::catch_unwind(AssertUnwindSafe(call)).map_err(|payload| panic_message(&*payload))?;
    AssertUnwindSafe(future)
        .catch_unwind()
        .await
        .map_err(|payload| panic_message(&*payload))
}

/// The message a panic was raised with.
fn panic_message(payload: &(dyn Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_owned()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "(panic without a message)".to_owned()
    }
}

impl fmt::Debug for Registry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Registry")
            .field("enabled", &self.enabled())
            .finish_non_exhaustive()
    }
}
