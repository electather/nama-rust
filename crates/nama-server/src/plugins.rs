//! The plugin registry (ADR 0006): which Plugins are enabled, what each registered, and how the
//! Kernel calls them at Extension Points.

mod blob_store;
mod builtin;
mod contract;
mod failing;
mod points;
mod registry;
mod retrying;
mod scope;
mod tick;
mod wasm;

use std::{
    any::Any,
    future::Future,
    panic::{self, AssertUnwindSafe},
    sync::Arc,
    time::Duration,
};

use futures_util::FutureExt;
use nama_plugin_api::{
    Manifest, Plugin, PluginId,
    settings::{SettingKind, Settings, SettingsError, SettingsSchema},
};
use nama_proto::nama::v1::PluginKind;
use thiserror::Error;
use tracing::{Instrument as _, Span, error, warn};

use crate::{
    diagnostics,
    images::{MAX_SVG_BYTES, clean_svg},
};

pub(crate) use blob_store::PluginBlobs;
pub(crate) use contract::{download, notification, observed, request};
pub use failing::Failing;
pub use points::ExtensionPoints;
pub use registry::Registry;
pub(crate) use retrying::{RETRIES, Retryable};
pub(crate) use wasm::read as read_installed;

/// How long a Plugin's settings check may take before it counts as failed.
pub const SETTINGS_CHECK_TIMEOUT: Duration = Duration::from_secs(10);

/// A Plugin the Server has, as [`Registry::plugins`] lists it.
#[derive(Debug, Clone)]
pub struct Listed {
    /// Its unique id, or for an Installed Plugin the Server refused, its file's name.
    pub id: PluginId,
    /// Whether it is Built-in or Installed.
    pub kind: PluginKind,
    /// What it said about itself when the Server read it, without a logo the Server dropped, or
    /// why the Server refused it, in which case it can't be enabled.
    pub manifest: Result<Manifest, String>,
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

/// A Provider's answer, and the Plugin whose Provider gave it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer<R, K = ()> {
    /// The Plugin whose Provider answered.
    pub plugin: PluginId,
    /// What it answered, or why it failed.
    pub result: Result<R, Failure<K>>,
}

/// Why a Provider failed, as the Kernel's own data: the Plugin's error was formatted and dropped
/// inside containment, and the message has the Plugin's secret settings blanked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure<K = ()> {
    /// The kind of failure, for a caller that branches on it.
    pub kind: K,
    /// What the Plugin's error said.
    pub message: String,
}

impl<K> Failure<K> {
    fn blanked(self, secrets: &[String]) -> Self {
        let message = without_secrets(self.message, secrets);
        Self { message, ..self }
    }
}

/// An error a Provider call may end in, which the Registry turns into a [`Failure`] of the kind
/// `K` before it leaves containment.
// `K` is a parameter, not an associated type, so a caller's own kind can stay private to it.
pub trait ProviderError<K> {
    /// What kind of failure this is and what it says. Called inside containment, since formatting
    /// and dropping a Plugin's error run Plugin code.
    fn failure(self) -> Failure<K>;
}

// For any lifetime, not just `PluginError`'s `'static`: an `async` caller's error may be inferred
// with a shorter one.
impl ProviderError<()> for Box<dyn std::error::Error + Send + Sync + '_> {
    fn failure(self) -> Failure {
        Failure {
            kind: (),
            message: self.to_string(),
        }
    }
}

/// Why a Plugin couldn't be enabled.
#[derive(Debug, Error)]
pub enum EnableError {
    /// No Plugin has the id.
    #[error("there is no plugin {0}")]
    Unknown(PluginId),
    /// The Server refused the Plugin, for the reason given.
    #[error("the Server refused plugin {plugin}: {reason}")]
    Refused {
        /// The Plugin.
        plugin: PluginId,
        /// Why the Server refused it.
        reason: String,
    },
    /// A Plugin with the same id is already enabled.
    #[error("plugin {0} is already enabled")]
    AlreadyEnabled(PluginId),
    /// The Plugin panicked in [`Plugin::manifest`], so it has no id to report.
    #[error("a plugin panicked while giving its manifest: {message}")]
    ManifestPanicked {
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
    /// The Plugin provides a Fulfiller for a kind of title another enabled Plugin already provides
    /// one for. The Kernel hands each Request to one Fulfiller, so each kind has at most one.
    #[error("plugin {plugin} provides a {kind} Fulfiller, which plugin {by} already provides")]
    FulfillerTaken {
        /// The Plugin.
        plugin: PluginId,
        /// The kind of title, `Movie` or `Show`.
        kind: &'static str,
        /// The enabled Plugin that provides it.
        by: PluginId,
    },
    /// The Plugin panicked while being enabled, such as in [`Plugin::register`].
    #[error("plugin {plugin} panicked while being enabled: {message}")]
    Panicked {
        /// The Plugin.
        plugin: PluginId,
        /// The panic's message.
        message: String,
    },
    /// The Plugin's [`Plugin::register`] refused to start with its settings.
    #[error("plugin {plugin} refused to start: {reason}")]
    RefusedToStart {
        /// The Plugin.
        plugin: PluginId,
        /// The Plugin's error, with no secret setting's value in it.
        reason: String,
    },
}

/// Why a Plugin's settings check offered no choices.
#[derive(Debug, Error)]
pub enum CheckError {
    /// No Plugin has the id.
    #[error("there is no plugin {0}")]
    Unknown(PluginId),
    /// The settings don't fit the Plugin's schema.
    #[error("the settings don't fit the plugin: {0}")]
    InvalidSettings(SettingsError),
    /// The check failed or panicked, or the Plugin has none. The message is in plain words for an
    /// admin, with no secret setting's value in it.
    #[error("{0}")]
    Failed(String),
    /// The check ran past the time it had: [`SETTINGS_CHECK_TIMEOUT`], or less for an Installed
    /// Plugin, whose every call has a shorter one.
    #[error(
        "{0} didn't finish checking its settings within {seconds} seconds",
        seconds = .1.as_secs()
    )]
    TimedOut(PluginId, Duration),
}

