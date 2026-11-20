//! Plugin development kit for Installed Plugins (ADR 0006, ADR 0009).
//!
//! The crate's `README.md` walks through writing a Plugin with it, from the Manifest to the `.wasm`.

// Compiles and runs the README's examples with this crate's doctests.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeExamples;

use std::{collections::HashMap, fmt, time::Duration};

pub use ::extism_pdk::{FnResult, plugin_fn};
pub use nama_plugin_proto::{
    self,
    nama::plugin::v1::{
        Artwork, ArtworkAnswer, ArtworkAsked, ArtworkKind, Candidate, CheckedSettings,
        ConnectionStarted, Download, Episode, EpisodeAnswer, EpisodeAsked, Fulfilled, Image,
        ImageAnswer, ImageAsked, Manifest, Movie, MovieAnswer, Notification, Point, Request,
        SearchQuery, SearchResults, Season, SeasonAnswer, SeasonAsked, Show, ShowAnswer,
        StopMonitoring, TitleAsked, TitleId, TitleKind, delivery::Recipient,
    },
    services,
};

mod automation;
mod blob_store;
mod home_row;
mod log;
mod notification_text;
mod recogniser;
mod subtitle_source;
#[cfg(not(target_arch = "wasm32"))]
pub mod testing;

pub use automation::{
    Automation, Library, LibraryKind, Permission, Tick, Title, TitleRef, TitleRefKind,
    TitleRequesters, TitleWatchStates, Titles, User, WatchState, kernel,
};
pub use blob_store::{BLOB_MAX_BYTES, BlobPage, BlobStore};
pub use home_row::{HomeRow, HomeRowSource, HomeRowTitle, HomeRows, UserRowsAsked};
pub use recogniser::{
    FileRecognised, FileToRecognise, Recognised, RecognisedEpisodes, RecognisedMovie, Recogniser,
    TitleRecognised, TitleToRecognise,
};
pub use subtitle_source::{
    SubtitleDownload, SubtitleFile, SubtitleResult, SubtitleResults, SubtitleSearch, SubtitleSource,
};

#[doc(hidden)]
pub use log::write_log_line;

/// Extism's PDK, as `plugin_fn` and the Plugins use it, without its log macros: a Plugin logs
/// with [`info!`] and the others, which the Server names its Plugin in, blanks its secrets in and
/// bounds (ADR 0061).
///
/// It also lacks Extism's `config`, `http` and `HttpResponse`: a Plugin reads a setting with
/// [`setting`] and sends a request with [`http_request`], which a test can answer (ADR 0065).
///
/// ```compile_fail
/// nama_pdk::extism_pdk::info!("Extism's own log is closed");
/// ```
///
/// ```compile_fail
/// nama_pdk::extism_pdk::config::get("url");
/// ```
///
/// ```compile_fail
/// let request = nama_pdk::extism_pdk::HttpRequest::new("http://service.test");
/// nama_pdk::extism_pdk::http::request(&request, None::<String>);
/// ```
///
/// ```compile_fail
/// fn answered(_response: nama_pdk::extism_pdk::HttpResponse) {}
/// ```
pub mod extism_pdk {
    pub use ::extism_pdk::*;

    #[expect(hidden_glob_reexports, reason = "closing Extism's to a Plugin")]
    mod config {}
    #[expect(hidden_glob_reexports, reason = "closing Extism's to a Plugin")]
    mod http {}
    #[expect(
        dead_code,
        hidden_glob_reexports,
        reason = "closing Extism's to a Plugin"
    )]
    enum HttpResponse {}

    // Named, these shadow the glob's macros of the same names.
    #[doc(hidden)]
    pub use crate::{
        __extism_log_is_closed as debug, __extism_log_is_closed as error,
        __extism_log_is_closed as info, __extism_log_is_closed as log,
        __extism_log_is_closed as trace, __extism_log_is_closed as warn,
    };
}

