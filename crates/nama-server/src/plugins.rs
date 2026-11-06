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
    settings::{Settings, SettingsError},
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
    plugins: BTreeMap<PluginId, Enabled>,
    /// The generation the next enabled Plugin gets.
    next_generation: u64,
    /// Each Extension Point's handlers, in the order they run.
    points: HashMap<PointKey, Vec<Entry>>,
    /// The admin's ranking of Providers, by capability. It outlasts Plugins being disabled.
    precedence: HashMap<PointKey, Vec<PluginId>>,
}

/// One enabled Plugin.
struct Enabled {
    // Held so the Plugin lives as long as it is enabled.
    _plugin: Arc<dyn Plugin>,
    /// Tells this enablement apart from earlier ones of the same Plugin, so a call that started
    /// before a disable can't reach or disable the Plugin enabled again since.
    generation: u64,
}

/// One handler and the Plugin that registered it.
#[derive(Clone)]
struct Entry {
    plugin: PluginId,
    /// The generation of the Plugin's enablement that registered it.
    generation: u64,
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
    /// The Plugin panicked in [`Plugin::id`], so it has no id to report.
    #[error("a plugin panicked while giving its id: {message}")]
    IdPanicked {
        /// The panic's message.
        message: String,
    },
    /// The Plugin's settings don't fit the schema it declares.
    #[error("plugin {plugin} has invalid settings: {error}")]
    InvalidSettings {
        /// The Plugin.
        plugin: PluginId,
        /// What is wrong with them.
        error: SettingsError,
    },
    /// The Plugin panicked while being enabled, such as in [`Plugin::register`].
    #[error("plugin {plugin} panicked while being enabled: {message}")]
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

    /// Enables `plugin` with no settings: calls its [`Plugin::register`] and records everything
    /// it registers under its id.
    ///
    /// # Errors
    ///
    /// If a Plugin with the same id is already enabled, the Plugin requires a setting, or the
    /// Plugin panics.
    pub fn enable(&self, plugin: Arc<dyn Plugin>) -> Result<(), EnableError> {
        self.enable_with_settings(plugin, Settings::default())
    }

    /// Enables `plugin` with `settings`, once they are checked against its
    /// [`Plugin::settings_schema`] and its defaults are filled in.
    ///
    /// # Errors
    ///
    /// If a Plugin with the same id is already enabled, the settings don't fit the Plugin's
    /// schema, or the Plugin panics.
    pub fn enable_with_settings(
        &self,
        plugin: Arc<dyn Plugin>,
        settings: Settings,
    ) -> Result<(), EnableError> {
        // Everything a Plugin does here (its `id`, `version`, `register` and `Drop`) is Plugin
        // code, so a panic in any of it is contained.
        let id = match contain_sync(|| plugin.id()) {
            Ok(id) => id,
            Err(message) => {
                // A panic dropping it is moot: the Plugin is already refused.
                let _dropped = contain_sync(|| drop(plugin));
                return Err(EnableError::IdPanicked { message });
            }
        };
        contain_sync(|| self.enable_as(&id, plugin, settings)).unwrap_or_else(|message| {
            Err(EnableError::Panicked {
                plugin: id,
                message,
            })
        })
    }

