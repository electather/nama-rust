//! Installed Plugins (ADR 0006): `.wasm` files bundled in the Server (ADR 0003) or that an admin
//! drops into `<data_dir>/plugins/`, run through Extism behind the same [`Plugin`] trait as a
//! Built-in Plugin, so the Kernel can't tell them apart.

use std::{
    collections::HashMap,
    io, panic,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
};

use buffa::{DecodeError, Message};
use extism::CompiledPlugin;
use nama_plugin_api::{
    BoxFuture, Manifest, Observer, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    connector::{ConnectionStarted, Connector},
    fulfiller::{Download, Fulfiller, Movies, Request, Shows},
    notifier::{Notification, Notifier, Recipient, RecipientGone},
    points::{Events, ObservedEvents},
    settings::{Choice, Choices, SettingKind, Settings, SettingsCheck},
};
use nama_plugin_proto::{
    CONTRACT_VERSION, SERVICES,
    nama::plugin::v1::{
        self as wire, Channel, CheckedSettings, ConnectionPoll, ConnectionPolled, ConnectionStart,
        Delivered, Delivery, Fulfilled, Progress, ProgressAsked, StopMonitoring,
    },
    services,
};
use semver::{Version, VersionReq};
use thiserror::Error;
use tokio::task;
use tracing::warn;

use super::{CheckTimedOut, registry::Make, secret_values, without_secrets};

mod bundled;
mod contract;
mod listing;
mod load;

pub(crate) use listing::read;
use load::{CALL_TIMEOUT, MEMORY_MAX_PAGES, allowed_hosts, compile, extism_config};

/// How many calls in a row may crash before the Plugin is disabled, so one bad input doesn't
/// cost an admin a Plugin.
const CRASHES_BEFORE_DISABLING: u32 = 3;

/// The export that checks a Plugin's settings, if it has one.
pub(super) const CHECK_SETTINGS: &str = "check_settings";

const FULFILL: &str = "fulfill";
const UNFOLLOW: &str = "unfollow";
/// A Fulfiller's optional export: a Plugin built before it isn't asked.
pub(super) const STOP_MONITORING: &str = "stop_monitoring";
const PROGRESS: &str = "progress";
const DELIVER: &str = "deliver";
const START_CONNECTION: &str = "start_connection";
const POLL_CONNECTION: &str = "poll_connection";
const OBSERVE_EVENTS: &str = "observe_events";

/// The Extension Points an Installed Plugin can serve, one per service of the Plugin contract.
pub(super) const POINTS: &[Point] = &[
    Point {
        service: services::MOVIE_FULFILLER,
        exports: &[FULFILL, PROGRESS],
        key: PointKey::provide::<dyn Fulfiller<Movies>>,
        register: |registrar, running| {
            registrar.provide::<dyn Fulfiller<Movies>>(Priority::DEFAULT, Arc::new(running));
        },
    },
    Point {
        service: services::SHOW_FULFILLER,
        exports: &[FULFILL, UNFOLLOW, PROGRESS],
        key: PointKey::provide::<dyn Fulfiller<Shows>>,
        register: |registrar, running| {
            registrar.provide::<dyn Fulfiller<Shows>>(Priority::DEFAULT, Arc::new(running));
        },
    },
    Point {
        service: services::NOTIFIER,
        exports: &[DELIVER],
        key: PointKey::provide::<dyn Notifier>,
        register: |registrar, running| {
            registrar.provide::<dyn Notifier>(Priority::DEFAULT, Arc::new(running));
        },
    },
    Point {
        service: services::CONNECTOR,
        exports: &[START_CONNECTION, POLL_CONNECTION],
        key: PointKey::provide::<dyn Connector>,
        register: |registrar, running| {
            registrar.provide::<dyn Connector>(Priority::DEFAULT, Arc::new(running));
        },
    },
    Point {
        service: services::EVENT_OBSERVER,
        exports: &[OBSERVE_EVENTS],
        key: PointKey::observe::<Events>,
        register: |registrar, running| registrar.observe::<Events>(running),
    },
];

/// One Extension Point an Installed Plugin can take part at.
pub(super) struct Point {
    /// The service's full name, which a package's Manifest names it by.
    service: &'static str,
    /// The exports the Server calls there, which a package naming it must have.
    exports: &'static [&'static str],
    key: fn() -> PointKey,
    register: fn(&mut Registrar, Running),
}

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
    /// Whether it exports [`STOP_MONITORING`].
    stops_monitoring: bool,
}

