//! The Registry: which Plugins are enabled, and enabling, disabling and listing them.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    convert::Infallible,
    fmt,
    ops::Deref,
    sync::Arc,
};

use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    Registration, Stop,
    automation::{
        DeleteAsked, DeleteStarted, Kernel, KernelRefused, Libraries, LibrariesAsked, Permission,
        StoreEntry, StoreKey, StoreValue, Stored, Titles, TitlesAsked, Users, UsersAsked,
        WatchStates, WatchStatesAsked,
    },
    fulfiller::{Fulfiller, Movies, Shows},
    settings::{Choices, SettingKind, Settings, SettingsSchema},
};
use nama_proto::nama::v1::PluginKind;
use tokio::sync::{mpsc, watch};
use tracing::{error, info};

use super::{
    CheckError, CheckTimedOut, EnableError, ExtensionPoints, LastProvider, Listed,
    SETTINGS_CHECK_TIMEOUT, contain_call, contain_sync, scope::Scope, secret_values, tick,
    without_secrets,
};
/// Every Plugin enabled on this Server and everything they registered.
///
/// Only Plugin settings and startup hold one (`crates/nama-server/AGENTS.md`, "One owner for
/// Plugin changes"). Cloning a `Registry` gives another handle to the same one.
#[derive(Clone, Default)]
pub struct Registry {
    points: ExtensionPoints,
}

// A Registry calls Plugins as every other module does; only it also changes them.
impl Deref for Registry {
    type Target = ExtensionPoints;

    fn deref(&self) -> &ExtensionPoints {
        &self.points
    }
}

#[derive(Default)]
pub(super) struct State {
    pub(super) plugins: BTreeMap<PluginId, Enabled>,
    next_generation: u64,
    /// Each Extension Point's handlers, in the order they run.
    pub(super) points: HashMap<PointKey, Vec<Entry>>,
    /// The admin's ranking of Providers, by capability. It outlasts Plugins being disabled.
    pub(super) precedence: HashMap<PointKey, Vec<PluginId>>,
    /// Each Plugin the Server has, Built-in or Installed, enabled or not.
    pub(super) known: BTreeMap<PluginId, Described>,
    /// Where [`Registry::crashed`] sends the Installed Plugins it disables for crashing.
    pub(super) crashed: Option<mpsc::UnboundedSender<PluginId>>,
    /// What makes the Kernel's functions for a Plugin and the Permissions granted it, once startup
    /// sets it with [`ExtensionPoints::serve_kernel`]. A watch, so a tick can wait for it.
    pub(super) kernel: Arc<watch::Sender<Option<MakeKernel>>>,
    /// What makes each in-process Plugin [`Registry::list_as_installed`] lists as Installed.
    #[cfg(feature = "testing")]
    pub(super) as_installed: Vec<fn() -> Arc<dyn Plugin>>,
}

pub(super) type Make = Arc<dyn Fn() -> Arc<dyn Plugin> + Send + Sync>;

/// What makes the Kernel's functions for a Plugin, given its id and the Permissions granted it.
pub(crate) type MakeKernel =
    Arc<dyn Fn(PluginId, HashSet<Permission>) -> Arc<dyn Kernel> + Send + Sync>;

/// A Plugin the Server has, with what it said about itself when the Server read it, so listing
/// runs no Plugin code.
pub(super) struct Described {
    pub(super) kind: PluginKind,
    /// What makes the Plugin anew for each enablement, kept beside its Manifest, or why the Server
    /// refused it. Keeping one instance instead would hold it past disabling, so its `Drop` would
    /// run uncontained when the registry goes.
    pub(super) plugin: Result<(Make, Manifest), String>,
    pub(super) checks_settings: bool,
}

pub(super) struct Enabled {
    // Held so the Plugin lives as long as it is enabled.
    _plugin: Arc<dyn Plugin>,
    /// Tells this enablement apart from earlier ones of the same Plugin, so a call that started
    /// before a disable can't reach or disable the Plugin enabled again since.
    pub(super) generation: u64,
    /// The Kernel tasks started for this enablement, joined before the Plugin is dropped.
    pub(super) scope: Scope,
}

