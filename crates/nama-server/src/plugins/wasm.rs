//! Installed Plugins (ADR 0006): `.wasm` files bundled in the Server (ADR 0003) or that an admin
//! drops into `<data_dir>/plugins/`, run through Extism behind the same [`Plugin`] trait as a
//! Built-in Plugin, so the Kernel can't tell them apart.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    io, panic,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
    time::Duration,
};

use buffa::{DecodeError, Message};
use nama_plugin_api::{
    BoxFuture, Manifest, Observer, Plugin, PluginError, PluginId, Registrar,
    automation::{Automation, Tick},
    connector::{ConnectionStarted, Connector},
    fulfiller::{Download, Fulfiller, Request, TryAgainIn},
    notifier::{Notification, Notifier, Recipient, RecipientGone},
    points::{Events, ObservedEvents},
    settings::{Choice, Choices, PersonalSettingsCheck, SettingKind, Settings, SettingsCheck},
};
use nama_plugin_proto::{
    CONTRACT_VERSION, SERVICES,
    nama::plugin::v1::{
        self as wire, Channel, CheckedSettings, ConnectionPoll, ConnectionPolled, ConnectionStart,
        Delivered, Delivery, EventsObserved, Fulfilled, PersonalSettingsChecked, Progress,
        ProgressAsked, StopMonitoring, StoppedMonitoring, Ticked, Unfollowed, UserRecipient,
    },
};
use semver::{Version, VersionReq};
use thiserror::Error;
use tokio::task;
use tracing::warn;

use super::{CheckTimedOut, registry::Make, secret_values, without_secrets};

mod automation;
mod blob_store;
mod bundled;
mod calls;
mod contract;
mod listing;
mod load;
mod recogniser;
mod services;
mod title_source;

#[cfg(test)]
#[path = "wasm/calls_tests.rs"]
mod calls_tests;
#[cfg(test)]
#[path = "wasm/fulfiller_tests.rs"]
mod fulfiller_tests;

pub(crate) use listing::read;
use load::{
    CALL_TIMEOUT, Compiled, MEMORY_MAX_PAGES, allowed_hosts, compile, compile_with_kernel,
    extism_config,
};
use services::{
    CHECK_PERSONAL_SETTINGS, CHECK_SETTINGS, DELIVER, OBSERVE_EVENTS, POINTS, POLL_CONNECTION,
    Point, START_CONNECTION, TICK,
};

/// How many calls in a row may crash before the Plugin is disabled, so one bad input doesn't
/// cost an admin a Plugin.
const CRASHES_BEFORE_DISABLING: u32 = 3;

/// Why the Server refused an Installed Plugin.
#[derive(Debug, Error)]
pub(super) enum Refused {
    #[error("could not read it: {0}")]
    Read(io::Error),
    #[error("it is not a WASM module")]
    NotWasm,
    #[error("the Server could not load it: {0:#}")]
    Load(extism::Error),
    // The root cause is the Plugin's own error or panic message, or the trap; extism logs the
    // wasm backtrace wrapped around it.
    #[error("its manifest() export failed: {}", .0.root_cause())]
    Manifest(extism::Error),
    #[error("the Server panicked while loading it: {0}")]
    Panicked(String),
    #[error("its manifest() returned something that isn't a Manifest: {0}")]
    Decode(DecodeError),
    #[error("its Plugin contract requirement {wants:?} isn't a semver requirement: {error}")]
    BadRequirement { wants: String, error: semver::Error },
    #[error(
        "it wants Plugin contract {wants}, and this Server has {}",
        CONTRACT_VERSION
    )]
    OutOfRange { wants: String },
    #[error("its setting {key} is of a kind this Server doesn't know")]
    UnknownSettingKind { key: String },
    #[error("its setting {key}'s default isn't {kind:?}")]
    DefaultWrongKind { key: String, kind: SettingKind },
    #[error("its allowed host {0} has a port, and allowed hosts are matched without one")]
    HostWithPort(String),
    #[error("its id {0} is another Plugin's")]
    IdTaken(PluginId),
    #[error(
        "its Manifest names no Extension Points, so it would do nothing; a Plugin built before \
         Extension Points became services must name them in points"
    )]
    NoPoints,
    #[error("its Manifest names {0}, an Extension Point this Server doesn't serve")]
    UnknownPoint(String),
    #[error("it wants {service} {wants}, and this Server serves {version}")]
    PointVersion {
        service: &'static str,
        wants: String,
        version: &'static str,
    },
    #[error("its Manifest names {service}, and it has no {export} export")]
    MissingExport {
        service: &'static str,
        export: &'static str,
    },
    // The reason starts "its Personal setting", as the registry words it for a Built-in.
    #[error("{0}")]
    PersonalSettings(String),
    #[error("its Manifest requests the Permission {0}, which this Server doesn't know")]
    UnknownPermission(String),
    #[error(
        "its Manifest names {}, and ticks every {seconds} seconds, under the minimum of {}",
        nama_plugin_proto::services::AUTOMATION,
        load::MIN_TICK_EVERY.as_secs()
    )]
    TickTooOften { seconds: u32 },
    #[error("its {field} names {namespace:?}, which isn't a namespace")]
    BadNamespace {
        field: &'static str,
        namespace: String,
    },
}

