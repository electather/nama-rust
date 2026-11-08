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
//! [`http_request`] to the hosts its Manifest's `allowed_hosts` and its URL settings name.
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
//!
//! A Plugin that hands Requests to an outside service implements [`Fulfiller`] and exports it with
//! [`fulfiller!`].

use std::collections::HashMap;

pub use extism_pdk::{self, FnResult, plugin_fn};
pub use nama_proto::{
    self,
    nama::{
        plugin::v1::{CheckedSettings, Manifest},
        v1::{Download, Request},
    },
};

use extism_pdk::{Error, FromBytesOwned, HttpRequest, HttpResponse, ToBytes, ToMemory};
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

/// A Fulfiller: hands Approved Movie or Season Requests to an outside service, such as Radarr
/// (ADR 0003). Its Manifest lists `PROVIDE_POINT_MOVIE_FULFILLER`, `PROVIDE_POINT_SHOW_FULFILLER` or
/// both in `providers`, and [`fulfiller!`] exports it:
///
/// ```
/// use std::collections::HashMap;
///
/// use nama_pdk::{Download, Fulfiller, Request, extism_pdk::Error};
///
/// struct Radarr;
///
/// impl Fulfiller for Radarr {
///     fn fulfill(request: Request) -> Result<String, Error> {
///         Ok(format!("radarr-{}", request.movie.tmdb_id))
///     }
///
///     fn progress(_references: Vec<String>) -> Result<HashMap<String, Vec<Download>>, Error> {
///         Ok(HashMap::new())
///     }
/// }
///
/// nama_pdk::fulfiller!(Radarr);
/// ```
pub trait Fulfiller {
    /// Hands `request` to the service and returns at once, without waiting for it to be
    /// fulfilled, with a reference the Server keeps with the Request and passes back to
    /// [`Fulfiller::progress`]. A Plugin providing both kinds tells a Movie Request from a Season
    /// one by which it holds.
    ///
    /// # Errors
    ///
    /// When the service didn't take the Request, such as when it is unreachable or doesn't know
    /// the title. The error's message is the reason the Request shows as Failed.
    fn fulfill(request: Request) -> Result<String, Error>;

    /// What the service is downloading for each of `references`, by reference. A reference with
    /// nothing downloading may be left out.
    ///
    /// # Errors
    ///
    /// When the service couldn't be asked.
    fn progress(references: Vec<String>) -> Result<HashMap<String, Vec<Download>>, Error>;
}

/// Exports a [`Fulfiller`] as the Plugin's `fulfill` and `progress`, taking and returning the
/// `nama.plugin.v1` messages the Server exchanges with them.
#[macro_export]
macro_rules! fulfiller {
    ($fulfiller:ty) => {
        const _: () = {
            // `plugin_fn` expands to paths starting with `extism_pdk`.
            use $crate::{
                Buffa, FnResult, Fulfiller, Request, extism_pdk,
                nama_proto::nama::plugin::v1::{Downloads, Fulfilled, Progress, ProgressAsked},
                plugin_fn,
            };

            #[plugin_fn]
            pub fn fulfill(Buffa(request): Buffa<Request>) -> FnResult<Buffa<Fulfilled>> {
                Ok(Buffa(Fulfilled {
                    reference: <$fulfiller as Fulfiller>::fulfill(request)?,
                    ..Fulfilled::default()
                }))
            }

            #[plugin_fn]
            pub fn progress(Buffa(asked): Buffa<ProgressAsked>) -> FnResult<Buffa<Progress>> {
                let downloads = <$fulfiller as Fulfiller>::progress(asked.references)?
                    .into_iter()
                    .map(|(reference, downloads)| {
                        let downloads = Downloads {
                            downloads,
                            ..Downloads::default()
                        };
                        (reference, downloads)
                    })
                    .collect();
                Ok(Buffa(Progress {
                    downloads,
                    ..Progress::default()
                }))
            }
        };
    };
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

/// Sends `request` with `body`, as [`extism_pdk::http::request`] does. If the Server can't send
/// it, such as when nothing answers at its host, the call fails with `unreachable` as its error,
/// in place of the HTTP client's own words.
///
/// # Errors
///
/// When `request` or `body` can't be passed to the Server.
pub fn http_request<T: ToMemory>(
    request: &HttpRequest,
    body: Option<T>,
    unreachable: &str,
) -> Result<HttpResponse, Error> {
    // The Server ends the call at once when it can't send a request, so the Plugin never sees
    // that error: the call fails with the one set beforehand.
    set_error(unreachable);
    let response = extism_pdk::http::request(request, body);
    clear_error();
    response
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

/// Undoes [`set_error`], so the call in progress succeeds unless it returns an `Err`.
fn clear_error() {
    #[expect(unsafe_code, reason = "Extism has no safe way to clear the error")]
    // SAFETY: Extism reads an error offset of 0 as no error.
    unsafe {
        extism_pdk::extism::error_set(0);
    }
}