#[derive(Clone)]
pub(super) struct Entry {
    pub(super) plugin: PluginId,
    pub(super) generation: u64,
    /// Position among the Plugin's own registrations, which breaks the last tie.
    index: usize,
    /// The values of the secret settings the Plugin was enabled with, which its errors mustn't
    /// show.
    pub(super) secrets: Arc<[String]>,
    pub(super) registration: Registration,
}

impl Entry {
    fn order(&self) -> (Priority, &PluginId, usize) {
        (self.registration.priority(), &self.plugin, self.index)
    }
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
    /// Plugin refuses to start or panics.
    pub fn enable(&self, plugin: Arc<dyn Plugin>) -> Result<(), EnableError> {
        self.enable_with_settings(plugin, Settings::default())
    }

    /// Enables `plugin` with `settings`, once they are checked against its
    /// [`Manifest::settings`] and its defaults are filled in.
    ///
    /// # Errors
    ///
    /// If a Plugin with the same id is already enabled, the settings don't fit the Plugin's
    /// schema, or the Plugin refuses to start or panics.
    pub fn enable_with_settings(
        &self,
        plugin: Arc<dyn Plugin>,
        settings: Settings,
    ) -> Result<(), EnableError> {
        self.enable_granting(plugin, settings, HashSet::new())
    }

    /// [`Registry::enable_with_settings`], granting the Plugin those of `granted` its Manifest
    /// requests.
    fn enable_granting(
        &self,
        plugin: Arc<dyn Plugin>,
        settings: Settings,
        granted: HashSet<Permission>,
    ) -> Result<(), EnableError> {
        // Everything a Plugin does here (its `manifest`, `register` and `Drop`) is Plugin code, so
        // a panic in any of it is contained.
        let manifest = match contain_sync(|| plugin.manifest()) {
            Ok(manifest) => manifest,
            Err(message) => {
                // A panic dropping it is moot: the Plugin is already refused.
                let _dropped = contain_sync(|| drop(plugin));
                return Err(EnableError::ManifestPanicked { message });
            }
        };
        contain_sync(|| self.enable_as(&manifest, plugin, settings, granted)).unwrap_or_else(
            |message| {
                Err(EnableError::Panicked {
                    plugin: manifest.id,
                    message,
                })
            },
        )
    }

    /// Enables the Plugin `id` that [`Registry::plugins`] lists with `settings`, as
    /// [`Registry::enable_with_settings`] does.
    ///
    /// # Errors
    ///
    /// If no Plugin has the id, the Server refused it, or as [`Registry::enable_with_settings`]
    /// fails, such as the Plugin refusing to start.
    pub fn enable_by_id(&self, id: &PluginId, settings: Settings) -> Result<(), EnableError> {
        self.enable_by_id_granting(id, settings, HashSet::new())
    }

    /// [`Registry::enable_by_id`], granting the Plugin those of `granted` its Manifest requests.
    ///
    /// # Errors
    ///
    /// As [`Registry::enable_by_id`].
    pub fn enable_by_id_granting(
        &self,
        id: &PluginId,
        settings: Settings,
        granted: HashSet<Permission>,
    ) -> Result<(), EnableError> {
        let make = self
            .read(|state| {
                let known = state.known.get(id)?;
                Some(
                    known
                        .plugin
                        .as_ref()
                        .map(|(make, _)| Arc::clone(make))
                        .map_err(Clone::clone),
                )
            })
            .ok_or_else(|| EnableError::Unknown(id.clone()))?
            .map_err(|reason| EnableError::Refused {
                plugin: id.clone(),
                reason,
            })?;
        let plugin = contain_sync(|| make()).map_err(|message| EnableError::Panicked {
            plugin: id.clone(),
            message,
        })?;
        self.enable_granting(plugin, settings, granted)
    }