    /// Enables `plugin`, whose id is `id`. Plugin code runs here, so only call it from within
    /// [`contain_sync`].
    fn enable_as(
        &self,
        id: &PluginId,
        plugin: Arc<dyn Plugin>,
        settings: Settings,
    ) -> Result<(), EnableError> {
        if self.read(|state| state.plugins.contains_key(id)) {
            return Err(EnableError::AlreadyEnabled(id.clone()));
        }
        let settings = settings
            .resolve(&plugin.settings_schema())
            .map_err(|error| EnableError::InvalidSettings {
                plugin: id.clone(),
                error,
            })?;
        let mut registrar = Registrar::with_settings(settings);
        plugin.register(&mut registrar);
        let registrations = registrar.into_registrations();
        let count = registrations.len();
        let version = plugin.version().to_owned();
        // Plugin values the lock refuses are dropped after it is released, since a Plugin's `Drop`
        // is Plugin code.
        let refused = self.write(|state| {
            if state.plugins.contains_key(id) {
                return Some((plugin, registrations));
            }
            let generation = state.next_generation;
            state.next_generation += 1;
            state.plugins.insert(
                id.clone(),
                Enabled {
                    _plugin: plugin,
                    generation,
                },
            );
            for (index, registration) in registrations.into_iter().enumerate() {
                state
                    .points
                    .entry(registration.point())
                    .or_default()
                    .push(Entry {
                        plugin: id.clone(),
                        generation,
                        index,
                        registration,
                    });
            }
            // Once, after every push. A list nothing was pushed to is already sorted, which the
            // stable sort sees in one pass.
            for entries in state.points.values_mut() {
                entries.sort_by(|a, b| a.order().cmp(&b.order()));
            }
            None
        });
        if refused.is_some() {
            return Err(EnableError::AlreadyEnabled(id.clone()));
        }
        info!(plugin = %id, %version, registrations = count, "enabled plugin");
        Ok(())
    }