/// Stands in for each of Extism's log macros in [`extism_pdk`], so calling one fails to build.
#[doc(hidden)]
#[macro_export]
macro_rules! __extism_log_is_closed {
    ($($arg:tt)*) => {
        ::std::compile_error!(
            "log with nama_pdk::{debug!, info!, warn!, error!}, not Extism's macros"
        )
    };
}

/// What a notification says happened, in the Glossary's words, or `None` for what isn't told.
pub use notification_text::what_happened;

use ::extism_pdk::{Error, FromBytesOwned, HttpRequest, ToBytes};
use nama_plugin_proto::nama::plugin::v1::{Choice, SettingChoices};

/// The value of the id in `namespace` among a title's `ids`, such as its `tmdb` id, if it has one.
#[must_use]
pub fn title_id<'a>(ids: &'a [TitleId], namespace: &str) -> Option<&'a str> {
    ids.iter()
        .find(|id| id.namespace == namespace)
        .map(|id| id.value.as_str())
}

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

/// A Manifest for the Plugin `id` at `version`, which works with the Plugin contract versions this
/// `nama-pdk` was built against. Fill in its other fields, such as `name`, `description`,
/// `settings` and `logo_svg` (`include_str!("../logo.svg")`), with struct update syntax.
#[must_use]
pub fn manifest(id: &str, version: &str) -> Manifest {
    Manifest {
        id: id.to_owned(),
        version: version.to_owned(),
        contract: format!("^{}", nama_plugin_proto::CONTRACT_VERSION),
        ..Manifest::default()
    }
}

/// The Extension Point `service`, one of [`services`], at the versions this `nama-pdk` was built
/// against, for a Manifest's `points`:
///
/// ```
/// use nama_pdk::{Manifest, services};
///
/// let manifest = Manifest {
///     points: vec![nama_pdk::point(services::NOTIFIER)],
///     ..nama_pdk::manifest("hello", "0.1.0")
/// };
/// ```
///
/// # Panics
///
/// When `service` isn't one of [`services`]: a Plugin's own bug, which its first `manifest()`
/// call finds.
#[must_use]
#[expect(
    clippy::panic,
    reason = "an unknown service is the Plugin author's bug"
)]
pub fn point(service: &str) -> Point {
    let &(service, version) = nama_plugin_proto::SERVICES
        .iter()
        .find(|&&(name, _)| name == service)
        .unwrap_or_else(|| panic!("{service} isn't an Extension Point of the Plugin contract"));
    Point {
        service: service.to_owned(),
        version: format!("^{version}"),
        ..Point::default()
    }
}