    /// Enables `plugin`, whose Manifest is `manifest`. Plugin code runs here, so only call it from
    /// within [`contain_sync`].
    fn enable_as(
        &self,
        manifest: &Manifest,
        plugin: Arc<dyn Plugin>,
        settings: Settings,
        granted: HashSet<Permission>,
    ) -> Result<(), EnableError> {
        let id = &manifest.id;
        if self.read(|state| state.plugins.contains_key(id)) {
            return Err(EnableError::AlreadyEnabled(id.clone()));
        }
        refuse_bad_personal_settings(manifest)?;
        let schema = &manifest.settings;
        let settings = settings
            .resolve(schema)
            .map_err(|error| EnableError::InvalidSettings {
                plugin: id.clone(),
                error,
            })?;
        let secrets: Arc<[String]> = secret_values(schema, &settings).into();
        let mut registrar = self.registrar_for(manifest, settings, granted);
        // The error and the half-filled registrar are Plugin values, dropped here inside the
        // containment the caller runs this in.
        if let Err(err) = plugin.register(&mut registrar) {
            return Err(EnableError::RefusedToStart {
                plugin: id.clone(),
                reason: without_secrets(err.to_string(), &secrets),
            });
        }
        let stop = registrar.stop();
        let registrations = registrar.into_registrations();
        let declared = &manifest.extension_points;
        if let Some(point) = registrations
            .iter()
            .map(Registration::point)
            .find(|point| !declared.contains(point))
        {
            return Err(EnableError::Refused {
                plugin: id.clone(),
                reason: format!(
                    "it registers at {}, which its Manifest doesn't name",
                    point.name()
                ),
            });
        }
        let count = registrations.len();
        // Plugin values the lock refuses are dropped after it is released, since a Plugin's `Drop`
        // is Plugin code.
        let refused = self.write(|state| {
            let refusal = if state.plugins.contains_key(id) {
                Some(EnableError::AlreadyEnabled(id.clone()))
            } else {
                state.fulfiller_taken(id, &registrations)
            };
            if let Some(refusal) = refusal {
                return Some((refusal, plugin, registrations));
            }
            let generation = state.next_generation;
            state.next_generation += 1;
            state.plugins.insert(
                id.clone(),
                Enabled {
                    _plugin: plugin,
                    generation,
                    scope: tick::scope(&self.points, manifest, &registrations).with_stop(stop),
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
        if let Some((refusal, ..)) = refused {
            return Err(refusal);
        }
        info!(plugin = %id, version = %manifest.version, registrations = count, "enabled plugin");
        Ok(())
    }

    /// The Registrar for an enablement of `manifest`'s Plugin with `settings`: it hands the
    /// enablement a fresh [`Stop`], and the Kernel's functions granted those of `granted` its
    /// Manifest requests, which refuse once that `Stop` fires.
    fn registrar_for(
        &self,
        manifest: &Manifest,
        settings: Settings,
        mut granted: HashSet<Permission>,
    ) -> Registrar {
        granted.retain(|permission| manifest.permissions.contains(permission));
        let stop = Stop::new();
        let kernel = Arc::new(EnabledKernel {
            slot: self.read(|state| Arc::clone(&state.kernel)),
            plugin: manifest.id.clone(),
            granted,
            stop: stop.clone(),
        });
        Registrar::with_settings(settings)
            .with_kernel(kernel)
            .with_stop(stop)
    }

    /// Disables the Plugin `id` and removes everything it registered, then cancels its scope and
    /// waits for the tasks in it before dropping the Plugin (ADR 0047). Returns whether it was
    /// enabled.
    ///
    /// Never call it from inside the Plugin's own call or scope, which it would wait on forever;
    /// [`Registry::disable_now`] is for that.
    pub async fn disable(&self, id: &PluginId) -> bool {
        let Ok(removed) = self.remove_if(id, |_, _| Ok::<_, Infallible>(()));
        let Some(removed) = removed else {
            return false;
        };
        removed.close(id).await;
        true
    }

    /// Disables the Plugin `id` and cancels its scope at once, for a caller inside the Plugin's
    /// own call or scope. Returns whether it was enabled. The tasks in its scope are joined and
    /// the Plugin dropped in the background, after this returns.
    #[expect(
        clippy::must_use_candidate,
        reason = "callers often disable without caring whether the Plugin was enabled"
    )]
    pub fn disable_now(&self, id: &PluginId) -> bool {
        let Ok(removed) = self.remove_if(id, |_, _| Ok::<_, Infallible>(()));
        removed.map(|removed| removed.close_later(id)).is_some()
    }

    /// Cancels every enabled Plugin's scope and waits for the tasks in each, for the Server
    /// stopping. The Plugins stay enabled in memory.
    pub async fn shutdown(&self) {
        let scopes: Vec<Scope> = self.read(|state| {
            state
                .plugins
                .values()
                .map(|enabled| enabled.scope.clone())
                .collect()
        });
        futures_util::future::join_all(scopes.into_iter().map(Scope::close)).await;
    }

    /// The refusal of disabling the Plugin `id` if no other enabled Plugin provides `T`, such as
    /// the Authenticator a Server can't do without, or `None`. Also refused if `id` isn't enabled
    /// and no Plugin provides `T`: what a Plugin that isn't running provides is unknown, and it
    /// may be one that panicked and would provide `T` after a restart.
    #[must_use]
    pub fn last_provider<T: ?Sized + 'static>(&self, id: &PluginId) -> Option<LastProvider> {
        let point = PointKey::provide::<T>();
        self.read(|state| {
            let entries = state.points.get(&point).map_or(&[][..], Vec::as_slice);
            let provides =
                !state.plugins.contains_key(id) || entries.iter().any(|entry| entry.plugin == *id);
            let others = entries.iter().any(|entry| entry.plugin != *id);
            (provides && !others).then(|| LastProvider {
                plugin: id.clone(),
                point: point.name(),
            })
        })
    }

    /// Runs the Plugin `id`'s settings check on `settings`, on a fresh instance whether
    /// it is enabled or not, and returns the choices it offers. The settings are checked against
    /// its schema first, with its Choice settings optional, and its defaults filled in.
    ///
    /// # Errors
    ///
    /// If no Plugin has the id, it has no check, the settings don't fit, or the check
    /// fails. A check that panics or runs past [`SETTINGS_CHECK_TIMEOUT`] fails without harming the
    /// Server.
    pub async fn check_settings(
        &self,
        id: &PluginId,
        settings: Settings,
    ) -> Result<Choices, CheckError> {
        let checkable = self
            .read(|state| {
                let known = state.known.get(id)?;
                let checkable = known.plugin.as_ref().ok().filter(|_| known.checks_settings);
                Some(
                    checkable.map(|(make, manifest)| (Arc::clone(make), manifest.settings.clone())),
                )
            })
            .ok_or_else(|| CheckError::Unknown(id.clone()))?;
        let no_check = || CheckError::Failed(format!("{id} has no settings check"));
        let Some((make, schema)) = checkable else {
            return Err(no_check());
        };
        let settings = settings
            .resolve(&with_optional_choices(schema.clone()))
            .map_err(CheckError::InvalidSettings)?;
        let secrets = secret_values(&schema, &settings);
        let checked = check_fresh(id, &make, &secrets, &settings, |plugin, settings| {
            Some(plugin.settings_check()?.check(settings))
        });
        checked.await?.ok_or_else(no_check)
    }

    /// Runs the Plugin `id`'s check of one User's Personal settings, `personal`, on a fresh
    /// instance with the Plugin's saved `settings`, whether it is enabled or not. A Plugin without
    /// one passes.
    ///
    /// # Errors
    ///
    /// If no Plugin has the id, the Server refused it, its settings don't fit, or the check fails.
    /// A check that panics or runs past [`SETTINGS_CHECK_TIMEOUT`] fails without harming the
    /// Server.
    pub async fn check_personal_settings(
        &self,
        id: &PluginId,
        settings: Settings,
        personal: &BTreeMap<String, String>,
    ) -> Result<(), CheckError> {
        let (make, manifest) = self
            .read(|state| {
                let (make, manifest) = state.known.get(id)?.plugin.as_ref().ok()?;
                Some((Arc::clone(make), manifest.clone()))
            })
            .ok_or_else(|| CheckError::Unknown(id.clone()))?;
        let settings = settings
            .resolve(&with_optional_choices(manifest.settings.clone()))
            .map_err(CheckError::InvalidSettings)?;
        let mut secrets = secret_values(&manifest.settings, &settings);
        let personal_secrets = manifest
            .personal_settings
            .fields
            .iter()
            .filter(|field| field.kind == SettingKind::Secret)
            .filter_map(|field| personal.get(&field.key));
        secrets.extend(personal_secrets.cloned());
        let asked = (settings, personal);
        let checked = check_fresh(id, &make, &secrets, &asked, |plugin, (saved, values)| {
            Some(plugin.personal_settings_check()?.check(saved, values))
        });
        checked.await.map(drop)
    }

    /// The ids of the enabled Plugins, sorted.
    #[must_use]
    pub fn enabled(&self) -> Vec<PluginId> {
        self.read(|state| state.plugins.keys().cloned().collect())
    }

    /// Every Plugin the Server has, enabled or not, sorted by id: the Built-in ones
    /// [`Registry::with_builtins`] or [`Registry::describe_builtin`] made known, and the Installed
    /// ones the Server last read.
    #[must_use]
    pub fn plugins(&self) -> Vec<Listed> {
        self.read(|state| {
            let listed = |(id, described)| state.listed(id, described);
            state.known.iter().map(listed).collect()
        })
    }

    /// The Plugin `id`, as [`Registry::plugins`] lists it, or `None` if the Server has none.
    #[must_use]
    pub fn plugin(&self, id: &PluginId) -> Option<Listed> {
        self.read(|state| Some(state.listed(id, state.known.get(id)?)))
    }

    /// How the rest of the Kernel calls these Plugins, which can't change them.
    #[must_use]
    pub fn extension_points(&self) -> ExtensionPoints {
        self.points.clone()
    }

    /// Sets which Plugins' Providers of `T` come first, and in what order, overriding their
    /// priorities. Plugins not in `plugins` follow in priority order. The ranking is kept while a
    /// Plugin in it is disabled or not yet enabled.
    pub fn set_provider_precedence<T: ?Sized + 'static>(&self, plugins: Vec<PluginId>) {
        self.write(|state| state.precedence.insert(PointKey::provide::<T>(), plugins));
    }

