//! The Link types the Kernel answers in, as it Links the Server and its Users with the Hub and
//! syncs them (ADR 0011, ADR 0105).

use std::net::SocketAddr;

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

/// A title on a User's Watchlist, or taken off it, as the change made last left it, as the Kernel
/// syncs it with the Hub (ADR 0056).
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