/// A Fulfiller: hands Approved Movie or Season Requests to an outside service, such as Radarr
/// (ADR 0003). Its Manifest's `points` name [`services::MOVIE_FULFILLER`],
/// [`services::SHOW_FULFILLER`] or both, built with [`point`]; it names the id namespaces it can
/// find a title by in `movie_id_namespaces` and `show_id_namespaces`, such as `tmdb`, and
/// [`fulfiller!`] exports it:
///
/// ```
/// use std::collections::HashMap;
///
/// use nama_pdk::{
///     Download, Fulfilled, Fulfiller, Request, extism_pdk::Error,
///     nama_plugin_proto::nama::plugin::v1::request::MovieOrSeason, title_id,
/// };
///
/// struct Radarr;
///
/// impl Fulfiller for Radarr {
///     fn fulfill(request: Request) -> Result<Fulfilled, Error> {
///         let Some(MovieOrSeason::Movie(movie)) = request.movie_or_season else {
///             return Err(Error::msg("Radarr takes only Movie Requests"));
///         };
///         let Some(tmdb) = title_id(&movie.ids, "tmdb") else {
///             return Err(Error::msg("Radarr needs the Movie's TMDB id"));
///         };
///         Ok(Fulfilled {
///             reference: format!("radarr-{tmdb}"),
///             ..Fulfilled::default()
///         })
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
    /// [`Fulfiller::progress`], and the quality profile the service used. A Plugin providing both
    /// kinds tells a Movie Request from a Season one by which it holds.
    ///
    /// A Season Request naming a Followed Show in `followed_show_id` also asks it to follow the
    /// Show: to fetch the Seasons that come out later too, until [`Fulfiller::unfollow`].
    ///
    /// # Errors
    ///
    /// [`TryAgainIn`] when the service can't take it yet, such as Sonarr still adding the Show:
    /// the Server asks again with the same Request after that long. Otherwise when the service
    /// didn't take the Request, such as when it is unreachable or doesn't know the title or the
    /// quality profile `request` names, or the Plugin can't follow a Show it was asked to. The
    /// error's message is the reason the Request shows as Failed.
    fn fulfill(request: Request) -> Result<Fulfilled, Error>;

    /// Stops following the Show `request` is a Season of, as a hand-off asked it to, since its
    /// Followed Show ended. A Plugin that never follows, such as one for Movies, leaves it be.
    ///
    /// # Errors
    ///
    /// [`TryAgainIn`] when the service can't be told yet: the Server asks again after that long.
    /// Otherwise when the service couldn't be told. The Server logs the error's message.
    fn unfollow(_request: Request) -> Result<(), Error> {
        Ok(())
    }

    /// Stops the service monitoring `title`, which is being deleted, so it doesn't download it
    /// again. The service keeps the title, and one it doesn't have means nothing to do. Separate
    /// from [`Fulfiller::unfollow`]: a Show's new Seasons still come.
    ///
    /// # Errors
    ///
    /// [`TryAgainIn`] when the service can't be told yet: the Server asks again after that long.
    /// Otherwise when the service couldn't be told. The delete goes ahead, and the person deleting
    /// is told the service may download the title again.
    fn stop_monitoring(_title: StopMonitoring) -> Result<(), Error> {
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

/// The error a [`Fulfiller`] returns from `fulfill`, `unfollow` or `stop_monitoring` when the
/// service can't act yet, such as Sonarr still adding a Show: the Server asks again with the same
/// input after this long, clamped to 1 to 300 seconds, while the Plugin stays enabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TryAgainIn(pub Duration);

impl fmt::Display for TryAgainIn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "try again in {} seconds", self.0.as_secs())
    }
}

impl std::error::Error for TryAgainIn {}

impl TryAgainIn {
    /// The `retry_after_seconds` a [`fulfiller!`] export answers for `error`: its whole seconds
    /// if it is a [`TryAgainIn`], saturating at `u32::MAX`, or `None` for any other error.
    #[doc(hidden)]
    #[must_use]
    pub fn seconds_in(error: &Error) -> Option<u32> {
        error
            .downcast_ref::<Self>()
            .map(|Self(wait)| u32::try_from(wait.as_secs()).unwrap_or(u32::MAX))
    }
}