    /// Testing only: ticks the Plugin `id`'s Automation once, now, as its schedule would, since
    /// ticks come at least `MIN_TICK_EVERY` apart.
    ///
    /// # Errors
    ///
    /// The tick's failure message; that the Plugin isn't enabled or provides no Automation; or
    /// that its tick panicked, which disabled it as any panic does.
    #[cfg(feature = "testing")]
    pub async fn tick_now(&self, id: &PluginId) -> Result<(), String> {
        match tick::once(&self.points, id).await {
            tick::Ticked::Ran(result) => result.map_err(|failure| failure.message),
            tick::Ticked::NoAutomation => {
                Err(format!("{id} isn't enabled or provides no Automation"))
            }
            tick::Ticked::Panicked => Err(format!("{id}'s tick panicked, so it is disabled")),
        }
    }

    /// The id of each Installed Plugin disabled from now on because it crashed, for the Server to
    /// save as disabled. Replaces the receiver an earlier call returned.
    pub(crate) fn crashed(&self) -> mpsc::UnboundedReceiver<PluginId> {
        let (sender, receiver) = mpsc::unbounded_channel();
        self.write(|state| state.crashed = Some(sender));
        receiver
    }
}

/// The Kernel's functions as one enablement of a Plugin calls them: those
/// [`ExtensionPoints::serve_kernel`] makes for it, found at call time, since Plugins are enabled
/// before startup can make them.
struct EnabledKernel {
    slot: Arc<watch::Sender<Option<MakeKernel>>>,
    plugin: PluginId,
    granted: HashSet<Permission>,
    /// The enablement's stop signal, which its scope's cancel fires.
    stop: Stop,
}

