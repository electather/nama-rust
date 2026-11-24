//! Provide: syncing the Users the Kernel Linked to Accounts with an outside service, once the
//! Kernel has Linked the Server and them with the Hub (ADR 0011, ADR 0105): their Watch State and
//! Subtitle and Audio Preferences on their other Servers come back through it (ADR 0008, ADR 0092),
//! and their Watchlist syncs with them (ADR 0056). The Link types the Kernel answers in live here too.
//! Only a Built-in Plugin provides it, by registering it:
//!
//! ```ignore
//! registrar.provide::<dyn ServerLinker>(Priority::DEFAULT, Arc::new(my_linker));
//! ```

use std::net::SocketAddr;

use crate::{
    BoxFuture, PluginError,
    points::{TrackPreference, WatchStateChange},
};

/// Where Linking the Server, or one of its Users, stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Link {
    /// Not Linked, and no code waiting.
    NotLinked,
    /// Waiting for someone to enter `code` at `url`.
    Waiting {
        /// What they enter, such as `BDFH-JKLM`.
        code: String,
        /// Where they enter it.
        url: String,
    },
    /// Linked to the service's `account`, such as its email.
    Linked {
        /// Who the Server or User belongs to at the service, as it names them.
        account: String,
        /// What the service found of how clients reach the Server, once it answered a report of
        /// the Server's addresses. Always `None` for a User.
        reachability: Option<Reachability>,
    },
}

/// How clients reach the Server, as the service found: where it tried to reach the Server from
/// outside its LAN and whether it could, and the names it gave the Server.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reachability {
    /// The Server's public IP address with its [`ServerAddresses::public_port`], such as
    /// `http://203.0.113.7:7980`, or empty without one.
    pub address: String,
    /// Whether the service reached it there, so clients outside the LAN try it.
    pub reachable: bool,
    /// The Server's names, its own first, such as `<id>.namadirect.net`, then one for each of its
    /// LAN IP addresses, such as `192-168-1-10.<id>.namadirect.net` (ADR 0104), and
    /// `relay.<id>.namadirect.net` with a [`ServerAddresses::relay`]. Empty while the service names
    /// no Server.
    pub names: Vec<String>,
}

/// A sign-in with an account at the service, just started. Not `Debug`, so its handle can't end up
/// in a log.
#[expect(
    missing_debug_implementations,
    reason = "it holds a secret handle, which must never be logged"
)]
#[derive(Clone, PartialEq, Eq)]
pub struct AccountSignIn {
    /// The service's page where the person approves it.
    pub url: String,
    /// What finishes it: a secret only the client that started it holds.
    pub handle: String,
}

/// How clients might reach the Server: its Server Addresses (GLOSSARY.md).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ServerAddresses {
    /// The Server URL its Admin set, such as `https://nama.example.com`, or empty for none.
    pub url: String,
    /// Its LAN addresses, such as `http://192.168.1.10:7980`.
    pub lan: Vec<String>,
    /// The port on its router's public IP address that reaches it: one the Server opened with UPnP
    /// or NAT-PMP, or the one its Admin forwarded. `None` without one.
    pub public_port: Option<u16>,
    /// The Relay its Admin runs, which the Server dials out to for clients that can't reach it
    /// directly (ADR 0057). `None` without one.
    pub relay: Option<SocketAddr>,
}

/// A title on a User's Watchlist, or taken off it, as the change made last left it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchlistEntry {
    /// Whether it is a Show rather than a Movie, whose TMDB ids are a separate id space.
    pub show: bool,
    /// The External ID it was added under, such as `tmdb-603`, whether or not the Server has it.
    pub id: String,
    /// Whether it was taken off.
    pub removed: bool,
    /// When it changed, in milliseconds since the Unix epoch.
    pub changed_at_ms: i64,
}

/// A Provider that syncs the Server's Linked Users with an outside service.
pub trait ServerLinker: Send + Sync + 'static {
    /// The Watch State the service keeps of the User `user_id` from every Server they use, such as
    /// their Account's on the Hub (ADR 0008), each title's by any of its ids, with when it last
    /// changed; empty while they aren't Linked. The Kernel takes what is newer than its own.
    ///
    /// # Errors
    ///
    /// When the service couldn't be asked: the Kernel asks again at the next sign-in or scan.
    fn watch_states(
        &self,
        user_id: i64,
    ) -> BoxFuture<'_, Result<Vec<WatchStateChange>, PluginError>>;

    /// The Subtitle and Audio Preferences the service keeps of the User `user_id` from every
    /// Server they use, with when each last changed; empty while they aren't Linked, and from a
    /// service that keeps none. The Kernel takes what is newer than its own.
    ///
    /// # Errors
    ///
    /// When the service couldn't be asked: the Kernel asks again at the next sign-in.
    fn track_preferences(
        &self,
        _user_id: i64,
    ) -> BoxFuture<'_, Result<Vec<TrackPreference>, PluginError>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    /// Hands the service `watchlist`, the User `user_id`'s whole Watchlist, and answers the one it
    /// keeps from every Server they use, such as their Account's on the Hub (ADR 0056), each
    /// title's under each of its ids apart; empty while they aren't Linked. The Kernel takes what
    /// is newer than its own.
    ///
    /// # Errors
    ///
    /// When the service couldn't be asked: the Kernel syncs again at the next change, sign-in or
    /// scan.
    fn sync_watchlist(
        &self,
        user_id: i64,
        watchlist: Vec<WatchlistEntry>,
    ) -> BoxFuture<'_, Result<Vec<WatchlistEntry>, PluginError>>;
}
