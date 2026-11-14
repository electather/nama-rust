//! How the Kernel calls the enabled Plugins at Extension Points.

use std::{
    collections::BTreeSet,
    fmt,
    future::Future,
    sync::{Arc, PoisonError, RwLock},
};

use nama_plugin_api::{ObservePoint, Observer, PluginId, PointKey};
use nama_proto::nama::v1::PluginKind;
use tracing::{error, info, warn};

use super::{
    Answer, ProviderError, contain, contain_sync, formatted,
    registry::{Enabled, Entry, State},
};
/// How the Kernel calls the Plugins enabled in a [`Registry`](super::Registry): observing,
/// asking Providers and who provides a capability. It can't enable, disable or list Plugins; only a
/// Provider or an Observer that panics disables its own Plugin, as it would through the
/// `Registry`.
///
/// Cloning gives another handle to the same Plugins.
#[derive(Clone, Default)]
pub struct ExtensionPoints {
    state: Arc<RwLock<State>>,
}

impl ExtensionPoints {
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

    /// Asks the Provider of `T` that comes first: those the admin ranked with
    /// [`Registry::set_provider_precedence`](super::Registry::set_provider_precedence) in that
    /// order, then the rest in [`Priority`](nama_plugin_api::Priority) order.
    ///
    /// A Provider that panics is disabled and the next one is asked instead. Returns `None` when
    /// no Provider of `T` is left.
    pub async fn ask_first<T, R, K, E, F>(&self, call: impl Fn(Arc<T>) -> F) -> Option<Answer<R, K>>
    where
        T: ?Sized + Send + Sync + 'static,
        F: Future<Output = Result<R, E>>,
        E: ProviderError<K>,
    {
        self.ask_in_turn(None, call).await
    }

    /// Asks the Plugin `plugin`'s Provider of `T`, such as for something that Provider handed out
    /// earlier. Returns `None` if the Plugin isn't enabled, doesn't provide `T`, or panics, which
    /// disables it.
    pub async fn ask<T, R, K, E, F>(
        &self,
        plugin: &PluginId,
        call: impl Fn(Arc<T>) -> F,
    ) -> Option<Answer<R, K>>
    where
        T: ?Sized + Send + Sync + 'static,
        F: Future<Output = Result<R, E>>,
        E: ProviderError<K>,
    {
        self.ask_in_turn(Some(plugin), call).await
    }

