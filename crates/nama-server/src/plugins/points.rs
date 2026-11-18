//! How the Kernel calls the enabled Plugins at Extension Points.

use std::{
    collections::BTreeSet,
    fmt,
    future::{Future, ready},
    ops::{Deref, DerefMut},
    sync::{Arc, PoisonError, RwLock},
};

use nama_plugin_api::{
    ObservePoint, Observer, PluginError, PluginId, PointKey, blob_store::BlobStore,
};
use nama_proto::nama::v1::PluginKind;
use tracing::{Instrument as _, Span, error, info, warn};

use crate::diagnostics::{self, PLUGIN_SPAN, PLUGIN_TARGET};

use super::{
    Answer, FailureKind, ProviderError, contain_call, contain_sync,
    failing::{Failing, Failures},
    formatted,
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
    /// Which Providers' and Observers' last calls failed, which asking, telling and notifying record
    /// and the Admin is shown.
    failures: Arc<Failures>,
}

impl ExtensionPoints {
    /// Tells every Observer at `P` about `event`, without waiting for them.
    ///
    /// Each Observer runs as its own task, so a slow one doesn't hold up the others, and two
    /// events may reach one Observer in either order. A failure is recorded for the Plugins page,
    /// without the Plugin's secret settings, until the Observer handles an event; one that panics
    /// is disabled. Must be called within a Tokio runtime.
    pub fn notify<P: ObservePoint>(&self, event: P::Event) {
        let observers = self.entries(PointKey::observe::<P>());
        if observers.is_empty() {
            return;
        }
        let event = Arc::new(event);
        let span = diagnostics::following_current(tracing::info_span!(parent: None, "notify"));
        for entry in observers {
            let Some(observer) = entry.registration.observer::<P>() else {
                continue;
            };
            let observer = self.contained(entry.plugin.clone(), observer);
            let delivering = self.clone().deliver(entry, observer, Arc::clone(&event));
            tokio::spawn(delivering.instrument(span.clone()));
        }
    }

    async fn deliver<P: ObservePoint>(
        self,
        entry: Contained<Entry>,
        observer: Contained<Arc<dyn Observer<P>>>,
        event: Arc<P::Event>,
    ) {
        // The Plugin may have been disabled since `notify`.
        if self.is_current(&entry) {
            self.observe_contained::<P>(&entry, &observer, &event).await;
        }
    }

    async fn observe_contained<P: ObservePoint>(
        &self,
        entry: &Entry,
        observer: &Arc<dyn Observer<P>>,
        event: &Arc<P::Event>,
    ) {
        let key = PointKey::observe::<P>();
        let delivered = self.call_contained(entry, P::NAME, || formatted(observer.observe(event)));
        match delivered.await {
            Ok(Ok(())) => self.provider_answered(&entry.plugin, key),
            Ok(Err(failure)) => {
                let error = failure.blanked(&entry.secrets).message;
                self.provider_failed(&entry.plugin, key, &error);
                // An Installed Plugin's Observer already logged it, in `call_method`.
                if !self.read(|state| state.is_installed(&entry.plugin)) {
                    warn!(plugin = %entry.plugin, point = P::NAME, error, "observer failed");
                }
            }
            Err(message) => self.disable_after_panic(entry, P::NAME, &message),
        }
    }

    /// Asks the Provider of `T` that comes first: those the admin ranked with
    /// [`Registry::set_provider_precedence`](super::Registry::set_provider_precedence) in that
    /// order, then the rest in [`Priority`](nama_plugin_api::Priority) order.
    ///
    /// A Provider that panics is disabled and the next one is asked instead. Returns `None` when
    /// no Provider of `T` is left. The answer is recorded for the Plugins page: a failure whose
    /// kind [counts](FailureKind::is_failure), or else an answer.
    pub async fn ask_first<T, R, K, E, F>(&self, call: impl Fn(Arc<T>) -> F) -> Option<Answer<R, K>>
    where
        T: ?Sized + Send + Sync + 'static,
        F: Future<Output = Result<R, E>>,
        E: ProviderError<K>,
        K: FailureKind,
    {
        let asked = self.ask_in_turn(None, call).await;
        self.record::<T, _, _>(&asked);
        asked.answered()
    }

