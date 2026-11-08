//! The plugin registry (ADR 0006): which Plugins are enabled, what each registered, and how the
//! Kernel calls them at Extension Points.

mod builtin;
pub mod installed;

use std::{
    any::Any,
    collections::{BTreeMap, HashMap},
    convert::Infallible,
    fmt,
    future::Future,
    panic::{self, AssertUnwindSafe},
    sync::{Arc, PoisonError, RwLock},
    time::Duration,
};

use futures_util::FutureExt;
use nama_plugin_api::{
    InterceptPoint, ObservePoint, Observer, OnFailure, Plugin, PluginId, PointKey, Priority,
    Registrar, Registration, Verdict,
    settings::{Choices, SettingKind, Settings, SettingsError, SettingsSchema},
};
use thiserror::Error;
use tracing::{error, info, warn};

/// How long a Plugin's settings check may take before it counts as failed.
pub const SETTINGS_CHECK_TIMEOUT: Duration = Duration::from_secs(10);

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
    next_generation: u64,
    /// Each Extension Point's handlers, in the order they run.
    points: HashMap<PointKey, Vec<Entry>>,
    /// The admin's ranking of Providers, by capability. It outlasts Plugins being disabled.
    precedence: HashMap<PointKey, Vec<PluginId>>,
    /// Each Built-in Plugin, enabled or not.
    builtins: BTreeMap<PluginId, Described>,
}

/// A Built-in Plugin, with what it said about itself once at start, so listing runs no Plugin
/// code.
struct Described {
    /// Makes the Plugin anew for each enablement. Keeping one instance instead would hold it past
    /// disabling, so its `Drop` would run uncontained when the registry goes.
    make: fn() -> Arc<dyn Plugin>,
    version: String,
    settings_schema: SettingsSchema,
    checks_settings: bool,
}

struct Enabled {
    // Held so the Plugin lives as long as it is enabled.
    _plugin: Arc<dyn Plugin>,
    /// Tells this enablement apart from earlier ones of the same Plugin, so a call that started
    /// before a disable can't reach or disable the Plugin enabled again since.
    generation: u64,
}

#[derive(Clone)]
struct Entry {
    plugin: PluginId,
    generation: u64,
    /// Position among the Plugin's own registrations, which breaks the last tie.
    index: usize,
    /// The values of the secret settings the Plugin was enabled with, which its errors mustn't
    /// show.
    secrets: Arc<[String]>,
    registration: Registration,
}

impl Entry {
    fn order(&self) -> (Priority, &PluginId, usize) {
        (self.registration.priority(), &self.plugin, self.index)
    }
}

/// A Built-in Plugin, as [`Registry::builtins`] lists it.
#[derive(Debug, Clone)]
pub struct Builtin {
    /// Its unique id.
    pub id: PluginId,
    /// Its version.
    pub version: String,
    /// The settings it takes.
    pub settings_schema: SettingsSchema,
    /// Whether it has a settings check, which [`Registry::check_settings`] runs.
    pub checks_settings: bool,
    /// Whether it is enabled now.
    pub enabled: bool,
}

