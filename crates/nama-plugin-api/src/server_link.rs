//! Provide: Linking the Server's Users to an outside service through a code each enters there, once
//! the Kernel has Linked the Server itself with the Hub (ADR 0011, ADR 0105). Its Users then sign in
//! with their accounts there, and an Admin creates Users from such accounts (ADR 0092). Only a
//! Built-in Plugin provides it, by registering it:
//!
//! ```ignore
//! registrar.provide::<dyn ServerLinker>(Priority::DEFAULT, Arc::new(my_linker));
//! ```

use std::net::SocketAddr;

use crate::{BoxFuture, PluginError};

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

/// A Provider that Links the Server's Users to an outside service.
pub trait ServerLinker: Send + Sync + 'static {
    /// Where Linking the User `user_id` stands, or `None` while the Server isn't Linked, when its
    /// Users can't Link. While a code waits, this is also when the Plugin asks the service whether
    /// it was entered, so the Kernel asks only while someone is looking.
    ///
    /// # Errors
    ///
    /// When the Plugin can't answer even from what it last heard.
    fn user_link(&self, user_id: i64) -> BoxFuture<'_, Result<Option<Link>, PluginError>>;

    /// Starts Linking the User `user_id`, whom the service lists as `user_name`, replacing any
    /// code they wait on, and answers the new code, or their Link if they are Linked already.
    ///
    /// # Errors
    ///
    /// When the Server isn't Linked, or the service couldn't be asked.
    fn start_user_link<'a>(
        &'a self,
        user_id: i64,
        user_name: &'a str,
    ) -> BoxFuture<'a, Result<Link, PluginError>>;

    /// Unlinks the User `user_id`, telling the service, or forgets the code they wait on.
    /// Unlinking a User who isn't Linked does nothing.
    ///
    /// # Errors
    ///
    /// When the service couldn't be told: the User stays Linked.
    fn unlink_user(&self, user_id: i64) -> BoxFuture<'_, Result<(), PluginError>>;

    /// Asks the account at the service with `email`, if there is one, to accept the User
    /// `user_id`, whom the service lists as `user_name`; once it does, they are Linked to it. It
    /// says nothing of whether there is one.
    ///
    /// # Errors
    ///
    /// When the Server isn't Linked, or the service couldn't be asked.
    fn invite_user<'a>(
        &'a self,
        user_id: i64,
        user_name: &'a str,
        email: &'a str,
    ) -> BoxFuture<'a, Result<(), PluginError>>;

    /// Starts someone signing in to the Server with their account at the service, which sends
    /// them back to `return_url` once they approve.
    ///
    /// # Errors
    ///
    /// When the Server isn't Linked, or the service couldn't be asked.
    fn start_account_sign_in<'a>(
        &'a self,
        return_url: &'a str,
    ) -> BoxFuture<'a, Result<AccountSignIn, PluginError>>;

    /// The id of the User the sign-in `handle` stands for, as the service vouches, once their
    /// account approved it there: the User Linked to it. `code` is the one-time code the service
    /// sent the person back with, which the service checks. `None` while it isn't approved, once
    /// it expired or was used, or with a wrong code, which ends it.
    ///
    /// # Errors
    ///
    /// When the Server isn't Linked, the service couldn't be asked, or what it answered doesn't
    /// check out.
    fn finish_account_sign_in<'a>(
        &'a self,
        handle: &'a str,
        code: &'a str,
    ) -> BoxFuture<'a, Result<Option<i64>, PluginError>>;
}