    /// Asks the Plugin `plugin`'s Provider of `T`, such as for something that Provider handed out
    /// earlier. Returns `None` if the Plugin isn't enabled, doesn't provide `T`, or panics, which
    /// disables it. The answer is recorded as [`ExtensionPoints::ask_first`] records it.
    pub async fn ask<T, R, K, E, F>(
        &self,
        plugin: &PluginId,
        call: impl Fn(Arc<T>) -> F,
    ) -> Option<Answer<R, K>>
    where
        T: ?Sized + Send + Sync + 'static,
        F: Future<Output = Result<R, E>>,
        E: ProviderError<K>,
        K: FailureKind,
    {
        self.ask_one(plugin, call).await.answered()
    }

    /// Asks the Plugin `plugin`'s Provider of `T` as [`ExtensionPoints::ask`] does, telling a
    /// Plugin that isn't enabled or doesn't provide `T` from one that panicked.
    pub(super) async fn ask_one<T, R, K, E, F>(
        &self,
        plugin: &PluginId,
        call: impl Fn(Arc<T>) -> F,
    ) -> Asked<R, K>
    where
        T: ?Sized + Send + Sync + 'static,
        F: Future<Output = Result<R, E>>,
        E: ProviderError<K>,
        K: FailureKind,
    {
        let asked = self.ask_in_turn(Some(plugin), call).await;
        self.record::<T, _, _>(&asked);
        asked
    }

    /// What the Plugin `plugin`'s Provider of `T` says of itself, such as which methods it serves,
    /// read inside containment as [`ExtensionPoints::ask`] asks it. Records nothing for the Plugins
    /// page, since it says nothing of whether the Provider's service is failing. `None` as `ask`
    /// gives it.
    pub async fn read_provider<T, R>(&self, plugin: &PluginId, read: impl Fn(&T) -> R) -> Option<R>
    where
        T: ?Sized + Send + Sync + 'static,
    {
        let described = |provider: Arc<T>| ready(Ok::<_, PluginError>(read(&provider)));
        let answer = self.ask_in_turn(Some(plugin), described).await.answered()?;
        answer.result.ok()
    }

    /// Asks the Providers of `T` in the order [`ExtensionPoints::ask_first`] describes,
    /// considering only `only`'s if given, until one answers without panicking. With none
    /// answering, [`Asked::Panicked`] if any of them panicked.
    async fn ask_in_turn<T, R, K, E, F>(
        &self,
        only: Option<&PluginId>,
        call: impl Fn(Arc<T>) -> F,
    ) -> Asked<R, K>
    where
        T: ?Sized + Send + Sync + 'static,
        F: Future<Output = Result<R, E>>,
        E: ProviderError<K>,
    {
        let mut asked = Asked::Absent;
        for entry in self.ranked_entries(PointKey::provide::<T>(), only) {
            let outcome = self.ask_if_current(&entry, &call).await;
            match outcome {
                Asked::Answered(_) => {
                    asked = outcome;
                    break;
                }
                Asked::Panicked => asked = outcome,
                Asked::Absent => {}
            }
        }
        asked
    }

