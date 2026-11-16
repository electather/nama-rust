//! How the Kernel calls the enabled Plugins at Extension Points.

use std::{
    collections::BTreeSet,
    fmt,
    future::Future,
    sync::{Arc, PoisonError, RwLock},
};

use nama_plugin_api::{ObservePoint, Observer, PluginId, PointKey, blob_store::BlobStore};
use nama_proto::nama::v1::PluginKind;
use tracing::{error, info, warn};

use super::{
    Answer, ProviderError, contain, contain_sync, formatted,
    registry::{Enabled, Entry, MakeKernel, State},
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
        let mut entries = self.ranked_entries(point);
        entries.retain(|entry| only.is_none_or(|plugin| entry.plugin == *plugin));
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

    /// The enabled Plugins that serve the blob store point, sorted by id, for the Admin to pick a
    /// store from.
    #[must_use]
    #[cfg_attr(not(test), expect(dead_code, reason = "wired in by bs-switch-api"))]
    pub(crate) fn blob_stores(&self) -> Vec<PluginId> {
        self.providers::<dyn BlobStore>().into_iter().collect()
    }

    /// The enabled Plugins that provide a `T`, in the order [`ExtensionPoints::ask_first`] asks
    /// them, for a caller that picks which to [`ask`](ExtensionPoints::ask) itself.
    #[must_use]
    pub fn ranked<T: ?Sized + 'static>(&self) -> Vec<PluginId> {
        let entries = self.ranked_entries(PointKey::provide::<T>());
        entries.into_iter().map(|entry| entry.plugin).collect()
    }

    /// The handlers at `point` in the order [`ExtensionPoints::ask_first`] asks them.
    fn ranked_entries(&self, point: PointKey) -> Vec<Entry> {
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
        entries
    }

    /// Sets what makes the Kernel's functions each enabled Plugin calls, given its id and the
    /// Permissions granted it. Startup calls it once, when the Kernel's parts exist; until then
    /// every Kernel function answers `refused`. A later call is ignored.
    ///
    /// No Automation is ticked on its schedule until this is called: one enabled before then gets
    /// its first tick when it is, and one enabled after at once. `Registry::tick_now` doesn't wait.
    pub(crate) fn serve_kernel(&self, make: MakeKernel) {
        let first = self
            .read(|state| Arc::clone(&state.kernel))
            .send_if_modified(|served| {
                let first = served.is_none();
                if first {
                    *served = Some(make);
                }
                first
            });
        if !first {
            warn!("the Kernel's functions were already served; keeping the first");
        }
    }

    /// Waits until startup has called [`Self::serve_kernel`], or returns at once if it has.
    pub(super) async fn kernel_served(&self) {
        let mut served = self.read(|state| state.kernel.subscribe());
        // Fails only once every sender is gone, but `self` holds the State that holds it. The
        // `Ref` it returns holds the watch's read lock, so it goes at once.
        drop(served.wait_for(Option::is_some).await);
    }

    /// Spawns `task` in the scope of the Plugin `plugin`'s current enablement, so disabling it
    /// cancels and joins the task before the Plugin is dropped (ADR 0047). Returns false, without
    /// running `task`, if the Plugin isn't enabled. Must be called within a Tokio runtime.
    pub(crate) fn spawn_in(
        &self,
        plugin: &PluginId,
        task: impl Future<Output = ()> + Send + 'static,
    ) -> bool {
        // Under the lock, so a disable either finds the task in the scope it joins or removed the
        // enablement before it was spawned.
        self.read(|state| {
            let Some(enabled) = state.plugins.get(plugin) else {
                return false;
            };
            enabled.scope.spawn(task);
            true
        })
    }

    /// Removes the Plugin `id` and everything it registered if `check`, given the state and its
    /// enablement if it is enabled, lets it, all under one lock. Returns what was removed, which
    /// [`Removed::close`] or [`Removed::close_later`] drops.
    pub(super) fn remove_if<E>(
        &self,
        id: &PluginId,
        check: impl FnOnce(&State, Option<&Enabled>) -> Result<(), E>,
    ) -> Result<Option<Removed>, E> {
        self.write(|state| {
            check(state, state.plugins.get(id))?;
            let Some(enabled) = state.plugins.remove(id) else {
                return Ok(None);
            };
            let mut handlers = Vec::new();
            for entries in state.points.values_mut() {
                handlers.extend(entries.extract_if(.., |entry| entry.plugin == *id));
            }
            state.points.retain(|_, entries| !entries.is_empty());
            Ok(Some(Removed {
                enabled,
                _handlers: handlers,
            }))
        })
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
        let removed = self
            .remove_if(&entry.plugin, |_, enabled| {
                if enabled.is_some_and(|enabled| enabled.generation == entry.generation) {
                    Ok(())
                } else {
                    Err(())
                }
            })
            .ok()
            .flatten();
        let disabled = removed
            .map(|removed| removed.close_later(&entry.plugin))
            .is_some();
        // A Built-in Plugin is only disabled until a restart, since saving the last Authenticator
        // disabled would stop the Server starting.
        let crashed = self.read(|state| {
            let installed = state
                .known
                .get(&entry.plugin)
                .is_some_and(|known| known.kind == PluginKind::PLUGIN_KIND_INSTALLED);
            state.crashed.clone().filter(|_| installed)
        });
        if disabled && let Some(crashed) = crashed {
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

/// What disabling a Plugin removed: its enablement and its handlers, kept until its scope is
/// joined.
pub(super) struct Removed {
    enabled: Enabled,
    // Held so the handlers are dropped with the Plugin, contained.
    _handlers: Vec<Entry>,
}

impl Removed {
    /// Cancels the scope and waits for its tasks, then drops the Plugin.
    pub(super) async fn close(self, id: &PluginId) {
        self.enabled.scope.clone().close().await;
        self.dropped(id);
    }

    /// Cancels the scope at once, and joins it and drops the Plugin in the background.
    pub(super) fn close_later(self, id: &PluginId) {
        self.enabled.scope.cancel();
        let id = id.clone();
        match tokio::runtime::Handle::try_current() {
            Ok(runtime) => drop(runtime.spawn(async move { self.close(&id).await })),
            // Without a runtime no task was spawned in the scope, so there is nothing to join.
            Err(_) => self.dropped(&id),
        }
    }

    /// Drops the Plugin and its handlers, containing a panic in their `Drop`, which is Plugin code.
    fn dropped(self, id: &PluginId) {
        if let Err(message) = contain_sync(|| drop(self)) {
            error!(plugin = %id, message, "plugin panicked while being dropped");
        }
        info!(plugin = %id, "disabled plugin");
    }
}

// Doesn't print the enabled Plugins, so a holder can't list them through `{:?}` either.
impl fmt::Debug for ExtensionPoints {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExtensionPoints").finish_non_exhaustive()
    }
}
