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
//!
//! A Plugin that tells people about Requests through an outside service implements [`Notifier`] and
//! exports it with [`notifier!`].
//!
//! A Plugin that connects Users to an outside service, such as their Telegram chat, implements
//! [`Connector`] and exports it with [`connector!`].

use std::{collections::HashMap, fmt};

pub use extism_pdk::{self, FnResult, plugin_fn};
pub use nama_proto::{
    self,
    nama::{
        plugin::v1::{
            CheckedSettings, ConnectionStarted, Manifest, Notification, delivery::Recipient,
        },
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
/// `nama-pdk` was built against. Fill in its other fields, such as `name`, `description`,
/// `settings` and `logo_svg` (`include_str!("../logo.svg")`), with struct update syntax.
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
    /// A Season Request naming a Followed Show in `followed_show_id` also asks it to follow the
    /// Show: to fetch the Seasons that come out later too, until [`Fulfiller::unfollow`].
    ///
    /// # Errors
    ///
    /// When the service didn't take the Request, such as when it is unreachable or doesn't know
    /// the title, or the Plugin can't follow a Show it was asked to. The error's message is the
    /// reason the Request shows as Failed.
    fn fulfill(request: Request) -> Result<String, Error>;

    /// Stops following the Show `request` is a Season of, as a hand-off asked it to, since its
    /// Followed Show ended. A Plugin that never follows, such as one for Movies, leaves it be.
    ///
    /// # Errors
    ///
    /// When the service couldn't be told. The Server logs the error's message.
    fn unfollow(_request: Request) -> Result<(), Error> {
        Ok(())
    }

    /// What the service is downloading for each of `references`, by reference. A reference with
    /// nothing downloading may be left out.
    ///
    /// # Errors
    ///
    /// When the service couldn't be asked.
    fn progress(references: Vec<String>) -> Result<HashMap<String, Vec<Download>>, Error>;
}

/// Exports a [`Fulfiller`] as the Plugin's `fulfill`, `unfollow` and `progress`, taking and
/// returning the `nama.plugin.v1` messages the Server exchanges with them.
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
            pub fn unfollow(Buffa(request): Buffa<Request>) -> FnResult<()> {
                Ok(<$fulfiller as Fulfiller>::unfollow(request)?)
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

/// A Notifier: tells people about Requests through an outside service, such as Telegram (ADR
/// 0021). The Server decides who is told what, and hands it each notification with its recipient.
/// Its Manifest lists `PROVIDE_POINT_NOTIFIER` in `providers`, and [`notifier!`] exports it:
///
/// ```
/// use nama_pdk::{Notification, Notifier, Recipient, extism_pdk::Error};
///
/// struct Telegram;
///
/// impl Notifier for Telegram {
///     fn deliver(notification: Notification, recipient: Recipient) -> Result<(), Error> {
///         match recipient {
///             // Format `notification` and post it to the channel in the settings, if there is one.
///             Recipient::Channel(_) => Ok(()),
///             // Send it to the User whose Plugin Connection, such as a chat id, this is.
///             Recipient::Connection(_) => Ok(()),
///         }
///     }
/// }
///
/// nama_pdk::notifier!(Telegram);
/// ```
pub trait Notifier {
    /// Formats `notification` and sends it to `recipient`. A Server with a recipient this
    /// `nama-pdk` doesn't know gets an error instead, so nobody is told what isn't for them.
    ///
    /// # Errors
    ///
    /// [`RecipientGone`] when the service says a [`Recipient::Connection`] is gone. Otherwise when
    /// the service didn't take the notification: the Server logs the error's message and doesn't
    /// retry.
    fn deliver(notification: Notification, recipient: Recipient) -> Result<(), Error>;
}

/// The error a [`Notifier`] returns when the service says a [`Recipient::Connection`] is gone,
/// such as a User who blocked the bot. The Server then deletes that Plugin Connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecipientGone;

impl fmt::Display for RecipientGone {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the recipient is gone")
    }
}

impl std::error::Error for RecipientGone {}

/// Exports a [`Notifier`] as the Plugin's `deliver`, taking the `nama.plugin.v1.Delivery` the
/// Server sends it and returning a `nama.plugin.v1.Delivered`.
#[macro_export]
macro_rules! notifier {
    ($notifier:ty) => {
        const _: () = {
            // `plugin_fn` expands to paths starting with `extism_pdk`.
            use $crate::{
                Buffa, FnResult, Notifier, RecipientGone,
                extism_pdk::{self, Error},
                nama_proto::nama::plugin::v1::{Delivered, Delivery},
                plugin_fn,
            };

            #[plugin_fn]
            pub fn deliver(Buffa(delivery): Buffa<Delivery>) -> FnResult<Buffa<Delivered>> {
                let recipient = delivery
                    .recipient
                    .ok_or_else(|| Error::msg("it doesn't know whom the notification is for"))?;
                let notification = delivery
                    .notification
                    .ok_or_else(|| Error::msg("the Server sent no notification"))?;
                let recipient_gone = match <$notifier as Notifier>::deliver(notification, recipient)
                {
                    Ok(()) => false,
                    Err(error) if error.is::<RecipientGone>() => true,
                    Err(error) => return Err(error.into()),
                };
                Ok(Buffa(Delivered {
                    recipient_gone,
                    ..Delivered::default()
                }))
            }
        };
    };
}

/// A Connector: connects a User to an outside service through a link they open, such as a
/// Telegram deep link (ADR 0021). The Server keeps what it finds as the User's Plugin Connection.
/// Its Manifest lists `PROVIDE_POINT_CONNECTOR` in `providers`, and [`connector!`] exports it:
///
/// ```
/// use nama_pdk::{ConnectionStarted, Connector, extism_pdk::Error};
///
/// struct Telegram;
///
/// impl Connector for Telegram {
///     fn start_connection(code: String) -> Result<ConnectionStarted, Error> {
///         Ok(ConnectionStarted {
///             url: format!("https://t.me/my_bot?start={code}"),
///             pending: code,
///             ..ConnectionStarted::default()
///         })
///     }
///
///     fn poll_connection(_pending: String) -> Result<Option<String>, Error> {
///         // Look for `/start <code>` among the bot's messages, and return its chat id.
///         Ok(None)
///     }
/// }
///
/// nama_pdk::connector!(Telegram);
/// ```
pub trait Connector {
    /// The link a User opens to connect, made with `code`, a fresh random one-time code from the
    /// Server, and what the Server passes back to [`Connector::poll_connection`] while it waits.
    ///
    /// # Errors
    ///
    /// When the service couldn't be asked. The Server logs the error's message.
    fn start_connection(code: String) -> Result<ConnectionStarted, Error>;

    /// The User's identity at the service once they finished with the link `pending` came from,
    /// or `None` while they haven't. The Server asks every few seconds for up to 10 minutes.
    ///
    /// # Errors
    ///
    /// When the service couldn't be asked. The Server logs the error's message and asks again.
    fn poll_connection(pending: String) -> Result<Option<String>, Error>;
}

/// Exports a [`Connector`] as the Plugin's `start_connection` and `poll_connection`, taking and
/// returning the `nama.plugin.v1` messages the Server exchanges with them.
#[macro_export]
macro_rules! connector {
    ($connector:ty) => {
        const _: () = {
            // `plugin_fn` expands to paths starting with `extism_pdk`.
            use $crate::{
                Buffa, ConnectionStarted, Connector, FnResult, extism_pdk,
                nama_proto::nama::plugin::v1::{ConnectionPoll, ConnectionPolled, ConnectionStart},
                plugin_fn,
            };

            #[plugin_fn]
            pub fn start_connection(
                Buffa(start): Buffa<ConnectionStart>,
            ) -> FnResult<Buffa<ConnectionStarted>> {
                Ok(Buffa(<$connector as Connector>::start_connection(
                    start.code,
                )?))
            }

            #[plugin_fn]
            pub fn poll_connection(
                Buffa(poll): Buffa<ConnectionPoll>,
            ) -> FnResult<Buffa<ConnectionPolled>> {
                Ok(Buffa(ConnectionPolled {
                    connection: <$connector as Connector>::poll_connection(poll.pending)?,
                    ..ConnectionPolled::default()
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