    /// Disables the Plugin `id` and removes everything it registered. Returns whether it was
    /// enabled.
    #[expect(
        clippy::must_use_candidate,
        reason = "callers often disable without caring whether the Plugin was enabled"
    )]
    pub fn disable(&self, id: &PluginId) -> bool {
        self.disable_generation(id, None)
    }

    /// Disables the Plugin `id`, but only its enablement `generation` if one is given.
    fn disable_generation(&self, id: &PluginId, generation: Option<u64>) -> bool {
        // What is removed is dropped after the lock is released, since a Plugin's `Drop` is
        // Plugin code.
        let removed = self.write(|state| {
            let current = state.plugins.get(id)?.generation;
            if generation.is_some_and(|generation| generation != current) {
                return None;
            }
            let plugin = state.plugins.remove(id)?;
            let mut handlers = Vec::new();
            for entries in state.points.values_mut() {
                handlers.extend(entries.extract_if(.., |entry| entry.plugin == *id));
            }
            state.points.retain(|_, entries| !entries.is_empty());
            Some((plugin, handlers))
        });
        let was_enabled = removed.is_some();
        if let Err(message) = contain_sync(|| drop(removed)) {
            error!(plugin = %id, message, "plugin panicked while being dropped");
        }
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
            tokio::spawn(self.clone().deliver(entry, observer, Arc::clone(&event)));
        }
    }

    /// Tells one Observer about one event.
    async fn deliver<P: ObservePoint>(
        self,
        entry: Entry,
        observer: Arc<dyn Observer<P>>,
        event: Arc<P::Event>,
    ) {
        // The Plugin may have been disabled since `notify`.
        if !self.is_current(&entry) {
            return;
        }
        if let Err(message) = contain(|| observer.observe(&event)).await {
            self.disable_after_panic(&entry, P::NAME, &message);
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
            if !self.is_current(&entry) {
                continue;
            }
            let called =
                tokio::time::timeout(P::TIMEOUT, contain(|| interceptor.intercept(&action))).await;
            let verdict = match called {
                Ok(Ok(verdict)) => verdict,
                Ok(Err(message)) => {
                    self.disable_after_panic(&entry, P::NAME, &message);
                    continue;
                }
                Err(elapsed) => Err(format!("timed out after {:?} ({elapsed})", P::TIMEOUT).into()),
            };
            let verdict = verdict.or_else(|error| {
                warn!(
                    plugin = %entry.plugin,
                    point = P::NAME,
                    on_failure = ?P::ON_FAILURE,
                    %error,
                    "interceptor failed"
                );
                match P::ON_FAILURE {
                    OnFailure::Allow => Ok(Verdict::Continue),
                    OnFailure::Block => Err(error),
                }
            });
            match verdict {
                Ok(Verdict::Continue) => {}
                Ok(Verdict::Modify(modified)) => action = modified,
                Ok(Verdict::Veto(reason)) => {
                    return Outcome::Vetoed {
                        by: entry.plugin,
                        reason,
                    };
                }
                Err(error) => {
                    return Outcome::Failed {
                        by: entry.plugin,
                        error: error.to_string(),
                    };
                }
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
        self.call_providers(None, call)
            .await
            .map(|(_, result)| result)
    }

    /// Like [`Registry::call_provider`], but also says which Plugin's Provider answered, so the
    /// caller can go back to it with [`Registry::call_provider_of`].
    pub async fn call_provider_with_id<T, R, F>(
        &self,
        call: impl Fn(Arc<T>) -> F,
    ) -> Option<(PluginId, R)>
    where
        T: ?Sized + Send + Sync + 'static,
        F: Future<Output = R>,
    {
        self.call_providers(None, call).await
    }

    /// Calls `call` with the Plugin `plugin`'s Provider of `T`, such as to fetch something that
    /// Provider handed out earlier. Returns `None` if the Plugin isn't enabled, doesn't provide
    /// `T`, or panics, which disables it.
    pub async fn call_provider_of<T, R, F>(
        &self,
        plugin: &PluginId,
        call: impl Fn(Arc<T>) -> F,
    ) -> Option<R>
    where
        T: ?Sized + Send + Sync + 'static,
        F: Future<Output = R>,
    {
        self.call_providers(Some(plugin), call)
            .await
            .map(|(_, result)| result)
    }

    /// Calls `call` with the first Provider of `T` that answers without panicking, in the order
    /// [`Registry::call_provider`] describes, considering only `only`'s if given.
    async fn call_providers<T, R, F>(
        &self,
        only: Option<&PluginId>,
        call: impl Fn(Arc<T>) -> F,
    ) -> Option<(PluginId, R)>
    where
        T: ?Sized + Send + Sync + 'static,
        F: Future<Output = R>,
    {
        let point = PointKey::provide::<T>();
        let mut entries = self.entries(point);
        entries.retain(|entry| only.is_none_or(|plugin| entry.plugin == *plugin));
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
            if !self.is_current(&entry) {
                continue;
            }
            match contain(|| call(provider)).await {
                Ok(result) => return Some((entry.plugin, result)),
                Err(message) => self.disable_after_panic(&entry, point.name(), &message),
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

    /// Whether the enablement that registered `entry` is still the enabled one.
    fn is_current(&self, entry: &Entry) -> bool {
        self.read(|state| {
            state
                .plugins
                .get(&entry.plugin)
                .is_some_and(|enabled| enabled.generation == entry.generation)
        })
    }

    /// Logs that the handler `entry` panicked at `point` and disables the enablement of its Plugin
    /// that registered it. A Plugin enabled again since is left alone.
    fn disable_after_panic(&self, entry: &Entry, point: &str, message: &str) {
        error!(plugin = %entry.plugin, point, message, "plugin panicked; disabling it");
        self.disable_generation(&entry.plugin, Some(entry.generation));
    }

    /// A snapshot of the handlers at `point`, in the order they run.
    fn entries(&self, point: PointKey) -> Vec<Entry> {
        self.read(|state| state.points.get(&point).cloned().unwrap_or_default())
    }

    // No Plugin code runs while the lock is held (not even a Plugin's `Drop`), so a poisoned lock's
    // state is still whole.
    fn read<T>(&self, f: impl FnOnce(&State) -> T) -> T {
        f(&self.state.read().unwrap_or_else(PoisonError::into_inner))
    }

    fn write<T>(&self, f: impl FnOnce(&mut State) -> T) -> T {
        f(&mut self.state.write().unwrap_or_else(PoisonError::into_inner))
    }
}

/// Calls Plugin code that isn't async, catching a panic.
pub(crate) fn contain_sync<T>(call: impl FnOnce() -> T) -> Result<T, String> {
    panic::catch_unwind(AssertUnwindSafe(call)).map_err(|payload| panic_message(&*payload))
}

/// Calls a Plugin's handler and awaits it, catching a panic either in the call or while awaiting.
async fn contain<F: Future>(call: impl FnOnce() -> F) -> Result<F::Output, String> {
    let future = contain_sync(call)?;
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
