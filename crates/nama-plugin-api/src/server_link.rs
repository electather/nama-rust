//! Provide: Linking the Server to an outside service through a code an Admin enters there, such as
//! the Hub (ADR 0011, ADR 0094), and then Linking its Users the same way. The Plugin keeps whatever
//! identity the service gives the Server. Its Users then sign in with their accounts there, and an
//! Admin creates Users from such accounts (ADR 0092). Only a Built-in Plugin provides it, by
//! registering it:
//!
//! ```ignore
//! registrar.provide::<dyn ServerLinker>(Priority::DEFAULT, Arc::new(my_linker));
//! ```

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
        /// Whether the service reached the Server from outside its LAN, once it tried. Always
        /// `None` for a User.
        reachability: Option<Reachability>,
    },
}

/// Where the service tried to reach the Server from outside its LAN, and whether it could.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reachability {
    /// The Server's public IP address with its [`ServerAddresses::public_port`], such as
    /// `http://203.0.113.7:7980`.
    pub address: String,
    /// Whether the service reached it there, so clients outside the LAN try it.
    pub reachable: bool,
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
}

/// A Provider that Links the Server, and its Users, to an outside service.
pub trait ServerLinker: Send + Sync + 'static {
    /// Where Linking stands. While a code waits, this is also when the Plugin asks the service
    /// whether it was entered, so the Kernel asks only while an Admin is looking.
    ///
    /// # Errors
    ///
    /// When the service couldn't be asked while a code waits.
    fn link(&self) -> BoxFuture<'_, Result<Link, PluginError>>;

    /// Starts Linking the Server, which the service lists as `server_name`, replacing any code
    /// waiting, and answers the new code.
    ///
    /// # Errors
    ///
    /// When the service couldn't be asked.
    fn start_link<'a>(&'a self, server_name: &'a str) -> BoxFuture<'a, Result<Link, PluginError>>;

    /// Unlinks the Server, telling the service so it revokes the Server's identity there, or
    /// forgets a waiting code. Unlinking a Server that isn't Linked does nothing.
    ///
    /// # Errors
    ///
    /// When the service couldn't be told: the Server stays Linked.
    fn unlink(&self) -> BoxFuture<'_, Result<(), PluginError>>;

    /// Where Linking the User `user_id` stands, or `None` while the Server isn't Linked, when its
    /// Users can't Link. As with [`ServerLinker::link`], this is when the Plugin asks the service.
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
    /// account approved it there: the User Linked to it. `None` while it isn't approved, or once
    /// it expired or was used.
    ///
    /// # Errors
    ///
    /// When the Server isn't Linked, the service couldn't be asked, or what it answered doesn't
    /// check out.
    fn finish_account_sign_in<'a>(
        &'a self,
        handle: &'a str,
    ) -> BoxFuture<'a, Result<Option<i64>, PluginError>>;
    /// Tells the Plugin the Server's `addresses`, for the service to hand its clients. The Kernel
    /// tells it every minute whether or not they changed, so the Plugin passes on only a change,
    /// or the same now and then for the service to check its public port again, and only while
    /// the Server is Linked. [`ServerLinker::link`] then answers what the service found.
    ///
    /// # Errors
    ///
    /// When the service couldn't be told: the Kernel tells the Plugin again a minute later.
    fn set_addresses(&self, addresses: ServerAddresses) -> BoxFuture<'_, Result<(), PluginError>>;
}