    /// Records `asked`'s answer from a Provider of `T`, as [`ExtensionPoints::ask_first`] says.
    /// A panic records nothing: it disabled the Plugin, which forgets its failures.
    fn record<T: ?Sized + 'static, R, K: FailureKind>(&self, asked: &Asked<R, K>) {
        let Asked::Answered(Answer { plugin, result }) = asked else {
            return;
        };
        let point = PointKey::provide::<T>();
        match result {
            Err(failure) if failure.kind.is_failure() => {
                self.provider_failed(plugin, point, &failure.message);
            }
            _ => self.provider_answered(plugin, point),
        }
    }

    /// Asks `entry`'s Provider of `T` with `call`, unless its Plugin has been disabled since
    /// [`ExtensionPoints::ask_in_turn`] took it.
    async fn ask_if_current<T, R, K, E, F>(
        &self,
        entry: &Entry,
        call: impl Fn(Arc<T>) -> F,
    ) -> Asked<R, K>
    where
        T: ?Sized + Send + Sync + 'static,
        F: Future<Output = Result<R, E>>,
        E: ProviderError<K>,
    {
        let Some(provider) = entry.registration.provider::<T>() else {
            return Asked::Absent;
        };
        if !self.is_current(entry) {
            return Asked::Absent;
        }
        let point = entry.registration.point().name();
        let called = self.call_contained(entry, point, || formatted(call(provider)));
        match called.await {
            Ok(result) => Asked::Answered(Answer {
                plugin: entry.plugin.clone(),
                result: result.map_err(|failure| failure.blanked(&entry.secrets)),
            }),
            Err(message) => {
                self.disable_after_panic(entry, point, &message);
                Asked::Panicked
            }
        }
    }

    /// Tells every Provider of `T`, given the id of the Plugin providing it, each as its own task,
    /// without waiting for them.
    ///
    /// A failure is logged and recorded for the Plugins page, without the Plugin's secret settings,
    /// and not retried; an answer clears the record. A Provider that panics is disabled. Must be
    /// called within a Tokio runtime.
    pub fn tell_each<T, E, F>(&self, call: impl Fn(PluginId, Arc<T>) -> F + Clone + Send + 'static)
    where
        T: ?Sized + Send + Sync + 'static,
        F: Future<Output = Result<(), E>> + Send + 'static,
        E: ProviderError<()> + 'static,
    {
        let span = diagnostics::following_current(tracing::info_span!(parent: None, "tell"));
        for entry in self.entries(PointKey::provide::<T>()) {
            let Some(provider) = entry.registration.provider::<T>() else {
                continue;
            };
            let provider = self.contained(entry.plugin.clone(), provider);
            let telling = self.clone().tell_one(entry, provider, call.clone());
            tokio::spawn(telling.instrument(span.clone()));
        }
    }

    /// The task [`ExtensionPoints::tell_each`] runs for one Provider: calls `call` with `provider`,
    /// recording and logging its failure, unless `entry`'s Plugin has been disabled since.
    async fn tell_one<T, E, F>(
        self,
        entry: Contained<Entry>,
        provider: Contained<Arc<T>>,
        call: impl FnOnce(PluginId, Arc<T>) -> F,
    ) where
        T: ?Sized,
        F: Future<Output = Result<(), E>>,
        E: ProviderError<()>,
    {
        if !self.is_current(&entry) {
            return;
        }
        let key = entry.registration.point();
        let point = key.name();
        let called = self.call_contained(&entry, point, || {
            formatted(call(entry.plugin.clone(), Arc::clone(&provider)))
        });
        match called.await {
            Ok(Ok(())) => self.provider_answered(&entry.plugin, key),
            Ok(Err(failure)) => {
                let error = failure.blanked(&entry.secrets).message;
                self.provider_failed(&entry.plugin, key, &error);
                warn!(plugin = %entry.plugin, point, error, "provider failed");
            }
            Err(message) => self.disable_after_panic(&entry, point, &message),
        }
    }

    /// [`contain_call`] in [`Self::log_span`]. The pending call is Plugin code, so a caller dropped
    /// mid-call drops it contained too.
    async fn call_contained<F: Future>(
        &self,
        entry: &Entry,
        point: &str,
        call: impl FnOnce() -> F,
    ) -> Result<F::Output, String> {
        let called =
            contain_call(&entry.plugin, point, call).instrument(self.log_span(&entry.plugin));
        let mut called = self.contained(entry.plugin.clone(), Box::pin(called));
        called.as_mut().await
    }

    /// Records that the Plugin `plugin`'s Provider or Observer at `point` failed with `message`, a
    /// [`Failure`](super::Failure)'s, and returns its run of failures so far, so a caller can log only
    /// the first of a run. Records nothing and returns `None` once the Plugin is disabled, such as a call its disabling interrupted. The
    /// record lives in memory: a success or disabling the Plugin clears it, and a restart forgets
    /// it.
    pub(crate) fn provider_failed(
        &self,
        plugin: &PluginId,
        point: PointKey,
        message: &str,
    ) -> Option<Failing> {
        // Checked under the state lock, which `remove_if` holds while it forgets, so a call that
        // ends after its Plugin was disabled can't leave a record behind for the next enablement.
        // ponytail: a call that outlives a disable and the next enable still records against the
        // new one; telling them apart would need the enablement's generation in `Answer`.
        self.read(|state| {
            state
                .plugins
                .contains_key(plugin)
                .then(|| self.failures.failed(plugin, point, message))
        })
    }

    /// Records that the Plugin `plugin`'s Provider or Observer at `point` answered, which clears what
    /// [`Self::provider_failed`] recorded.
    pub(crate) fn provider_answered(&self, plugin: &PluginId, point: PointKey) {
        self.failures.answered(plugin, point);
    }

    /// The Plugin `plugin`'s Provider or Observer at `point`, if its last recorded call failed.
    #[must_use]
    pub fn failing(&self, plugin: &PluginId, point: PointKey) -> Option<Failing> {
        self.failures.get(plugin, point)
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
        self.read(|state| {
            let entries = state.points.get(&PointKey::provide::<T>());
            entries
                .into_iter()
                .flatten()
                .map(|entry| entry.plugin.clone())
                .collect()
        })
    }

    /// The enabled Plugins that serve the blob store point, sorted by id, for the Admin to pick a
    /// store from.
    #[must_use]
    pub(crate) fn blob_stores(&self) -> Vec<PluginId> {
        self.providers::<dyn BlobStore>().into_iter().collect()
    }

    /// The enabled Plugins that provide a `T`, in the order [`ExtensionPoints::ask_first`] asks
    /// them, for a caller that picks which to [`ask`](ExtensionPoints::ask) itself.
    #[must_use]
    pub fn ranked<T: ?Sized + 'static>(&self) -> Vec<PluginId> {
        self.read(|state| {
            let entries = state.ranked(PointKey::provide::<T>());
            entries
                .into_iter()
                .map(|entry| entry.plugin.clone())
                .collect()
        })
    }

    /// The handlers at `point` in the order [`ExtensionPoints::ask_first`] asks them, only
    /// `only`'s if given.
    fn ranked_entries(&self, point: PointKey, only: Option<&PluginId>) -> Vec<Contained<Entry>> {
        let entries = self.read(|state| {
            let entries = state.ranked(point).into_iter();
            let kept = entries.filter(|entry| only.is_none_or(|plugin| entry.plugin == *plugin));
            kept.cloned().collect()
        });
        self.contain_each(entries)
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

    /// Runs `enabling` the Plugin `id`, keeping the Plugin Log lines it logs from the start, and
    /// forgets them if it fails, unless a racing enabling won, whose lines they are.
    pub(super) fn keeping_lines<T, E>(
        &self,
        id: &PluginId,
        enabling: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, E> {
        diagnostics::keep_plugin_lines(id);
        let enabled = enabling();
        if enabled.is_err() && !self.read(|state| state.plugins.contains_key(id)) {
            diagnostics::forget_plugin_lines(id);
        }
        enabled
    }

    /// Removes the Plugin `id` and everything it registered if `check`, given the state and its
    /// enablement if it is enabled, lets it, all under one lock. Returns what was removed, which
    /// [`Removed::close`] or [`Removed::close_later`] drops.
    pub(super) fn remove_if<E>(
        &self,
        id: &PluginId,
        check: impl FnOnce(&State, Option<&Enabled>) -> Result<(), E>,
    ) -> Result<Option<Removed>, E> {
        let log_span = self.log_span(id);
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
            // A Plugin enabled again, perhaps with its settings fixed, starts with a clean slate.
            self.failures.forget(id);
            diagnostics::forget_plugin_lines(id);
            Ok(Some(Removed {
                enabled,
                _handlers: handlers,
                log_span,
            }))
        })
    }

    /// The span a call into the Plugin `plugin` runs in, which names it on stdout before each line
    /// a Built-in logs with `tracing` (ADR 0061). None for an Installed Plugin, whose lines carry
    /// its id themselves. A Plugin the Registry doesn't list, enabled directly, runs in-process, so
    /// it counts as Built-in.
    pub(super) fn log_span(&self, plugin: &PluginId) -> Span {
        if self.read(|state| state.is_installed(plugin)) {
            return Span::none();
        }
        builtin_log_span(plugin)
    }

    /// The span a check of the Plugin `plugin`'s settings runs in: as [`Self::log_span`], but
    /// named `check`, since a check has no enablement and so no Plugin Log keeps its lines.
    pub(super) fn check_span(&self, plugin: &PluginId) -> Span {
        if self.read(|state| state.is_installed(plugin)) {
            return Span::none();
        }
        tracing::error_span!(target: PLUGIN_TARGET, "check", plugin = %plugin)
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
        let crashed = self
            .read(|state| (state.crashed.clone()).filter(|_| state.is_installed(&entry.plugin)));
        if disabled && let Some(crashed) = crashed {
            // Nothing saves it once the Server stops serving.
            let _stopped = crashed.send(entry.plugin.clone());
        }
    }

    /// A snapshot of the handlers at `point`, in the order they run.
    fn entries(&self, point: PointKey) -> Vec<Contained<Entry>> {
        let entries = self.read(|state| state.points.get(&point).cloned().unwrap_or_default());
        self.contain_each(entries)
    }

    /// Holds each of `entries` [`Contained`]: once the lock is released, a clone may be the last
    /// handle on a Plugin disabled meanwhile.
    fn contain_each(&self, entries: Vec<Entry>) -> Vec<Contained<Entry>> {
        entries
            .into_iter()
            .map(|entry| self.contained(entry.plugin.clone(), entry))
            .collect()
    }

    fn contained<T>(&self, plugin: PluginId, held: T) -> Contained<T> {
        Contained {
            log_span: self.log_span(&plugin),
            plugin,
            held: Some(held),
        }
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

/// Holds a handle on the Plugin `plugin`, or its pending call, and drops it contained and in the
/// span naming the Plugin however its holder ends, even dropped mid-`.await`: it may be the last
/// handle on a Plugin disabled meanwhile, and its `Drop` is Plugin code. A panic there is logged;
/// it disables nothing, and a dropped call records no failure. Never made under the State lock,
/// since making it reads [`ExtensionPoints::log_span`].
struct Contained<T> {
    log_span: Span,
    plugin: PluginId,
    /// `Some` until dropped.
    held: Option<T>,
}

impl<T> Deref for Contained<T> {
    type Target = T;

    #[expect(
        clippy::expect_used,
        reason = "only `drop` takes it, after which nothing derefs"
    )]
    fn deref(&self) -> &T {
        self.held.as_ref().expect("held until dropped")
    }
}