/// What a settings check the Server runs itself, such as an Installed Plugin's, fails with when
/// it runs past the time it had, so the registry refuses it as [`CheckError::TimedOut`].
#[derive(Debug, Error)]
#[error("ran past {} seconds", .0.as_secs())]
struct CheckTimedOut(Duration);

/// The value of each secret in `settings`, leaving out blank ones.
fn secret_values(schema: &SettingsSchema, settings: &Settings) -> Vec<String> {
    schema
        .fields
        .iter()
        .filter(|field| field.kind == SettingKind::Secret)
        .filter_map(|field| settings.text(&field.key))
        .filter(|secret| !secret.is_empty())
        .map(str::to_owned)
        .collect()
}

/// `message` with each of `secrets` blanked out, since a Plugin's error may quote one, such as in
/// a URL it fetched.
pub(crate) fn without_secrets(mut message: String, secrets: &[String]) -> String {
    // An empty secret would blank the gap between every character.
    for secret in secrets.iter().filter(|secret| !secret.is_empty()) {
        message = message.replace(secret.as_str(), "…");
    }
    message
}

/// Logs why the Plugin `id` wasn't enabled at start.
pub(crate) fn log_not_enabled(id: &PluginId, err: &EnableError) {
    // Not an error: a Plugin such as TMDB needs an API key before it can run.
    if matches!(err, EnableError::InvalidSettings { .. }) {
        warn!(plugin = %id, "plugin not enabled: {err}");
    } else {
        error!(plugin = %id, "could not enable plugin: {err}");
    }
}

/// What the Built-in Plugin `make` makes says about itself: its Manifest and whether it has a
/// settings check, or `None`, logged, if it panics being made or giving them. All of it is Plugin
/// code, so a panic in it is contained.
fn what_it_says(make: &dyn Fn() -> Arc<dyn Plugin>) -> Option<(Manifest, bool)> {
    let plugin = contain_sync(make)
        .inspect_err(|message| error!(message, "a plugin panicked while being made"))
        .ok()?;
    let manifest = match contain_sync(|| plugin.manifest()) {
        Ok(manifest) => manifest,
        Err(message) => {
            error!(message, "a plugin panicked while giving its manifest");
            // A panic dropping it is moot: it said nothing about itself.
            let _dropped = contain_sync(|| drop(plugin));
            return None;
        }
    };
    let span = points::builtin_log_span(&manifest.id);
    let checks_settings = span.in_scope(|| contain_sync(|| plugin.settings_check().is_some()));
    // A panic dropping it is moot: what it said about itself is already read.
    let _dropped = span.in_scope(|| contain_sync(|| drop(plugin)));
    let checks_settings = checks_settings
        .inspect_err(|message| {
            error!(plugin = %manifest.id, message, "a plugin panicked while describing itself");
        })
        .ok()?;
    Some((manifest, checks_settings))
}

/// The Plugin's `logo` as [`clean_svg`] leaves it, if the Server keeps it: at most
/// [`MAX_SVG_BYTES`] and an SVG document. Logs why it drops one, which leaves the Plugin without a
/// logo but otherwise as it was. An empty one, as protobuf sends no logo, is none.
fn kept_logo(plugin: &PluginId, logo: &str) -> Option<String> {
    if logo.is_empty() {
        return None;
    }
    let why = if logo.len() > MAX_SVG_BYTES {
        format!("it is over {MAX_SVG_BYTES} bytes")
    } else {
        match clean_svg(logo.as_bytes()) {
            // svg-hush writes UTF-8 whatever it reads, so nothing is lost.
            Ok(cleaned) => return Some(String::from_utf8_lossy(&cleaned).into_owned()),
            Err(err) => format!("it isn't SVG: {err}"),
        }
    };
    warn!(plugin = %plugin, "dropped its logo: {why}");
    None
}

/// `answer`, its error turned into a [`Failure`] once it is ready, for [`contain_call`] to await
/// and mark its span failed.
async fn formatted<R, K, E: ProviderError<K>>(
    answer: impl Future<Output = Result<R, E>>,
) -> Result<R, Failure<K>> {
    answer.await.map_err(|err| {
        diagnostics::failed(&Span::current(), std::any::type_name::<E>());
        err.failure()
    })
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

/// [`contain`]s a call into the Plugin `plugin` at `point`, an Extension Point or its settings
/// check, in a span naming both, marked failed if it panicked.
async fn contain_call<F: Future>(
    plugin: &PluginId,
    point: &str,
    call: impl FnOnce() -> F,
) -> Result<F::Output, String> {
    // A Provider's point is named after its trait's path, such as
    // `dyn nama_plugin_api::auth::Authenticator`.
    let point = point.rsplit("::").next().unwrap_or(point);
    let span = tracing::info_span!(
        "plugin call",
        otel.name = format!("plugin {point}"),
        plugin.id = %plugin,
        plugin.point = point,
    );
    let contained = contain(call).instrument(span.clone()).await;
    if contained.is_err() {
        diagnostics::failed(&span, "panicked");
    }
    contained
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
