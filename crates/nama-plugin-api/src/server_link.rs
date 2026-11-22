//! Provide: Linking the Server to an outside service through a code an Admin enters there, such as
//! the Hub (ADR 0011, ADR 0094), and then Linking its Users the same way. The Plugin keeps whatever
//! identity the service gives the Server. Only a Built-in Plugin provides it, by registering it:
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
    },
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
}