impl EnabledKernel {
    /// The functions to answer one call with. Checked on each call, so nothing an enablement asks
    /// after its disable, or after a grant change enabled it again, reaches the Kernel; a call
    /// already past here, such as a delete that started, goes on.
    fn resolve(&self) -> Result<Arc<dyn Kernel>, KernelRefused> {
        if self.stop.is_stopped() {
            return Err(KernelRefused("the Plugin is disabled".to_owned()));
        }
        let make = self
            .slot
            .borrow()
            .clone()
            .ok_or_else(|| KernelRefused("the Server isn't serving yet".to_owned()))?;
        Ok(make(self.plugin.clone(), self.granted.clone()))
    }
}

impl Kernel for EnabledKernel {
    fn store_get<'a>(
        &'a self,
        key: &'a StoreKey,
    ) -> BoxFuture<'a, Result<StoreValue, KernelRefused>> {
        Box::pin(async move { self.resolve()?.store_get(key).await })
    }

    fn store_put<'a>(
        &'a self,
        entry: &'a StoreEntry,
    ) -> BoxFuture<'a, Result<Stored, KernelRefused>> {
        Box::pin(async move { self.resolve()?.store_put(entry).await })
    }

    fn store_delete<'a>(
        &'a self,
        key: &'a StoreKey,
    ) -> BoxFuture<'a, Result<Stored, KernelRefused>> {
        Box::pin(async move { self.resolve()?.store_delete(key).await })
    }

    fn list_libraries<'a>(
        &'a self,
        asked: &'a LibrariesAsked,
    ) -> BoxFuture<'a, Result<Libraries, KernelRefused>> {
        Box::pin(async move { self.resolve()?.list_libraries(asked).await })
    }

    fn list_titles<'a>(
        &'a self,
        asked: &'a TitlesAsked,
    ) -> BoxFuture<'a, Result<Titles, KernelRefused>> {
        Box::pin(async move { self.resolve()?.list_titles(asked).await })
    }

    fn list_watch_states<'a>(
        &'a self,
        asked: &'a WatchStatesAsked,
    ) -> BoxFuture<'a, Result<WatchStates, KernelRefused>> {
        Box::pin(async move { self.resolve()?.list_watch_states(asked).await })
    }

    fn list_users<'a>(
        &'a self,
        asked: &'a UsersAsked,
    ) -> BoxFuture<'a, Result<Users, KernelRefused>> {
        Box::pin(async move { self.resolve()?.list_users(asked).await })
    }

    fn delete_media<'a>(
        &'a self,
        asked: &'a DeleteAsked,
    ) -> BoxFuture<'a, Result<DeleteStarted, KernelRefused>> {
        Box::pin(async move { self.resolve()?.delete_media(asked).await })
    }
}

