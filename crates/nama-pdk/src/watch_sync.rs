//! Watch sync: follows each User's Watch State to an outside tracker and brings the changes made
//! there back.

use extism_pdk::Error;
pub use nama_plugin_proto::nama::plugin::v1::{
    ImportAsked, Imported, LinkPolled, LinkStarted, PollLinkAsked, ScrobbleAsked, Scrobbled,
    StartLinkAsked, SyncedWatchState,
};

/// Watch sync. Its Manifest's `points` name [`services::WATCH_SYNC`](crate::services::WATCH_SYNC),
/// its SECRET `personal_settings` hold each User's tokens, and [`watch_sync!`](crate::watch_sync!)
/// exports it.
///
/// A User links their account by device code: `start_link` gives the code they enter, and
/// `poll_link` the tokens once they did, which the Server keeps as their Personal settings.
/// The Server asks `scrobble` and `import` only for a User who set any of the Personal settings, with theirs: it tells
/// each change to their Watch State as it is made, and asks every 15 minutes what changed on the
/// tracker, which it takes where newer than its own and never tells back.
///
/// ```
/// use nama_pdk::{
///     ImportAsked, Imported, LinkPolled, LinkStarted, PollLinkAsked, ScrobbleAsked, Scrobbled,
///     StartLinkAsked, WatchSync, extism_pdk::Error,
/// };
///
/// struct Nowhere;
///
/// impl WatchSync for Nowhere {
///     fn scrobble(_asked: ScrobbleAsked) -> Result<Scrobbled, Error> {
///         Ok(Scrobbled::default())
///     }
///
///     fn import(asked: ImportAsked) -> Result<Imported, Error> {
///         Ok(Imported {
///             cursor: asked.since,
///             ..Imported::default()
///         })
///     }
///
///     fn start_link(_asked: StartLinkAsked) -> Result<LinkStarted, Error> {
///         Err(Error::msg("Nowhere has no accounts"))
///     }
///
///     fn poll_link(_asked: PollLinkAsked) -> Result<LinkPolled, Error> {
///         Ok(LinkPolled {
///             expired: true,
///             ..LinkPolled::default()
///         })
///     }
/// }
///
/// nama_pdk::watch_sync!(Nowhere);
/// ```
pub trait WatchSync {
    /// Takes one write's changes to the User's Watch State to the tracker. Position updates while
    /// they play come here too, for the Plugin to send or skip.
    ///
    /// # Errors
    ///
    /// When it couldn't: the Server logs it and doesn't ask again.
    fn scrobble(asked: ScrobbleAsked) -> Result<Scrobbled, Error>;

    /// The changes to the User's Watch State on the tracker since `asked.since`, empty on the
    /// first ask, and the cursor the next ask continues from.
    ///
    /// # Errors
    ///
    /// When it couldn't ask: the Server keeps the cursor and asks again next time.
    fn import(asked: ImportAsked) -> Result<Imported, Error>;

    /// Starts linking a User's account at the tracker: the code they enter, where, for how long,
    /// and the device code `poll_link` is asked with.
    ///
    /// # Errors
    ///
    /// When the tracker couldn't be asked for a code.
    fn start_link(asked: StartLinkAsked) -> Result<LinkStarted, Error>;

    /// Empty while the User hasn't entered the code `asked.device_code` came with; the tokens, by
    /// SECRET Personal setting key, once they did; or `expired`.
    ///
    /// # Errors
    ///
    /// When the tracker couldn't be asked.
    fn poll_link(asked: PollLinkAsked) -> Result<LinkPolled, Error>;
}

/// Exports a [`WatchSync`] as the Plugin's `scrobble`, `import`, `start_link` and `poll_link`,
/// taking the `nama.plugin.v1` messages the Server sends and returning their answers.
#[macro_export]
macro_rules! watch_sync {
    ($sync:ty) => {
        const _: () = {
            // `plugin_fn` expands to paths starting with `extism_pdk`.
            use $crate::{
                Buffa, FnResult, ImportAsked, Imported, LinkPolled, LinkStarted, PollLinkAsked,
                ScrobbleAsked, Scrobbled, StartLinkAsked, WatchSync, extism_pdk, plugin_fn,
            };

            #[plugin_fn]
            pub fn scrobble(Buffa(asked): Buffa<ScrobbleAsked>) -> FnResult<Buffa<Scrobbled>> {
                Ok(Buffa(<$sync as WatchSync>::scrobble(asked)?))
            }

            #[plugin_fn]
            pub fn import(Buffa(asked): Buffa<ImportAsked>) -> FnResult<Buffa<Imported>> {
                Ok(Buffa(<$sync as WatchSync>::import(asked)?))
            }

            #[plugin_fn]
            pub fn start_link(Buffa(asked): Buffa<StartLinkAsked>) -> FnResult<Buffa<LinkStarted>> {
                Ok(Buffa(<$sync as WatchSync>::start_link(asked)?))
            }

            #[plugin_fn]
            pub fn poll_link(Buffa(asked): Buffa<PollLinkAsked>) -> FnResult<Buffa<LinkPolled>> {
                Ok(Buffa(<$sync as WatchSync>::poll_link(asked)?))
            }
        };
    };
}