/// An enabled Installed Plugin, compiled once with its allowed hosts and settings. Each call into
/// it gets a fresh instance, so one that traps or times out leaves nothing behind for the next.
#[derive(Clone)]
struct Running {
    package: Arc<wire::Manifest>,
    compiled: Arc<CompiledPlugin>,
    /// The values of its secret settings, which the failures it logs mustn't show.
    secrets: Arc<[String]>,
    /// How many calls in a row crashed, counted apart for its Observer, its Fulfiller, its
    /// Notifier and its Connector.
    crashes: Arc<AtomicU32>,
    /// Whether it exports [`STOP_MONITORING`].
    stops_monitoring: bool,
}

impl Running {
    /// Calls the Plugin's export `name` with `input`, a protobuf message, on a fresh instance.
    /// Blocks while the Plugin runs.
    fn call(&self, name: &str, input: Vec<u8>) -> Result<Vec<u8>, extism::Error> {
        extism::Plugin::new_from_compiled(&self.compiled)?.call(name, input)
    }

    /// [`Self::call`] on a blocking thread. A panic there unwinds here, so the registry disables
    /// the Plugin as it does one whose Rust code panics.
    async fn call_blocking(
        &self,
        name: &'static str,
        input: Vec<u8>,
    ) -> Result<Result<Vec<u8>, extism::Error>, task::JoinError> {
        let plugin = self.clone();
        match task::spawn_blocking(move || plugin.call(name, input)).await {
            Err(err) if err.is_panic() => panic::resume_unwind(err.into_panic()),
            called => called,
        }
    }