    /// Asks the Providers of `T` in the order [`ExtensionPoints::ask_first`] describes,
    /// considering only `only`'s if given, until one answers without panicking.
    async fn ask_in_turn<T, R, K, E, F>(
        &self,
        only: Option<&PluginId>,
        call: impl Fn(Arc<T>) -> F,
    ) -> Option<Answer<R, K>>
    where
        T: ?Sized + Send + Sync + 'static,
        F: Future<Output = Result<R, E>>,
        E: ProviderError<K>,
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
            match contain(|| formatted(call(provider))).await {
                Ok(result) => {
                    let result = result.map_err(|failure| failure.blanked(&entry.secrets));
                    return Some(Answer {
                        plugin: entry.plugin,
                        result,
                    });
                }
                Err(message) => self.disable_after_panic(&entry, point.name(), &message),
            }
        }
        None
    }

    /// Tells every Provider of `T`, given the id of the Plugin providing it, each as its own task,
    /// without waiting for them.
    ///
    /// A failure is logged, without the Plugin's secret settings, and not retried. A Provider that
    /// panics is disabled. Must be called within a Tokio runtime.
    pub fn tell_each<T, E, F>(&self, call: impl Fn(PluginId, Arc<T>) -> F + Clone + Send + 'static)
    where
        T: ?Sized + Send + Sync + 'static,
        F: Future<Output = Result<(), E>> + Send + 'static,
        E: ProviderError<()> + 'static,
    {
        for entry in self.entries(PointKey::provide::<T>()) {
            if let Some(provider) = entry.registration.provider::<T>() {
                tokio::spawn(self.clone().tell_one(entry, provider, call.clone()));
            }
        }
    }

    /// The task [`ExtensionPoints::tell_each`] runs for one Provider.
    async fn tell_one<T, E, F>(
        self,
        entry: Entry,
        provider: Arc<T>,
        call: impl FnOnce(PluginId, Arc<T>) -> F,
    ) where
        T: ?Sized,
        F: Future<Output = Result<(), E>>,
        E: ProviderError<()>,
    {
        self.tell_if_current(&entry, Arc::clone(&provider), call)
            .await;
        // This task may hold the last handle on a Plugin disabled meanwhile, and its `Drop` is
        // Plugin code.
        let plugin = entry.plugin.clone();
        if let Err(message) = contain_sync(|| drop((provider, entry))) {
            error!(plugin = %plugin, message, "plugin panicked while being dropped");
        }
    }

    /// Calls `call` with `provider`, logging its failure, unless `entry`'s Plugin has been
    /// disabled since [`ExtensionPoints::tell_each`].
    async fn tell_if_current<T, E, F>(
        &self,
        entry: &Entry,
        provider: Arc<T>,
        call: impl FnOnce(PluginId, Arc<T>) -> F,
    ) where
        T: ?Sized,
        F: Future<Output = Result<(), E>>,
        E: ProviderError<()>,
    {
        if !self.is_current(entry) {
            return;
        }
        let point = entry.registration.point().name();
        let called = contain(|| formatted(call(entry.plugin.clone(), provider)));
        match called.await {
            Ok(Ok(())) => {}
            Ok(Err(failure)) => {
                let error = failure.blanked(&entry.secrets).message;
                warn!(plugin = %entry.plugin, point, error, "provider failed");
            }
            Err(message) => self.disable_after_panic(entry, point, &message),
        }
    }

    /// What people know the Plugin `id` by: its name, or its id when it gives none or the Server
    /// has no such Plugin.
    #[must_use]
    pub fn name(&self, id: &PluginId) -> String {
        let named = self.read(|state| match &state.known.get(id)?.plugin {
            Ok((_, manifest)) if !manifest.name.is_empty() => Some(manifest.name.clone()),
            _ => None,
        });
        named.unwrap_or_else(|| id.to_string())
    }

    /// The enabled Plugins that provide a `T`, sorted by id.
    #[must_use]
    pub fn providers<T: ?Sized + 'static>(&self) -> BTreeSet<PluginId> {
        let entries = self.entries(PointKey::provide::<T>());
        entries.into_iter().map(|entry| entry.plugin).collect()
    }

    /// Disables the Plugin `id` if `check`, given the state and its enablement if it is enabled,
    /// lets it, all under one lock. Returns whether it was enabled.
    pub(super) fn disable_if<E>(
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

    fn is_current(&self, entry: &Entry) -> bool {
        self.read(|state| {
            state
                .plugins
                .get(&entry.plugin)
                .is_some_and(|enabled| enabled.generation == entry.generation)
        })
    }

    /// Logs that the handler `entry` panicked at `point` with `message`, disables the enablement
    /// of its Plugin that registered it, and tells [`Registry::crashed`] if it is Installed. A
    /// Plugin enabled again since is left alone.
    fn disable_after_panic(&self, entry: &Entry, point: &str, message: &str) {
        error!(plugin = %entry.plugin, point, message, "plugin crashed; disabling it");
        // A stale generation is no error: that enablement is already gone.
        let disabled = self.disable_if(&entry.plugin, |_, enabled| {
            if enabled.is_some_and(|enabled| enabled.generation == entry.generation) {
                Ok(())
            } else {
                Err(())
            }
        });
        // A Built-in Plugin is only disabled until a restart, since saving the last Authenticator
        // disabled would stop the Server starting.
        let crashed = self.read(|state| {
            let installed = state
                .known
                .get(&entry.plugin)
                .is_some_and(|known| known.kind == PluginKind::PLUGIN_KIND_INSTALLED);
            state.crashed.clone().filter(|_| installed)
        });
        if matches!(disabled, Ok(true))
            && let Some(crashed) = crashed
        {
            // Nothing saves it once the Server stops serving.
            let _stopped = crashed.send(entry.plugin.clone());
        }
    }

    /// A snapshot of the handlers at `point`, in the order they run.
    fn entries(&self, point: PointKey) -> Vec<Entry> {
        self.read(|state| state.points.get(&point).cloned().unwrap_or_default())
    }

    // No Plugin code runs while the lock is held (not even a Plugin's `Drop`), so a poisoned lock's
    // state is still whole.
    pub(super) fn read<T>(&self, f: impl FnOnce(&State) -> T) -> T {
        f(&self.state.read().unwrap_or_else(PoisonError::into_inner))
    }

    pub(super) fn write<T>(&self, f: impl FnOnce(&mut State) -> T) -> T {
        f(&mut self.state.write().unwrap_or_else(PoisonError::into_inner))
    }
}

// Doesn't print the enabled Plugins, so a holder can't list them through `{:?}` either.
impl fmt::Debug for ExtensionPoints {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExtensionPoints").finish_non_exhaustive()
    }
}