/// Exports a [`Fulfiller`] as the Plugin's `fulfill`, `unfollow`, `stop_monitoring` and `progress`,
/// taking and returning the `nama.plugin.v1` messages the Server exchanges with them. A
/// [`TryAgainIn`] error from the first three becomes their answer's `retry_after_seconds`.
#[macro_export]
macro_rules! fulfiller {
    ($fulfiller:ty) => {
        const _: () = {
            // `plugin_fn` expands to paths starting with `extism_pdk`.
            use $crate::{
                Buffa, FnResult, Fulfiller, Request, StopMonitoring, TryAgainIn, extism_pdk,
                nama_plugin_proto::nama::plugin::v1::{
                    Downloads, Fulfilled, Progress, ProgressAsked, StoppedMonitoring, Unfollowed,
                },
                plugin_fn,
            };

            #[plugin_fn]
            pub fn fulfill(Buffa(request): Buffa<Request>) -> FnResult<Buffa<Fulfilled>> {
                match <$fulfiller as Fulfiller>::fulfill(request) {
                    Ok(fulfilled) => Ok(Buffa(fulfilled)),
                    Err(error) => match TryAgainIn::seconds_in(&error) {
                        Some(seconds) => Ok(Buffa(Fulfilled {
                            retry_after_seconds: Some(seconds),
                            ..Fulfilled::default()
                        })),
                        None => Err(error.into()),
                    },
                }
            }

            #[plugin_fn]
            pub fn unfollow(Buffa(request): Buffa<Request>) -> FnResult<Buffa<Unfollowed>> {
                match <$fulfiller as Fulfiller>::unfollow(request) {
                    Ok(()) => Ok(Buffa(Unfollowed::default())),
                    Err(error) => match TryAgainIn::seconds_in(&error) {
                        Some(seconds) => Ok(Buffa(Unfollowed {
                            retry_after_seconds: Some(seconds),
                            ..Unfollowed::default()
                        })),
                        None => Err(error.into()),
                    },
                }
            }

            #[plugin_fn]
            pub fn stop_monitoring(
                Buffa(title): Buffa<StopMonitoring>,
            ) -> FnResult<Buffa<StoppedMonitoring>> {
                match <$fulfiller as Fulfiller>::stop_monitoring(title) {
                    Ok(()) => Ok(Buffa(StoppedMonitoring::default())),
                    Err(error) => match TryAgainIn::seconds_in(&error) {
                        Some(seconds) => Ok(Buffa(StoppedMonitoring {
                            retry_after_seconds: Some(seconds),
                            ..StoppedMonitoring::default()
                        })),
                        None => Err(error.into()),
                    },
                }
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
/// Its Manifest's `points` name [`services::NOTIFIER`], and [`notifier!`] exports it:
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
///             // Send it to a User by the values they set for the Manifest's `personal_settings`,
///             // such as their own topic. A Plugin with no Personal settings fails here.
///             Recipient::User(user) => {
///                 let _topic = user.personal_settings.get("topic");
///                 Ok(())
///             }
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
    /// [`RecipientGone`] when the service says the recipient is gone. Otherwise when the service
    /// didn't take the notification: the Server logs the error's message and doesn't retry.
    fn deliver(notification: Notification, recipient: Recipient) -> Result<(), Error>;
}

/// The error a [`Notifier`] returns when the service says a recipient is gone, such as a User who
/// blocked the bot or a deleted webhook. For a [`Recipient::Connection`] the Server then deletes
/// that Plugin Connection; for a [`Recipient::User`] it deletes that User's Personal settings for
/// the Plugin.
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
                nama_plugin_proto::nama::plugin::v1::{Delivered, Delivery},
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
/// Its Manifest's `points` name [`services::CONNECTOR`], and [`connector!`] exports it:
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
                nama_plugin_proto::nama::plugin::v1::{
                    ConnectionPoll, ConnectionPolled, ConnectionStart,
                },
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

/// A title source: tells the Server about Movies and Shows so it can Match the files in a Library
/// to titles. Every method is optional, and only those [`title_source!`] lists are exported; the
/// Server asks nothing of one that isn't.
///
/// Asked by an id in a namespace it can't look titles up by, a method answers with
/// `unsupported_id` set rather than failing, and the Server asks the next source; a failure means
/// the source couldn't answer, which is worth trying again. The Manifest's `title_namespace` and
/// `title_lookup_namespaces` say which namespaces it looks titles up by:
///
/// ```
/// use nama_pdk::{
///     Candidate, SearchQuery, SearchResults, TitleId, TitleSource, extism_pdk::Error,
/// };
///
/// struct Tvdb;
///
/// impl TitleSource for Tvdb {
///     fn search(query: SearchQuery) -> Result<SearchResults, Error> {
///         let id = TitleId {
///             namespace: "tvdb".to_owned(),
///             value: "81189".to_owned(),
///             ..TitleId::default()
///         };
///         Ok(SearchResults {
///             candidates: vec![Candidate {
///                 ids: vec![id.clone()],
///                 id: id.into(),
///                 title: query.title,
///                 ..Candidate::default()
///             }],
///             ..SearchResults::default()
///         })
///     }
/// }
///
/// nama_pdk::title_source!(Tvdb, [search]);
/// ```
pub trait TitleSource {
    /// The titles of `query.kind` that best fit `query`, best first; none if nothing fits.
    ///
    /// # Errors
    ///
    /// When the source couldn't be asked, which is worth trying again later.
    fn search(_query: SearchQuery) -> Result<SearchResults, Error> {
        Err(not_served("search"))
    }

    /// The Movie with the id `asked` names, unset if the source has none.
    ///
    /// # Errors
    ///
    /// When the source couldn't be asked.
    fn movie(_asked: TitleAsked) -> Result<MovieAnswer, Error> {
        Err(not_served("movie"))
    }

    /// The Show with the id `asked` names, unset if the source has none.
    ///
    /// # Errors
    ///
    /// When the source couldn't be asked.
    fn show(_asked: TitleAsked) -> Result<ShowAnswer, Error> {
        Err(not_served("show"))
    }

    /// The Season `asked` names, with its Episodes, unset if the source has none.
    ///
    /// # Errors
    ///
    /// When the source couldn't be asked.
    fn season(_asked: SeasonAsked) -> Result<SeasonAnswer, Error> {
        Err(not_served("season"))
    }

    /// The Episode `asked` names, unset if the source has none.
    ///
    /// # Errors
    ///
    /// When the source couldn't be asked.
    fn episode(_asked: EpisodeAsked) -> Result<EpisodeAnswer, Error> {
        Err(not_served("episode"))
    }

    /// The image at the path `asked` names, as the source gave it in a poster, backdrop, still or
    /// artwork, unset if it has none there.
    ///
    /// # Errors
    ///
    /// When the source couldn't be asked.
    fn image(_asked: ImageAsked) -> Result<ImageAnswer, Error> {
        Err(not_served("image"))
    }

    /// The artwork the source has for the title `asked` names, of every kind it has; none if it
    /// has none.
    ///
    /// # Errors
    ///
    /// When the source couldn't be asked.
    fn artwork(_asked: ArtworkAsked) -> Result<ArtworkAnswer, Error> {
        Err(not_served("artwork"))
    }

    /// The artwork the source has for the Season `asked` names, of every kind it has; none if it
    /// has none.
    ///
    /// # Errors
    ///
    /// When the source couldn't be asked.
    fn season_artwork(_asked: SeasonAsked) -> Result<ArtworkAnswer, Error> {
        Err(not_served("season artwork"))
    }
}

/// What a [`TitleSource`] method the Plugin doesn't override returns, should [`title_source!`]
/// list it anyway.
fn not_served(method: &str) -> Error {
    Error::msg(format!("this title source doesn't serve {method}"))
}

/// Exports the [`TitleSource`] methods a Plugin serves, each under its export name: `search` as
/// `title_search`, and `movie`, `show`, `season`, `episode`, `image`, `artwork` and
/// `season_artwork` likewise,
/// taking and returning their `nama.plugin.v1` messages:
/// `title_source!(Tvdb, [search, show, season, image])`.
///
/// The Server treats a missing export as a method the source doesn't serve, so only the methods
/// listed are exported. A macro can't see which of the trait's defaults a type overrides, so the
/// list is explicit.
#[macro_export]
macro_rules! title_source {
    ($source:ty, [$($method:ident),+ $(,)?]) => {
        $($crate::title_source!(@export $source, $method);)+
    };
    (@export $source:ty, search) => {
        const _: () = {
            // `plugin_fn` expands to paths starting with `extism_pdk`.
            use $crate::{
                Buffa, FnResult, SearchQuery, SearchResults, TitleSource, extism_pdk, plugin_fn,
            };

            #[plugin_fn]
            pub fn title_search(
                Buffa(query): Buffa<SearchQuery>,
            ) -> FnResult<Buffa<SearchResults>> {
                Ok(Buffa(<$source as TitleSource>::search(query)?))
            }
        };
    };
    (@export $source:ty, movie) => {
        const _: () = {
            // `plugin_fn` expands to paths starting with `extism_pdk`.
            use $crate::{
                Buffa, FnResult, TitleAsked, MovieAnswer, TitleSource, extism_pdk, plugin_fn,
            };

            #[plugin_fn]
            pub fn title_movie(
                Buffa(asked): Buffa<TitleAsked>,
            ) -> FnResult<Buffa<MovieAnswer>> {
                Ok(Buffa(<$source as TitleSource>::movie(asked)?))
            }
        };
    };
    (@export $source:ty, show) => {
        const _: () = {
            // `plugin_fn` expands to paths starting with `extism_pdk`.
            use $crate::{
                Buffa, FnResult, TitleAsked, ShowAnswer, TitleSource, extism_pdk, plugin_fn,
            };

            #[plugin_fn]
            pub fn title_show(
                Buffa(asked): Buffa<TitleAsked>,
            ) -> FnResult<Buffa<ShowAnswer>> {
                Ok(Buffa(<$source as TitleSource>::show(asked)?))
            }
        };
    };
    (@export $source:ty, season) => {
        const _: () = {
            // `plugin_fn` expands to paths starting with `extism_pdk`.
            use $crate::{
                Buffa, FnResult, SeasonAsked, SeasonAnswer, TitleSource, extism_pdk, plugin_fn,
            };

            #[plugin_fn]
            pub fn title_season(
                Buffa(asked): Buffa<SeasonAsked>,
            ) -> FnResult<Buffa<SeasonAnswer>> {
                Ok(Buffa(<$source as TitleSource>::season(asked)?))
            }
        };
    };
    (@export $source:ty, episode) => {
        const _: () = {
            // `plugin_fn` expands to paths starting with `extism_pdk`.
            use $crate::{
                Buffa, FnResult, EpisodeAsked, EpisodeAnswer, TitleSource, extism_pdk, plugin_fn,
            };

            #[plugin_fn]
            pub fn title_episode(
                Buffa(asked): Buffa<EpisodeAsked>,
            ) -> FnResult<Buffa<EpisodeAnswer>> {
                Ok(Buffa(<$source as TitleSource>::episode(asked)?))
            }
        };
    };
    (@export $source:ty, image) => {
        const _: () = {
            // `plugin_fn` expands to paths starting with `extism_pdk`.
            use $crate::{
                Buffa, FnResult, ImageAsked, ImageAnswer, TitleSource, extism_pdk, plugin_fn,
            };

            #[plugin_fn]
            pub fn title_image(
                Buffa(asked): Buffa<ImageAsked>,
            ) -> FnResult<Buffa<ImageAnswer>> {
                Ok(Buffa(<$source as TitleSource>::image(asked)?))
            }
        };
    };
    (@export $source:ty, artwork) => {
        const _: () = {
            // `plugin_fn` expands to paths starting with `extism_pdk`.
            use $crate::{
                ArtworkAnswer, ArtworkAsked, Buffa, FnResult, TitleSource, extism_pdk, plugin_fn,
            };

            #[plugin_fn]
            pub fn title_artwork(
                Buffa(asked): Buffa<ArtworkAsked>,
            ) -> FnResult<Buffa<ArtworkAnswer>> {
                Ok(Buffa(<$source as TitleSource>::artwork(asked)?))
            }
        };
    };
    (@export $source:ty, season_artwork) => {
        const _: () = {
            // `plugin_fn` expands to paths starting with `extism_pdk`.
            use $crate::{
                ArtworkAnswer, Buffa, FnResult, SeasonAsked, TitleSource, extism_pdk, plugin_fn,
            };

            #[plugin_fn]
            pub fn title_season_artwork(
                Buffa(asked): Buffa<SeasonAsked>,
            ) -> FnResult<Buffa<ArtworkAnswer>> {
                Ok(Buffa(<$source as TitleSource>::season_artwork(asked)?))
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

/// The Library ids the `SETTING_KIND_LIBRARIES` setting `key` holds, in the order the admin gave
/// them; none if it isn't set. A Library deleted since may be among them: skip an id the Kernel no
/// longer lists.
///
/// # Errors
///
/// When the config can't be read, or the setting holds something other than ids, which the Kernel
/// never writes.
pub fn setting_library_ids(key: &str) -> Result<Vec<i64>, Error> {
    library_ids(key, &setting(key)?.unwrap_or_default())
}

/// `text`, the setting `key`'s config, as Library ids: each in decimal, joined by `,`.
fn library_ids(key: &str, text: &str) -> Result<Vec<i64>, Error> {
    text.split(',')
        .filter(|id| !id.is_empty())
        .map(|id| {
            id.parse().map_err(|_not_a_number| {
                Error::msg(format!("the setting {key} holds {id:?}, not Library ids"))
            })
        })
        .collect()
}

/// The setting `key`, as text, or `None` if it isn't set; what it holds when is in [What the
/// Kernel gives a Plugin](https://github.com/electather/nama-rust/blob/main/crates/nama-plugin-proto/README.md#what-the-kernel-gives-a-plugin).
/// Off wasm32 it is the setting the [`testing::Kernel`] holds.
///
/// # Errors
///
/// When the Server's config can't be read.
pub fn setting(key: &str) -> Result<Option<String>, Error> {
    #[cfg(target_arch = "wasm32")]
    return ::extism_pdk::config::get(key);
    #[cfg(not(target_arch = "wasm32"))]
    Ok(testing::setting(key))
}

/// Sends `request` with `body`. If the Server can't send it, such as when nothing answers at its
/// host, the call fails with `unreachable` as its error, in place of the HTTP client's own words.
/// Off wasm32 the [`testing::Kernel`] answers it, and `unreachable` comes back to the Plugin
/// rather than ending the call, so a Plugin passes it up with `?` to behave the same in both.
///
/// # Errors
///
/// When `request` or `body` can't be passed to the Server.
pub fn http_request<'a, T: ToBytes<'a>>(
    request: &HttpRequest,
    body: Option<T>,
    unreachable: &str,
) -> Result<Response, Error> {
    let body = body.map(|body| body.to_bytes()).transpose()?;
    let body = body.as_ref().map(AsRef::as_ref);
    #[cfg(target_arch = "wasm32")]
    {
        // The Server ends the call at once when it can't send a request, so the Plugin never
        // sees that error: the call fails with the one set beforehand.
        set_error(unreachable);
        let response = ::extism_pdk::http::request(request, body);
        clear_error();
        response.map(Response::from)
    }
    #[cfg(not(target_arch = "wasm32"))]
    testing::http_request(request, body.unwrap_or_default())
        .map_err(|_handlers_error| Error::msg(unreachable.to_owned()))
}

/// An HTTP response a Plugin was answered: its status, headers and body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Response {
    /// A response with `status` and `body`, as a test answers a Plugin's request.
    #[must_use]
    pub fn new(status: u16, body: impl Into<Vec<u8>>) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: body.into(),
        }
    }

    /// The response, with the header `name` holding `value`, as a test answers a Plugin's request.
    #[must_use]
    pub fn with_header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_owned(), value.to_owned()));
        self
    }

    /// The value of its header `name`, such as `Retry-After`, matched whatever its case.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        (self.headers.iter())
            .find(|(held, _value)| held.eq_ignore_ascii_case(name))
            .map(|(_name, value)| value.as_str())
    }

    /// Its HTTP status code, such as 200.
    #[must_use]
    pub fn status_code(&self) -> u16 {
        self.status
    }

    /// Its body.
    #[must_use]
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    /// Its body, taken out so a large one, such as an image, isn't copied.
    #[must_use]
    pub fn into_body(self) -> Vec<u8> {
        self.body
    }

    /// Its body, read as JSON.
    ///
    /// # Errors
    ///
    /// When the body isn't JSON of a `T`.
    pub fn json<T>(&self) -> Result<T, Error>
    where
        ::extism_pdk::Json<T>: FromBytesOwned,
    {
        ::extism_pdk::Json::<T>::from_bytes_owned(&self.body).map(|::extism_pdk::Json(json)| json)
    }
}