impl<T> DerefMut for Contained<T> {
    #[expect(
        clippy::expect_used,
        reason = "only `drop` takes it, after which nothing derefs"
    )]
    fn deref_mut(&mut self) -> &mut T {
        self.held.as_mut().expect("held until dropped")
    }
}

impl<T> Drop for Contained<T> {
    fn drop(&mut self) {
        // Needs no runtime, so a task dropped as the runtime shuts down is contained too.
        drop_contained(&self.log_span, &self.plugin, self.held.take());
    }
}

/// Drops `held`, Plugin code of the Plugin `plugin`, contained and in `log_span`, logging a panic.
fn drop_contained(log_span: &Span, plugin: &PluginId, held: impl Sized) {
    if let Err(message) = log_span.in_scope(|| contain_sync(|| drop(held))) {
        error!(plugin = %plugin, message, "plugin panicked while being dropped");
    }
}

/// What asking a Plugin's Provider came to, for a caller that must tell absent from panicked.
pub(super) enum Asked<R, K> {
    /// It answered, or failed without panicking.
    Answered(Answer<R, K>),
    /// The Plugin isn't enabled or doesn't provide the capability, such as one disabled since.
    Absent,
    /// It panicked, which disabled it.
    Panicked,
}

impl<R, K> Asked<R, K> {
    fn answered(self) -> Option<Answer<R, K>> {
        match self {
            Self::Answered(answer) => Some(answer),
            Self::Absent | Self::Panicked => None,
        }
    }
}