/// An Installed Plugin the Server read and checked, ready to enable.
#[derive(Clone)]
pub(super) struct Installed {
    /// What its package says about it, as the Kernel reads it.
    manifest: Manifest,
    /// Its package's own Manifest, for what only an Installed Plugin says, such as its hosts.
    package: Arc<wire::Manifest>,
    /// The `.wasm` file's contents. Extism fixes the hosts a Plugin may reach and the settings it
    /// reads when it compiles it, so each enabling compiles them again.
    wasm: Arc<[u8]>,
    cache_config: Arc<Path>,
    /// Whether it exports [`CHECK_SETTINGS`].
    checks_settings: bool,
    /// Its exports among those the Server may call.
    exports: Arc<BTreeSet<&'static str>>,
}

/// An enabled Installed Plugin, compiled with its allowed hosts and settings. Each call into it
/// gets a fresh instance, so one that traps or times out leaves nothing behind for the next.
#[derive(Clone)]
struct Running {
    package: Arc<wire::Manifest>,
    compiled: Arc<Compiled>,
    /// The values of its secret settings, which the failures it logs mustn't show.
    secrets: Arc<[String]>,
    /// How many calls in a row crashed, counted apart for each of its points' `crash_group`s.
    crashes: Arc<AtomicU32>,
    /// Its exports among those the Server may call, which tell whether it serves an optional
    /// method.
    exports: Arc<BTreeSet<&'static str>>,
}

impl Running {
    /// Calls the Plugin's export `name` with `input`, a protobuf message, on a fresh instance,
    /// failing it as a timeout past `limit` of the Plugin's own time. Blocks while the Plugin runs.
    fn call(&self, name: &str, input: Vec<u8>, limit: Duration) -> Result<Vec<u8>, extism::Error> {
        let compiled = &self.compiled;
        compiled.calls.run(&compiled.plugin, name, input, limit)
    }

    /// [`Self::call`] on a blocking thread. A panic there unwinds here, so the registry disables
    /// the Plugin as it does one whose Rust code panics.
    async fn call_blocking(
        &self,
        name: &'static str,
        input: Vec<u8>,
        limit: Duration,
    ) -> Result<Result<Vec<u8>, extism::Error>, task::JoinError> {
        let plugin = self.clone();
        match task::spawn_blocking(move || plugin.call(name, input, limit)).await {
            Err(err) if err.is_panic() => panic::resume_unwind(err.into_panic()),
            called => called,
        }
    }

    /// [`Self::call_blocking`] for a Provider, whose caller gets why it failed, to log or show
    /// such as a Request's reason for being Failed: in the Plugin's own words, or that it crashed.
    /// Counts its crashes as [`Self::count_crash`] does.
    async fn provide(
        &self,
        name: &'static str,
        input: Vec<u8>,
        limit: Duration,
    ) -> Result<Vec<u8>, PluginError> {
        let err = match self.call_blocking(name, input, limit).await? {
            Ok(output) => {
                self.crashes.store(0, Ordering::Relaxed);
                return Ok(output);
            }
            Err(err) => err,
        };
        let cause = if is_timeout(&err) {
            format!("it didn't answer within {limit:?}")
        } else {
            failure_cause(&err)
        };
        self.count_crash(&err, &without_secrets(cause.clone(), &self.secrets));
        Err(cause.into())
    }