    /// [`Self::call_blocking`] for a Provider, whose caller gets why it failed, to log or show
    /// such as a Request's reason for being Failed: in the Plugin's own words, or that it crashed.
    /// Counts its crashes as [`Self::count_crash`] does.
    async fn provide(&self, name: &'static str, input: Vec<u8>) -> Result<Vec<u8>, PluginError> {
        let err = match self.call_blocking(name, input).await? {
            Ok(output) => {
                self.crashes.store(0, Ordering::Relaxed);
                return Ok(output);
            }
            Err(err) => err,
        };
        let cause = if is_timeout(&err) {
            format!("it didn't answer within {CALL_TIMEOUT:?}")
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
    /// Plugin as it does one whose Rust code panics.
    fn count_crash(&self, err: &extism::Error, cause: &str) {
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

/// Whether a failed call ran past [`CALL_TIMEOUT`].
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

    // ponytail: compiles on the calling thread, an async one when an admin enables it; wasmtime's
    // cache makes that quick, so spawn_blocking only if a Plugin is slow to enable.
    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        let settings = registrar.settings();
        let schema = &self.manifest.settings;
        let hosts = allowed_hosts(&self.package, schema, settings);
        let config = extism_config(schema, settings);
        let compiled = compile(&self.wasm, &self.cache_config, hosts, config)
            .map_err(|err| format!("could not compile it with its settings: {err:#}"))?;
        let running = Running {
            package: Arc::clone(&self.package),
            compiled: Arc::new(compiled),
            secrets: secret_values(schema, settings).into(),
            crashes: Arc::default(),
            stops_monitoring: self.stops_monitoring,
        };
        // The Kernel refuses a Plugin registering at a point its Manifest doesn't name, so read
        // them from the same list the Kernel checks.
        let declared = &self.manifest.extension_points;
        // Its Movie and Show Fulfillers call the same `fulfill` export, so they count as one.
        let fulfiller_crashes = Arc::<AtomicU32>::default();
        for point in POINTS
            .iter()
            .filter(|point| declared.contains(&(point.key)()))
        {
            // Each counting its own crashes, so one point working between them doesn't hide them.
            let crashes = if point.exports.contains(&FULFILL) {
                Arc::clone(&fulfiller_crashes)
            } else {
                Arc::default()
            };
            (point.register)(
                registrar,
                Running {
                    crashes,
                    ..running.clone()
                },
            );
        }
        Ok(())
    }
}

/// The Extension Points `package` names. One this Server doesn't serve, or doesn't serve at a
/// version the Manifest takes, refuses it, rather than leaving the Plugin doing less than its
/// Manifest says.
pub(super) fn points(package: &wire::Manifest) -> Result<Vec<&'static Point>, Refused> {
    package
        .points
        .iter()
        .map(|named| {
            let point = POINTS
                .iter()
                .find(|point| point.service == named.service)
                .ok_or_else(|| Refused::UnknownPoint(named.service.clone()))?;
            let version = served_version(point.service);
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
fn served_version(service: &str) -> &'static str {
    SERVICES
        .iter()
        .find(|&&(name, _)| name == service)
        .map_or("none", |&(_, version)| version)
}

/// Checks that a Plugin has every export `points` need, `has_export` telling which it has.
pub(super) fn check_exports(
    points: &[&Point],
    has_export: impl Fn(&str) -> bool,
) -> Result<(), Refused> {
    for point in points {
        if let Some(&export) = point.exports.iter().find(|&&export| !has_export(export)) {
            return Err(Refused::MissingExport {
                service: point.service,
                export,
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
        let input = request.encode_to_vec();
        Box::pin(async move {
            let output = self.provide(FULFILL, input).await?;
            Ok(Fulfilled::decode_from_slice(&output)?)
        })
    }

    fn unfollow<'a>(&'a self, request: &'a Request) -> BoxFuture<'a, Result<(), PluginError>> {
        let input = request.encode_to_vec();
        Box::pin(async move { self.provide(UNFOLLOW, input).await.map(drop) })
    }

    fn stop_monitoring<'a>(
        &'a self,
        title: &'a StopMonitoring,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        if !self.stops_monitoring {
            return Box::pin(async { Ok(()) });
        }
        let input = title.encode_to_vec();
        Box::pin(async move { self.provide(STOP_MONITORING, input).await.map(drop) })
    }

    fn progress<'a>(
        &'a self,
        references: &'a [String],
    ) -> BoxFuture<'a, Result<HashMap<String, Vec<Download>>, PluginError>> {
        let input = ProgressAsked {
            references: references.to_vec(),
            ..Default::default()
        }
        .encode_to_vec();
        Box::pin(async move {
            let output = self.provide(PROGRESS, input).await?;
            let downloads = Progress::decode_from_slice(&output)?.downloads;
            Ok(downloads
                .into_iter()
                .map(|(reference, downloads)| (reference, downloads.downloads))
                .collect())
        })
    }
}

impl Notifier for Running {
    fn deliver<'a>(
        &'a self,
        notification: &'a Notification,
        recipient: &'a Recipient,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        let recipient = match recipient {
            Recipient::Channel => wire::delivery::Recipient::from(Channel::default()),
            Recipient::Connection(connection) => {
                wire::delivery::Recipient::Connection(connection.clone())
            }
        };
        let input = Delivery {
            notification: Some(notification.clone()).into(),
            recipient: Some(recipient),
            ..Default::default()
        }
        .encode_to_vec();
        Box::pin(async move {
            let output = self.provide(DELIVER, input).await?;
            if Delivered::decode_from_slice(&output)?.recipient_gone {
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
        let input = ConnectionStart {
            code: code.to_owned(),
            ..Default::default()
        }
        .encode_to_vec();
        Box::pin(async move {
            let output = self.provide(START_CONNECTION, input).await?;
            Ok(ConnectionStarted::decode_from_slice(&output)?)
        })
    }

    fn poll_connection<'a>(
        &'a self,
        pending: &'a str,
    ) -> BoxFuture<'a, Result<Option<String>, PluginError>> {
        let input = ConnectionPoll {
            pending: pending.to_owned(),
            ..Default::default()
        }
        .encode_to_vec();
        Box::pin(async move {
            let output = self.provide(POLL_CONNECTION, input).await?;
            Ok(ConnectionPolled::decode_from_slice(&output)?.connection)
        })
    }
}

impl Observer<Events> for Running {
    fn observe<'a>(&'a self, event: &'a ObservedEvents) -> BoxFuture<'a, ()> {
        let input = event.encode_to_vec();
        Box::pin(async move {
            match self.call_blocking(OBSERVE_EVENTS, input).await {
                Ok(Ok(_)) => self.crashes.store(0, Ordering::Relaxed),
                Ok(Err(err)) => self.observer_failed(&err),
                Err(err) => warn!(plugin = %self.package.id, "observer failed: {err}"),
            }
        })
    }
}

impl SettingsCheck for Installed {
    fn check<'a>(&'a self, settings: &'a Settings) -> BoxFuture<'a, Result<Choices, PluginError>> {
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
                extism::Plugin::new_from_compiled(&compiled)?
                    .call::<(), Vec<u8>>(CHECK_SETTINGS, ())
            })
            .await;
            let output = match called {
                Ok(Ok(output)) => output,
                Ok(Err(err)) if is_timeout(&err) => return Err(CheckTimedOut(CALL_TIMEOUT).into()),
                // So the registry logs why and fails the check in plain words, as it does when a
                // Built-in Plugin's check panics.
                Ok(Err(err)) if is_crash(&err) => {
                    panic::resume_unwind(Box::new(failure_cause(&err)))
                }
                Ok(Err(err)) => return Err(failure_cause(&err).into()),
                Err(err) if err.is_panic() => panic::resume_unwind(err.into_panic()),
                Err(err) => return Err(err.into()),
            };
            let checked = CheckedSettings::decode_from_slice(&output).map_err(|err| {
                format!(
                    "its {CHECK_SETTINGS}() returned something that isn't CheckedSettings: {err}"
                )
            })?;
            Ok(choices(checked))
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