impl State {
    /// The handlers at `point` in the order [`ExtensionPoints::ask_first`] asks them.
    fn ranked(&self, point: PointKey) -> Vec<&Entry> {
        let mut entries: Vec<_> = self.points.get(&point).into_iter().flatten().collect();
        let precedence = self.precedence.get(&point).map_or(&[][..], Vec::as_slice);
        // Stable, so Providers the admin didn't rank keep their priority order.
        entries.sort_by_key(|entry| {
            precedence
                .iter()
                .position(|ranked| *ranked == entry.plugin)
                .unwrap_or(usize::MAX)
        });
        entries
    }

    /// Whether the Server lists `plugin` as Installed: one it doesn't list, enabled directly, runs
    /// in-process.
    fn is_installed(&self, plugin: &PluginId) -> bool {
        (self.known.get(plugin))
            .is_some_and(|known| known.kind == PluginKind::PLUGIN_KIND_INSTALLED)
    }
}

/// The span naming the Built-in Plugin `plugin` on stdout, as [`ExtensionPoints::log_span`] gives
/// it for one.
pub(super) fn builtin_log_span(plugin: &PluginId) -> Span {
    // `error`, so a level filter that keeps any of the Plugin's lines keeps the span naming it.
    tracing::error_span!(target: PLUGIN_TARGET, PLUGIN_SPAN, plugin = %plugin)
}