    /// Logs why a call to its Observer failed, and counts it as [`Self::count_crash`] does.
    fn observer_failed(&self, err: &extism::Error) {
        let cause = without_secrets(failure_cause(err), &self.secrets);
        let failed = if is_crash(err) { "crashed" } else { "failed" };
        warn!(plugin = %self.package.id, "observer {failed}: {cause}");
        self.count_crash(err, &cause);
    }

    /// Counts the crashes in a row, `err` among them if it is one. On the
    /// [`CRASHES_BEFORE_DISABLING`]th, unwinds with why, `cause`, so the registry disables the
    /// Plugin as it does one whose Rust code panics. A call interrupted because the enablement is
    /// ending, or by another call's timeout, leaves the count as it was: resetting it there would
    /// let a Plugin whose calls all loop, each ending the others, never be disabled.
    fn count_crash(&self, err: &extism::Error, cause: &str) {
        if calls::is_interrupted(err) || calls::is_collateral(err) {
            return;
        }
        if !is_crash(err) {
            self.crashes.store(0, Ordering::Relaxed);
        } else if self.crashes.fetch_add(1, Ordering::Relaxed) + 1 == CRASHES_BEFORE_DISABLING {
            let why =
                format!("it crashed {CRASHES_BEFORE_DISABLING} times in a row, last with: {cause}");
            panic::resume_unwind(Box::new(why));
        }
    }
}

/// Whether a failed call trapped, timed out or ran past [`MEMORY_MAX_PAGES`], rather than the
/// Plugin returning an error or a host function such as an HTTP request refusing it.
fn is_crash(err: &extism::Error) -> bool {
    // Any failed call but a timeout or one past the memory cap is wasmtime's own error, which
    // holds a trap (under any panic's message) only if the guest trapped. A host function's error
    // unwinds the guest too, but holds no trap.
    is_timeout(err)
        || is_out_of_memory(err)
        || err
            .downcast_ref::<wasmtime::Error>()
            .is_some_and(wasmtime::Error::is::<wasmtime::Trap>)
}

/// Whether a failed call ran past its own limit. Once [`calls::Calls::run`] has classified it, a
/// call a stop or another call's timeout interrupted fails as [`calls::Interrupted`] or
/// [`calls::Collateral`] instead.
fn is_timeout(err: &extism::Error) -> bool {
    // Extism (1.41) gives a timeout as a bare "timeout".
    err.to_string() == "timeout"
}

/// Whether a failed call ran past [`MEMORY_MAX_PAGES`], which Extism (1.41) gives as a bare "oom".
fn is_out_of_memory(err: &extism::Error) -> bool {
    err.to_string() == "oom"
}

/// Why a call failed: the root cause, with the memory cap's bare "oom" put in plain words. A
/// timeout's bare "timeout" is left to the callers: [`Running::provide`] words it, and the settings
/// check reports it as [`CheckTimedOut`].
fn failure_cause(err: &extism::Error) -> String {
    if is_out_of_memory(err) {
        format!("it used more than {} MiB of memory", MEMORY_MAX_PAGES / 16)
    } else {
        err.root_cause().to_string()
    }
}

/// A Plugin to list as Installed once [`describe`] finds its id free.
pub(super) struct Listable {
    make: Make,
    manifest: Manifest,
    checks_settings: bool,
}

impl From<Installed> for Listable {
    fn from(plugin: Installed) -> Self {
        Self {
            manifest: plugin.manifest.clone(),
            checks_settings: plugin.checks_settings,
            make: Arc::new(move || Arc::new(plugin.clone())),
        }
    }
}

impl Plugin for Installed {
    fn manifest(&self) -> Manifest {
        self.manifest.clone()
    }

    fn settings_check(&self) -> Option<&dyn SettingsCheck> {
        self.checks_settings.then_some(self)
    }

    fn personal_settings_check(&self) -> Option<&dyn PersonalSettingsCheck> {
        self.exports
            .contains(CHECK_PERSONAL_SETTINGS)
            .then_some(self)
    }