#[cfg(target_arch = "wasm32")]
impl From<::extism_pdk::HttpResponse> for Response {
    fn from(response: ::extism_pdk::HttpResponse) -> Self {
        let status = response.status_code();
        let headers = (response.headers().iter())
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect();
        // Copied out and freed, so a large body isn't held twice for the rest of the call.
        let memory = response.into_memory();
        let body = memory.to_vec();
        memory.free();
        Self {
            status,
            headers,
            body,
        }
    }
}

/// The time by the Server's clock, since `SystemTime::now()` panics on wasm32-unknown-unknown.
/// Off wasm32, as in a Plugin's unit tests, it is `SystemTime::now()`.
///
/// ```
/// let elapsed = nama_pdk::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
/// assert!(elapsed.as_secs() > 0);
/// ```
#[must_use]
pub fn now() -> std::time::SystemTime {
    #[cfg(target_arch = "wasm32")]
    {
        #[expect(unsafe_code, reason = "the Server's clock is a Wasm import")]
        #[link(wasm_import_module = "nama")]
        unsafe extern "C" {
            #[link_name = "now"]
            safe fn nama_now() -> i64;
        }
        // Milliseconds since the Unix epoch; the Server's clock is never before it.
        let millis = u64::try_from(nama_now()).unwrap_or_default();
        std::time::UNIX_EPOCH + Duration::from_millis(millis)
    }
    #[cfg(not(target_arch = "wasm32"))]
    std::time::SystemTime::now()
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

/// Undoes [`set_error`], so the call in progress succeeds unless it returns an `Err`.
#[cfg(target_arch = "wasm32")]
fn clear_error() {
    #[expect(unsafe_code, reason = "Extism has no safe way to clear the error")]
    // SAFETY: Extism reads an error offset of 0 as no error.
    unsafe {
        extism_pdk::extism::error_set(0);
    }
}

#[cfg(test)]
mod tests {
    use crate::{SearchQuery, SearchResults, TitleSource, extism_pdk::Error, library_ids};

    #[test]
    fn library_ids_are_read_from_their_config_text() {
        assert_eq!(library_ids("keep", "3,1,12").ok(), Some(vec![3, 1, 12]));
        assert_eq!(library_ids("keep", "").ok(), Some(Vec::new()));
        assert_eq!(
            library_ids("keep", "3,Movies").map_err(|err| err.to_string()),
            Err("the setting keep holds \"Movies\", not Library ids".to_owned())
        );
    }

    struct Searcher;

    impl TitleSource for Searcher {
        fn search(_query: SearchQuery) -> Result<SearchResults, Error> {
            Ok(SearchResults::default())
        }
    }

    struct EverythingElse;

    impl TitleSource for EverythingElse {}

    // Each export is a `no_mangle` symbol, so these two link only if `title_source!` exports
    // nothing but the methods it lists: an unlisted `title_search` would be defined twice.
    crate::title_source!(Searcher, [search]);
    crate::title_source!(
        EverythingElse,
        [movie, show, season, episode, image, artwork]
    );
}
