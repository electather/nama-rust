//! Plugin development kit for Installed Plugins (ADR 0006, ADR 0009).
//!
//! An Installed Plugin is a `cdylib` built for `wasm32-unknown-unknown`. Each of its exports is a
//! [`plugin_fn`] that takes and returns `nama.plugin.v1` messages wrapped in [`Buffa`]. Every
//! Plugin exports `manifest()`, which returns its [`Manifest`]:
//!
//! ```
//! // `plugin_fn` expands to paths starting with `extism_pdk`.
//! use nama_pdk::{Buffa, FnResult, Manifest, extism_pdk, plugin_fn};
//!
//! #[plugin_fn]
//! pub fn manifest() -> FnResult<Buffa<Manifest>> {
//!     Ok(Buffa(nama_pdk::manifest("hello", env!("CARGO_PKG_VERSION"))))
//! }
//! ```
//!
//! The Plugin reads its settings with [`extism_pdk::config::get`], and sends HTTP requests with
//! [`extism_pdk::http::request`] to the hosts its Manifest's `allowed_hosts` and its URL settings
//! name.
//!
//! A Plugin with [`SETTING_KIND_CHOICE`](nama_proto::nama::plugin::v1::SettingKind) settings exports `check_settings()`, which
//! checks its settings against the outside service and offers the choices for each, built with
//! [`checked_settings`]. It runs only when an admin tests settings, which may be unsaved, and may
//! reach the hosts of the URL settings tested:
//!
//! ```
//! use nama_pdk::{Buffa, CheckedSettings, FnResult, extism_pdk, plugin_fn};
//!
//! #[plugin_fn]
//! pub fn check_settings() -> FnResult<Buffa<CheckedSettings>> {
//!     let profiles = [("1", "HD"), ("4", "Ultra HD")];
//!     Ok(Buffa(nama_pdk::checked_settings([("profile", profiles)])))
//! }
//! ```
//!
//! If the Plugin panics, the Server gets the panic's message with the error of the call that
//! panicked, not just a bare trap.

pub use extism_pdk::{self, FnResult, plugin_fn};
pub use nama_proto::{
    self,
    nama::plugin::v1::{CheckedSettings, Manifest},
};

use extism_pdk::{Error, FromBytesOwned, ToBytes};
use nama_proto::nama::v1::{Choice, SettingChoices};

/// Wraps a protobuf message so it crosses an Extism call encoded as protobuf, as an export's
/// input or output.
#[derive(Debug)]
pub struct Buffa<T>(pub T);

impl<T: buffa::Message> ToBytes<'_> for Buffa<T> {
    type Bytes = Vec<u8>;

    fn to_bytes(&self) -> Result<Self::Bytes, Error> {
        Ok(self.0.try_encode_to_vec()?)
    }
}

impl<T: buffa::Message> FromBytesOwned for Buffa<T> {
    fn from_bytes_owned(data: &[u8]) -> Result<Self, Error> {
        Ok(Self(T::decode_from_slice(data)?))
    }
}

/// A Manifest for the Plugin `id` at `version`, which works with the Nama API versions this
/// `nama-pdk` was built against. Fill in its other fields, such as `settings` and `observers`,
/// with struct update syntax.
#[must_use]
pub fn manifest(id: &str, version: &str) -> Manifest {
    Manifest {
        id: id.to_owned(),
        version: version.to_owned(),
        nama_api: format!("^{}", nama_proto::NAMA_API_VERSION),
        ..Manifest::default()
    }
}

/// What `check_settings()` returns: for each Choice setting's key, its choices as `(value, label)`
/// pairs in the order a form shows them. The value is what is stored, such as a quality profile's
/// id, and the label what an admin picks it by, such as its name.
#[must_use]
pub fn checked_settings<K, C, V, L>(choices: impl IntoIterator<Item = (K, C)>) -> CheckedSettings
where
    K: Into<String>,
    C: IntoIterator<Item = (V, L)>,
    V: Into<String>,
    L: Into<String>,
{
    let setting_choices = |pairs: C| SettingChoices {
        choices: pairs
            .into_iter()
            .map(|(value, label)| Choice {
                value: value.into(),
                label: label.into(),
                ..Choice::default()
            })
            .collect(),
        ..SettingChoices::default()
    };
    CheckedSettings {
        choices: choices
            .into_iter()
            .map(|(key, pairs)| (key.into(), setting_choices(pairs)))
            .collect(),
        ..CheckedSettings::default()
    }
}

/// Installs the panic hook. Extism calls a guest's `_initialize` export whenever it instantiates
/// it, so every instance has the hook before its first call.
#[cfg(target_arch = "wasm32")]
#[expect(
    unsafe_code,
    reason = "`_initialize` must keep its name for Extism to find it, which takes `no_mangle`"
)]
#[unsafe(no_mangle)]
pub extern "C" fn _initialize() {
    // A panic's message puts its location on a line of its own; the Server logs it on one.
    std::panic::set_hook(Box::new(|info| {
        set_error(&info.to_string().replace('\n', " "));
    }));
}

/// Makes `message` the error of the call in progress, as `plugin_fn` does for an `Err`. The trap
/// that follows a panic keeps it, and the Server reads it as the call's error.
#[cfg(target_arch = "wasm32")]
fn set_error(message: &str) {
    let Ok(memory) = extism_pdk::Memory::from_bytes(message) else {
        return;
    };
    #[expect(
        unsafe_code,
        reason = "Extism has no safe way to set the error; `plugin_fn` does the same"
    )]
    // SAFETY: `memory` was allocated by Extism and holds the message, as `error_set` requires.
    unsafe {
        extism_pdk::extism::error_set(memory.offset());
    }
}