    // ponytail: compiles on the calling thread, an async one when an admin enables it; wasmtime's
    // cache makes that quick, so spawn_blocking only if a Plugin is slow to enable.
    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        let settings = registrar.settings();
        let schema = &self.manifest.settings;
        let hosts = allowed_hosts(&self.package, schema, settings);
        let config = extism_config(schema, settings);
        let kernel = registrar.kernel();
        let calls = calls::Calls::default();
        // The Kernel stops awaiting a call when it cancels the enablement's scope, but the call
        // runs on in its blocking thread, with this enablement's Kernel functions, until stopped.
        registrar.stop().on_stop({
            let calls = calls.clone();
            move || calls.stop()
        });
        let compile = || {
            let (hosts, config, kernel) = (hosts.clone(), config.clone(), kernel.clone());
            compile_with_kernel(
                &self.wasm,
                &self.cache_config,
                hosts,
                config,
                kernel,
                calls.clone(),
            )
            .map(Arc::new)
            .map_err(|err| format!("could not compile it with its settings: {err:#}"))
        };
        let secrets: Arc<[String]> = secret_values(schema, settings).into();
        // The Kernel refuses a Plugin registering at a point its Manifest doesn't name, so read
        // them from the same list the Kernel checks.
        let declared = &self.manifest.extension_points;
        let mut crash_groups = BTreeMap::<_, Arc<AtomicU32>>::new();
        // Extism ends a call, at its limit or cancelled, by bumping the epoch of the engine it
        // runs on, which interrupts every call running on that engine, and each compile has one.
        // A call ended so fails as `calls::Collateral`, not as a crash; a compile per call would
        // avoid it, but compiling costs too much. So a point whose calls may run long gets a
        // compile of its own: a tick ended at its limit doesn't end a delivery running beside it,
        // nor a delivery's timeout the tick. The other points share one, as their calls are short.
        let mut shared = None;
        for point in POINTS
            .iter()
            .filter(|point| declared.contains(&(point.key)()))
        {
            let compiled = if point.runs_long() {
                compile()?
            } else if let Some(shared) = &shared {
                Arc::clone(shared)
            } else {
                Arc::clone(shared.insert(compile()?))
            };
            let crashes = Arc::clone(crash_groups.entry(point.crash_group).or_default());
            (point.register)(
                registrar,
                Running {
                    package: Arc::clone(&self.package),
                    compiled,
                    secrets: Arc::clone(&secrets),
                    crashes,
                    exports: Arc::clone(&self.exports),
                },
            );
        }
        Ok(())
    }
}

/// The Extension Points `package` names. One this Server doesn't serve, or doesn't serve at a
/// version the Manifest takes, refuses it, rather than leaving the Plugin doing less than its
/// Manifest says.
fn points(package: &wire::Manifest) -> Result<Vec<&'static Point>, Refused> {
    package
        .points
        .iter()
        .map(|named| {
            let point = POINTS
                .iter()
                .find(|point| point.service == named.service)
                .ok_or_else(|| Refused::UnknownPoint(named.service.clone()))?;
            // POINTS equals SERVICES by test, so every point has a version.
            let version = served_version(point.service)
                .ok_or_else(|| Refused::UnknownPoint(named.service.clone()))?;
            let met = VersionReq::parse(&named.version)
                .is_ok_and(|wants| Version::parse(version).is_ok_and(|v| wants.matches(&v)));
            if !met {
                return Err(Refused::PointVersion {
                    service: point.service,
                    wants: named.version.clone(),
                    version,
                });
            }
            Ok(point)
        })
        .collect()
}

/// The version this Server serves `service` at, from the contract's `SERVICES`.
fn served_version(service: &str) -> Option<&'static str> {
    SERVICES
        .iter()
        .find(|&&(name, _)| name == service)
        .map(|&(_, version)| version)
}

/// Checks that a Plugin has every export `points` need, `has_export` telling which it has.
fn check_exports(points: &[&Point], has_export: impl Fn(&str) -> bool) -> Result<(), Refused> {
    for point in points {
        if let Some(method) = point.required().find(|method| !has_export(method.export)) {
            return Err(Refused::MissingExport {
                service: method.service,
                export: method.export,
            });
        }
    }
    Ok(())
}

