//! Provide: checking a User's password (ADR 0014).
//!
//! The Kernel owns Users, Devices and sessions. An [`Authenticator`] only turns a password into a
//! credential string and later checks a password against it. The Kernel stores that string, one
//! per User per Plugin, so the Plugin never sees the database. A Plugin provides it by registering
//! an `Arc<dyn Authenticator>`:
//!
//! ```ignore
//! registrar.provide::<dyn Authenticator>(Priority::DEFAULT, Arc::new(my_authenticator));
//! ```

use std::fmt;

use crate::PluginError;

/// A Provider that checks Users' passwords.
///
/// Checking a password is meant to be slow, so each method may take tens of milliseconds of CPU.
/// Call them where blocking is fine, such as in `tokio::task::spawn_blocking`.
pub trait Authenticator: Send + Sync + 'static {
    /// Turns a User's new password into the credential string the Kernel stores for them. The
    /// string never contains the password.
    ///
    /// # Errors
    ///
    /// [`PasswordError::Rejected`] if the password breaks this Provider's rules, and
    /// [`PasswordError::Failed`] if hashing it failed.
    fn hash_password(&self, password: &str) -> Result<String, PasswordError>;

    /// Whether `password` is the one `credential` was made from by [`Self::hash_password`].
    ///
    /// # Errors
    ///
    /// If `credential` isn't one this Provider made.
    fn verify_password(&self, password: &str, credential: &str) -> Result<bool, PluginError>;

    /// Spends as long as [`Self::verify_password`] would, for a sign-in with a User name that
    /// doesn't exist, so how long sign-in takes doesn't reveal which names do.
    fn verify_unknown_user(&self, password: &str);
}

/// Why [`Authenticator::hash_password`] made no credential.
#[derive(Debug)]
pub enum PasswordError {
    /// The password breaks the Provider's rules, for this reason, which can be shown to the User.
    Rejected(String),
    /// Hashing the password failed.
    Failed(PluginError),
}

impl fmt::Display for PasswordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rejected(reason) => write!(f, "password rejected: {reason}"),
            Self::Failed(error) => write!(f, "could not hash the password: {error}"),
        }
    }
}

impl std::error::Error for PasswordError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Rejected(_) => None,
            Self::Failed(error) => Some(&**error),
        }
    }
}