impl State {
    fn listed(&self, id: &PluginId, described: &Described) -> Listed {
        Listed {
            id: id.clone(),
            kind: described.kind,
            manifest: described
                .plugin
                .as_ref()
                .map(|(_, manifest)| manifest.clone())
                .map_err(Clone::clone),
            checks_settings: described.checks_settings,
            enabled: self.plugins.contains_key(id),
        }
    }

    /// The refusal of enabling `id` with `registrations`, if they include a Fulfiller for a kind of
    /// title another enabled Plugin already provides one for.
    fn fulfiller_taken(
        &self,
        id: &PluginId,
        registrations: &[Registration],
    ) -> Option<EnableError> {
        let kinds = [
            (PointKey::provide::<dyn Fulfiller<Movies>>(), "Movie"),
            (PointKey::provide::<dyn Fulfiller<Shows>>(), "Show"),
        ];
        kinds.into_iter().find_map(|(point, kind)| {
            let provides = registrations
                .iter()
                .any(|registration| registration.point() == point);
            if !provides {
                return None;
            }
            let by = self.points.get(&point)?.first()?.plugin.clone();
            Some(EnableError::FulfillerTaken {
                plugin: id.clone(),
                kind,
                by,
            })
        })
    }
}

/// Why the Kernel can't take `manifest`'s Personal settings, if it can't (ADR 0046): one of a
/// kind a User's value could widen the hosts the Plugin reaches by, one naming Libraries, which a
/// User can't pick from the Admin's list, or with a key that repeats or that a Plugin-wide setting
/// also has.
pub(super) fn personal_settings_refusal(manifest: &Manifest) -> Option<String> {
    let personal = &manifest.personal_settings.fields;
    personal.iter().enumerate().find_map(|(index, field)| {
        let key = &field.key;
        if matches!(
            field.kind,
            SettingKind::Url | SettingKind::Choice | SettingKind::Libraries
        ) {
            Some(format!(
                "its Personal setting {key} is {:?}, and a Personal setting may only be Text, \
                 Secret, Bool or Integer",
                field.kind
            ))
        } else if personal
            .iter()
            .take(index)
            .any(|earlier| earlier.key == *key)
        {
            Some(format!("its Personal setting {key} repeats a key"))
        } else if manifest
            .settings
            .fields
            .iter()
            .any(|setting| setting.key == *key)
        {
            Some(format!(
                "its Personal setting {key} has a Plugin setting's key"
            ))
        } else {
            None
        }
    })
}