// One impl for both kinds: the Plugin tells a Movie Request from a Season one by which it holds.
impl<Kind: contract::FulfillerKind + 'static> Fulfiller<Kind> for Running {
    fn needs(&self) -> Vec<String> {
        Kind::needs(&self.package)
    }

    fn fulfill<'a>(
        &'a self,
        request: &'a Request,
    ) -> BoxFuture<'a, Result<Fulfilled, PluginError>> {
        Box::pin(async move {
            let fulfilled: Fulfilled = self.call_method(Kind::FULFILL, request).await?;
            match try_again(fulfilled.retry_after_seconds) {
                Some(wait) => Err(wait.into()),
                None => Ok(fulfilled),
            }
        })
    }

    fn unfollow<'a>(&'a self, request: &'a Request) -> BoxFuture<'a, Result<(), PluginError>> {
        // The Kernel asks only a Show Fulfiller to unfollow, so only its service has the method.
        let Some(unfollow) = Kind::UNFOLLOW else {
            return Box::pin(async { Ok(()) });
        };
        Box::pin(async move {
            let unfollowed: Unfollowed = self.call_method(unfollow, request).await?;
            try_again(unfollowed.retry_after_seconds).map_or(Ok(()), |wait| Err(wait.into()))
        })
    }

    fn stop_monitoring<'a>(
        &'a self,
        title: &'a StopMonitoring,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        if !self.serves(Kind::STOP_MONITORING) {
            return Box::pin(async { Ok(()) });
        }
        Box::pin(async move {
            let stopped: StoppedMonitoring = self.call_method(Kind::STOP_MONITORING, title).await?;
            try_again(stopped.retry_after_seconds).map_or(Ok(()), |wait| Err(wait.into()))
        })
    }

    fn progress<'a>(
        &'a self,
        references: &'a [String],
    ) -> BoxFuture<'a, Result<HashMap<String, Vec<Download>>, PluginError>> {
        Box::pin(async move {
            let asked = ProgressAsked {
                references: references.to_vec(),
                ..Default::default()
            };
            let progress: Progress = self.call_method(Kind::PROGRESS, &asked).await?;
            Ok(progress
                .downloads
                .into_iter()
                .map(|(reference, downloads)| (reference, downloads.downloads))
                .collect())
        })
    }
}

/// A Fulfiller's ask to be asked again, clamped to 1..=300 seconds as `fulfiller.proto` says.
fn try_again(retry_after_seconds: Option<u32>) -> Option<TryAgainIn> {
    retry_after_seconds.map(|seconds| TryAgainIn(Duration::from_secs(seconds.clamp(1, 300).into())))
}

fn delivery(notification: &Notification, recipient: &Recipient) -> Delivery {
    let recipient = match recipient {
        Recipient::Channel => wire::delivery::Recipient::from(Channel::default()),
        Recipient::Connection(connection) => {
            wire::delivery::Recipient::Connection(connection.clone())
        }
        Recipient::User(settings) => wire::delivery::Recipient::from(UserRecipient {
            personal_settings: settings.clone().into_iter().collect(),
            ..Default::default()
        }),
    };
    Delivery {
        notification: Some(notification.clone()).into(),
        recipient: Some(recipient),
        ..Default::default()
    }
}

impl Notifier for Running {
    fn deliver<'a>(
        &'a self,
        notification: &'a Notification,
        recipient: &'a Recipient,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        let delivery = delivery(notification, recipient);
        Box::pin(async move {
            let delivered: Delivered = self.call_method(&DELIVER, &delivery).await?;
            if delivered.recipient_gone {
                return Err(RecipientGone.into());
            }
            Ok(())
        })
    }
}

impl Connector for Running {
    fn start_connection<'a>(
        &'a self,
        code: &'a str,
    ) -> BoxFuture<'a, Result<ConnectionStarted, PluginError>> {
        let start = ConnectionStart {
            code: code.to_owned(),
            ..Default::default()
        };
        Box::pin(async move { self.call_method(&START_CONNECTION, &start).await })
    }

    fn poll_connection<'a>(
        &'a self,
        pending: &'a str,
    ) -> BoxFuture<'a, Result<Option<String>, PluginError>> {
        let poll = ConnectionPoll {
            pending: pending.to_owned(),
            ..Default::default()
        };
        Box::pin(async move {
            let polled: ConnectionPolled = self.call_method(&POLL_CONNECTION, &poll).await?;
            Ok(polled.connection)
        })
    }
}

