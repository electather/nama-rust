//! Provide: Linking the Server to an outside service through a code an Admin enters there, such as
//! the Hub (ADR 0011, ADR 0094). The Plugin keeps whatever identity the service gives the Server.
//! Only a Built-in Plugin provides it, by registering it:
//!
//! ```ignore
//! registrar.provide::<dyn ServerLinker>(Priority::DEFAULT, Arc::new(my_linker));
//! ```

use crate::{BoxFuture, PluginError};

/// Where Linking the Server stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerLink {
    /// Not Linked, and no code waiting.
    NotLinked,
    /// Waiting for an Admin to enter `code` at `url`.
    Waiting {
        /// What the Admin enters, such as `BDFH-JKLM`.
        code: String,
        /// Where they enter it.
        url: String,
    },
    /// Linked to the service's `account`, such as its email.
    Linked {
        /// Who the Server belongs to at the service, as it names them.
        account: String,
    },
}

/// A Provider that Links the Server to an outside service.
pub trait ServerLinker: Send + Sync + 'static {
    /// Where Linking stands. While a code waits, this is also when the Plugin asks the service
    /// whether it was entered, so the Kernel asks only while an Admin is looking.
    ///
    /// # Errors
    ///
    /// When the service couldn't be asked while a code waits.
    fn link(&self) -> BoxFuture<'_, Result<ServerLink, PluginError>>;

    /// Starts Linking the Server, which the service lists as `server_name`, replacing any code
    /// waiting, and answers the new code.
    ///
    /// # Errors
    ///
    /// When the service couldn't be asked.
    fn start_link<'a>(
        &'a self,
        server_name: &'a str,
    ) -> BoxFuture<'a, Result<ServerLink, PluginError>>;

    /// Unlinks the Server, telling the service so it revokes the Server's identity there, or
    /// forgets a waiting code. Unlinking a Server that isn't Linked does nothing.
    ///
    /// # Errors
    ///
    /// When the service couldn't be told: the Server stays Linked.
    fn unlink(&self) -> BoxFuture<'_, Result<(), PluginError>>;
}