/// What disabling a Plugin removed: its enablement and its handlers, kept until its scope is
/// joined.
pub(super) struct Removed {
    enabled: Enabled,
    // Held so the handlers are dropped with the Plugin, contained.
    _handlers: Vec<Entry>,
    /// What the Plugin's `Stop` and `Drop` run in, as [`ExtensionPoints::log_span`] gives it.
    log_span: Span,
}

impl Removed {
    /// Cancels the scope and waits for its tasks, then drops the Plugin.
    pub(super) async fn close(self, id: &PluginId) {
        let closing = self.enabled.scope.clone().close();
        closing.instrument(self.log_span.clone()).await;
        self.dropped(id);
    }

    /// Cancels the scope at once, and joins it and drops the Plugin in the background.
    pub(super) fn close_later(self, id: &PluginId) {
        self.log_span.in_scope(|| self.enabled.scope.cancel());
        let id = id.clone();
        match tokio::runtime::Handle::try_current() {
            Ok(runtime) => drop(runtime.spawn(async move { self.close(&id).await })),
            // Without a runtime no task was spawned in the scope, so there is nothing to join.
            Err(_) => self.dropped(&id),
        }
    }

    /// Drops the Plugin and its handlers, containing a panic in their `Drop`, which is Plugin code.
    fn dropped(self, id: &PluginId) {
        let log_span = self.log_span.clone();
        drop_contained(&log_span, id, self);
        info!(plugin = %id, "disabled plugin");
    }
}

// Doesn't print the enabled Plugins, so a holder can't list them through `{:?}` either.
impl fmt::Debug for ExtensionPoints {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExtensionPoints").finish_non_exhaustive()
    }
}

#[cfg(test)]
#[path = "points_tests.rs"]
mod tests;