impl Observer<Events> for Running {
    fn observe<'a>(&'a self, event: &'a ObservedEvents) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            // An Observer's failure is logged and counted there; the Kernel waits for no answer.
            drop(
                self.call_method::<_, EventsObserved>(&OBSERVE_EVENTS, event)
                    .await,
            );
        })
    }
}

impl Automation for Running {
    fn tick<'a>(&'a self, tick: &'a Tick) -> BoxFuture<'a, Result<(), PluginError>> {
        Box::pin(async move {
            self.call_method::<_, Ticked>(&TICK, tick).await?;
            Ok(())
        })
    }
}

impl Installed {
    /// Calls `export` with `input` on a fresh instance compiled with `settings`, failing it as a
    /// [`CheckTimedOut`] past [`CALL_TIMEOUT`]. A trap or panic unwinds, so the registry logs why
    /// and fails the check in plain words, as it does when a Built-in Plugin's check panics.
    fn call_fresh(
        &self,
        settings: &Settings,
        export: &'static str,
        input: Vec<u8>,
    ) -> BoxFuture<'static, Result<Vec<u8>, PluginError>> {
        let schema = &self.manifest.settings;
        let hosts = allowed_hosts(&self.package, schema, settings);
        let config = extism_config(schema, settings);
        let wasm = Arc::clone(&self.wasm);
        let cache_config = Arc::clone(&self.cache_config);
        Box::pin(async move {
            // Compiled for this call alone, since the settings may be unsaved: the hosts of their
            // URL settings are allowed for it and gone once it returns.
            let called = task::spawn_blocking(move || {
                let compiled = compile(&wasm, &cache_config, hosts, config)?;
                extism::Plugin::new_from_compiled(&compiled.plugin)?
                    .call::<Vec<u8>, Vec<u8>>(export, input)
            })
            .await;
            match called {
                Ok(Ok(output)) => Ok(output),
                Ok(Err(err)) if is_timeout(&err) => Err(CheckTimedOut(CALL_TIMEOUT).into()),
                Ok(Err(err)) if is_crash(&err) => {
                    panic::resume_unwind(Box::new(failure_cause(&err)))
                }
                Ok(Err(err)) => Err(failure_cause(&err).into()),
                Err(err) if err.is_panic() => panic::resume_unwind(err.into_panic()),
                Err(err) => Err(err.into()),
            }
        })
    }
}

impl SettingsCheck for Installed {
    fn check<'a>(&'a self, settings: &'a Settings) -> BoxFuture<'a, Result<Choices, PluginError>> {
        let called = self.call_fresh(settings, CHECK_SETTINGS, Vec::new());
        Box::pin(async move {
            let checked = CheckedSettings::decode_from_slice(&called.await?).map_err(|err| {
                format!(
                    "its {CHECK_SETTINGS}() returned something that isn't CheckedSettings: {err}"
                )
            })?;
            Ok(choices(checked))
        })
    }
}

impl PersonalSettingsCheck for Installed {
    fn check<'a>(
        &'a self,
        settings: &'a Settings,
        personal: &'a BTreeMap<String, String>,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        let asked = wire::PersonalSettingsCheck {
            personal_settings: personal.clone().into_iter().collect(),
            ..Default::default()
        };
        let called = self.call_fresh(settings, CHECK_PERSONAL_SETTINGS, asked.encode_to_vec());
        Box::pin(async move {
            PersonalSettingsChecked::decode_from_slice(&called.await?).map_err(|err| {
                format!(
                    "its {CHECK_PERSONAL_SETTINGS}() returned something that isn't \
                     PersonalSettingsChecked: {err}"
                )
            })?;
            Ok(())
        })
    }
}

/// The choices `checked` offers, as the Kernel takes them.
fn choices(checked: CheckedSettings) -> Choices {
    checked
        .choices
        .into_iter()
        .map(|(key, offered)| {
            let offered = offered.choices.into_iter().map(|choice| Choice {
                value: choice.value,
                label: choice.label,
            });
            (key, offered.collect())
        })
        .collect()
}

#[cfg(test)]
#[path = "wasm_tests.rs"]
mod tests;