/// Refuses `manifest` if [`personal_settings_refusal`] says why.
fn refuse_bad_personal_settings(manifest: &Manifest) -> Result<(), EnableError> {
    personal_settings_refusal(manifest).map_or(Ok(()), |reason| {
        Err(EnableError::Refused {
            plugin: manifest.id.clone(),
            reason,
        })
    })
}

/// The check [`check_fresh`] runs: a fresh Plugin's check on what is asked, or `None` if the Plugin
/// has no such check.
type FreshCheck<A, T> =
    for<'p> fn(&'p dyn Plugin, &'p A) -> Option<BoxFuture<'p, Result<T, PluginError>>>;

/// Runs `check` on `asked` with a fresh instance of the Plugin `id` that `make` makes, contained
/// and under [`SETTINGS_CHECK_TIMEOUT`], with `secrets` blanked out of why it failed. `None` if the
/// Plugin has no such check.
async fn check_fresh<A: Sync, T>(
    id: &PluginId,
    make: &Make,
    secrets: &[String],
    asked: &A,
    check: FreshCheck<A, T>,
) -> Result<Option<T>, CheckError> {
    let scrubbed = |message: String| without_secrets(message, secrets);
    let panicked = |message: String| {
        let message = scrubbed(message);
        error!(plugin = %id, message, "plugin panicked while checking its settings");
        CheckError::Failed(format!("{id} failed while checking its settings"))
    };
    let plugin = contain_sync(|| make()).map_err(panicked)?;
    let fresh: &dyn Plugin = &*plugin;
    // The timeout is inside `contain_call`, so dropping a check it gave up on (Plugin code) is
    // contained too, as are formatting and dropping the Plugin's error.
    let checking = contain_call(id, "settings check", || async move {
        let checking = check(fresh, asked)?;
        let checked = tokio::time::timeout(SETTINGS_CHECK_TIMEOUT, checking)
            .await
            .unwrap_or_else(|_elapsed| Err(CheckTimedOut(SETTINGS_CHECK_TIMEOUT).into()));
        Some(
            checked.map_err(|error| match error.downcast::<CheckTimedOut>() {
                Ok(timed_out) => CheckError::TimedOut(id.clone(), timed_out.0),
                Err(error) => CheckError::Failed(scrubbed(error.to_string())),
            }),
        )
    });
    let checked = match checking.await {
        Ok(checked) => checked.transpose(),
        Err(message) => Err(panicked(message)),
    };
    if let Err(message) = contain_sync(|| drop(plugin)) {
        error!(plugin = %id, message, "plugin panicked while being dropped");
    }
    checked
}

/// `schema` with its Choice settings optional, since checking the settings is how an admin finds
/// what to pick for them.
fn with_optional_choices(mut schema: SettingsSchema) -> SettingsSchema {
    for field in &mut schema.fields {
        field.required &= field.kind != SettingKind::Choice;
    }
    schema
}

impl fmt::Debug for Registry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Registry")
            .field("enabled", &self.enabled())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