/// Why a Plugin wasn't disabled: no other enabled Plugin provides a capability it may provide.
#[derive(Debug, Error)]
#[error("plugin {plugin} may be the last one providing {point}")]
pub struct LastProvider {
    /// The Plugin.
    pub plugin: PluginId,
    /// The capability's Extension Point.
    pub point: &'static str,
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
    /// No Built-in Plugin has the id.
    #[error("there is no plugin {0}")]
    Unknown(PluginId),
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

/// Why a Plugin's settings check offered no choices.
#[derive(Debug, Error)]
pub enum CheckError {
    /// No Built-in Plugin has the id.
    #[error("there is no plugin {0}")]
    Unknown(PluginId),
    /// The settings don't fit the Plugin's schema.
    #[error("the settings don't fit the plugin: {0}")]
    InvalidSettings(SettingsError),
    /// The check failed or panicked, or the Plugin has none. The message is in plain words for an
    /// admin, with no secret setting's value in it.
    #[error("{0}")]
    Failed(String),
    /// The check ran past [`SETTINGS_CHECK_TIMEOUT`].
    #[error(
        "{0} didn't finish checking its settings within {seconds} seconds",
        seconds = SETTINGS_CHECK_TIMEOUT.as_secs()
    )]
    TimedOut(PluginId),
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

    /// Enables the Built-in Plugin `id` with `settings`, as [`Registry::enable_with_settings`]
    /// does.
    ///
    /// # Errors
    ///
    /// If no Built-in Plugin has the id, or as [`Registry::enable_with_settings`] fails.
    pub fn enable_builtin(&self, id: &PluginId, settings: Settings) -> Result<(), EnableError> {
        let make = self
            .read(|state| state.builtins.get(id).map(|builtin| builtin.make))
            .ok_or_else(|| EnableError::Unknown(id.clone()))?;
        let plugin = contain_sync(make).map_err(|message| EnableError::Panicked {
            plugin: id.clone(),
            message,
        })?;
        self.enable_with_settings(plugin, settings)
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
        let schema = plugin.settings_schema();
        let settings = settings
            .resolve(&schema)
            .map_err(|error| EnableError::InvalidSettings {
                plugin: id.clone(),
                error,
            })?;
        let secrets: Arc<[String]> = secret_values(&schema, &settings).into();
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
                        secrets: Arc::clone(&secrets),
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
        let Ok(was_enabled) = self.disable_if(id, |_, _| Ok::<_, Infallible>(()));
        was_enabled
    }

    /// Disables the Plugin `id` like [`Registry::disable`], unless no other enabled Plugin
    /// provides `T`, such as the Authenticator a Server can't do without.
    ///
    /// # Errors
    ///
    /// If `id` is the last enabled Plugin providing `T`, which leaves it enabled. Also if `id`
    /// isn't enabled and no Plugin provides `T`: what a Plugin that isn't running provides is
    /// unknown, and it may be one that panicked and would provide `T` after a restart.
    pub fn disable_unless_last_provider<T: ?Sized + 'static>(
        &self,
        id: &PluginId,
    ) -> Result<bool, LastProvider> {
        let point = PointKey::provide::<T>();
        self.disable_if(id, |state, enabled| {
            let entries = state.points.get(&point).map_or(&[][..], Vec::as_slice);
            let provides = enabled.is_none() || entries.iter().any(|entry| entry.plugin == *id);
            let others = entries.iter().any(|entry| entry.plugin != *id);
            if provides && !others {
                return Err(LastProvider {
                    plugin: id.clone(),
                    point: point.name(),
                });
            }
            Ok(())
        })
    }

    /// Disables the Plugin `id` if `check`, given the state and its enablement if it is enabled,
    /// lets it, all under one lock. Returns whether it was enabled.
    fn disable_if<E>(
        &self,
        id: &PluginId,
        check: impl FnOnce(&State, Option<&Enabled>) -> Result<(), E>,
    ) -> Result<bool, E> {
        // What is removed is dropped after the lock is released, since a Plugin's `Drop` is
        // Plugin code.
        let removed = self.write(|state| {
            check(state, state.plugins.get(id))?;
            let Some(plugin) = state.plugins.remove(id) else {
                return Ok(None);
            };
            let mut handlers = Vec::new();
            for entries in state.points.values_mut() {
                handlers.extend(entries.extract_if(.., |entry| entry.plugin == *id));
            }
            state.points.retain(|_, entries| !entries.is_empty());
            Ok(Some((plugin, handlers)))
        })?;
        let was_enabled = removed.is_some();
        if let Err(message) = contain_sync(|| drop(removed)) {
            error!(plugin = %id, message, "plugin panicked while being dropped");
        }
        if was_enabled {
            info!(plugin = %id, "disabled plugin");
        }
        Ok(was_enabled)
    }

    /// Runs the Built-in Plugin `id`'s settings check on `settings`, on a fresh instance whether
    /// it is enabled or not, and returns the choices it offers. The settings are checked against
    /// its schema first, with its Choice settings optional, and its defaults filled in.
    ///
    /// # Errors
    ///
    /// If no Built-in Plugin has the id, it has no check, the settings don't fit, or the check
    /// fails. A check that panics or runs past [`SETTINGS_CHECK_TIMEOUT`] fails without harming the
    /// Server.
    pub async fn check_settings(
        &self,
        id: &PluginId,
        settings: Settings,
    ) -> Result<Choices, CheckError> {
        let (make, schema, checks_settings) = self
            .read(|state| {
                let builtin = state.builtins.get(id)?;
                Some((
                    builtin.make,
                    builtin.settings_schema.clone(),
                    builtin.checks_settings,
                ))
            })
            .ok_or_else(|| CheckError::Unknown(id.clone()))?;
        let no_check = || CheckError::Failed(format!("{id} has no settings check"));
        if !checks_settings {
            return Err(no_check());
        }
        let settings = settings
            .resolve(&with_optional_choices(schema.clone()))
            .map_err(CheckError::InvalidSettings)?;
        let secrets = secret_values(&schema, &settings);
        let scrubbed = |message: String| without_secrets(message, &secrets);
        let panicked = |message: String| {
            let message = scrubbed(message);
            error!(plugin = %id, message, "plugin panicked while checking its settings");
            CheckError::Failed(format!("{id} failed while checking its settings"))
        };
        let plugin = contain_sync(make).map_err(panicked)?;
        // The timeout is inside `contain`, so dropping a check it gave up on (Plugin code) is
        // contained too, as are formatting and dropping the Plugin's error.
        let check = contain(|| async {
            let check = plugin.settings_check()?;
            let checked =
                tokio::time::timeout(SETTINGS_CHECK_TIMEOUT, check.check(&settings)).await;
            Some(checked.map(|checked| checked.map_err(|error| error.to_string())))
        });
        let checked = match check.await {
            Ok(Some(Ok(Ok(choices)))) => Ok(choices),
            Ok(Some(Ok(Err(message)))) => Err(CheckError::Failed(scrubbed(message))),
            Ok(Some(Err(_elapsed))) => Err(CheckError::TimedOut(id.clone())),
            Ok(None) => Err(no_check()),
            Err(message) => Err(panicked(message)),
        };
        if let Err(message) = contain_sync(|| drop(plugin)) {
            error!(plugin = %id, message, "plugin panicked while being dropped");
        }
        checked
    }

    /// The ids of the enabled Plugins, sorted.
    #[must_use]
    pub fn enabled(&self) -> Vec<PluginId> {
        self.read(|state| state.plugins.keys().cloned().collect())
    }

    /// Every Built-in Plugin [`Registry::with_builtins`] knew, enabled or not, sorted by id.
    #[must_use]
    pub fn builtins(&self) -> Vec<Builtin> {
        self.read(|state| {
            state
                .builtins
                .iter()
                .map(|(id, described)| Builtin {
                    id: id.clone(),
                    version: described.version.clone(),
                    settings_schema: described.settings_schema.clone(),
                    checks_settings: described.checks_settings,
                    enabled: state.plugins.contains_key(id),
                })
                .collect()
        })
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

    /// Like [`Registry::call_provider`], for a call whose error Users may see: an `Err`'s message
    /// comes back with the value of each secret setting of the Plugin that answered blanked out.
    pub async fn call_provider_blanking_secrets<T, V, F>(
        &self,
        call: impl Fn(Arc<T>) -> F,
    ) -> Option<Result<V, String>>
    where
        T: ?Sized + Send + Sync + 'static,
        F: Future<Output = Result<V, String>>,
    {
        let (entry, result) = self.call_providers(None, call).await?;
        Some(result.map_err(|message| without_secrets(message, &entry.secrets)))
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
        self.call_providers(None, call)
            .await
            .map(|(entry, result)| (entry.plugin, result))
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
    /// [`Registry::call_provider`] describes, considering only `only`'s if given. Returns the
    /// Provider's entry with its answer.
    async fn call_providers<T, R, F>(
        &self,
        only: Option<&PluginId>,
        call: impl Fn(Arc<T>) -> F,
    ) -> Option<(Entry, R)>
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
                Ok(result) => return Some((entry, result)),
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
        // A stale generation is no error: that enablement is already gone.
        let _stale = self.disable_if(&entry.plugin, |_, enabled| {
            if enabled.is_some_and(|enabled| enabled.generation == entry.generation) {
                Ok(())
            } else {
                Err(())
            }
        });
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

/// `schema` with its Choice settings optional, since checking the settings is how an admin finds
/// what to pick for them.
fn with_optional_choices(mut schema: SettingsSchema) -> SettingsSchema {
    for field in &mut schema.fields {
        field.required &= field.kind != SettingKind::Choice;
    }
    schema
}

/// The value of each secret in `settings`, leaving out blank ones.
fn secret_values(schema: &SettingsSchema, settings: &Settings) -> Vec<String> {
    schema
        .fields
        .iter()
        .filter(|field| field.kind == SettingKind::Secret)
        .filter_map(|field| settings.text(field.key))
        .filter(|secret| !secret.is_empty())
        .map(str::to_owned)
        .collect()
}

/// `message` with each of `secrets` blanked out, since a Plugin's error may quote one, such as in
/// a URL it fetched.
fn without_secrets(mut message: String, secrets: &[String]) -> String {
    for secret in secrets {
        message = message.replace(secret.as_str(), "…");
    }
    message
}

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
