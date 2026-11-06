//! A Built-in Plugin that checks Users' passwords, the way a Server signs people in without a Hub.
//!
//! It provides an [`Authenticator`] that hashes passwords with argon2id, using the `argon2`
//! crate's default parameters (OWASP's recommendation), so the credential string the Kernel stores
//! is a PHC string such as `$argon2id$v=19$m=19456,t=2,p=1$...`. It accepts passwords of 8 to 1024
//! characters, with no other rules.

use std::{ops::RangeInclusive, sync::Arc};

use argon2::{
    Argon2,
    password_hash::{self, PasswordHasher, PasswordVerifier, phc::PasswordHash},
};
use nama_plugin_api::{
    Plugin, PluginError, PluginId, Priority, Registrar,
    auth::{Authenticator, PasswordError},
};
use tracing::error;

/// How many characters a password may have.
const PASSWORD_LENGTH: RangeInclusive<usize> = 8..=1024;

/// What [`Authenticator::verify_unknown_user`] checks, against a credential made from it.
const DUMMY_PASSWORD: &str = "a password no User has";

/// The `local-auth` Built-in Plugin: the Server registers it so that Users can sign in with a
/// password.
#[derive(Debug, Default)]
pub struct LocalAuth;

impl LocalAuth {
    /// The Plugin's id.
    pub const ID: PluginId = PluginId::from_static("local-auth");
}

impl Plugin for LocalAuth {
    fn id(&self) -> PluginId {
        Self::ID
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn register(&self, registrar: &mut Registrar) {
        match Argon2id::new() {
            Ok(argon2id) => {
                let provider: Arc<dyn Authenticator> = Arc::new(argon2id);
                registrar.provide::<dyn Authenticator>(Priority::DEFAULT, provider);
            }
            Err(err) => error!("could not set up password hashing: {err}"),
        }
    }
}

/// The [`Authenticator`] `local-auth` provides.
struct Argon2id {
    /// Made from [`DUMMY_PASSWORD`] with the same parameters as every new credential, so checking
    /// against it costs the same as checking a real User's.
    dummy_credential: String,
}

impl Argon2id {
    fn new() -> Result<Self, password_hash::Error> {
        Ok(Self {
            dummy_credential: hash(DUMMY_PASSWORD)?,
        })
    }
}

impl Authenticator for Argon2id {
    fn hash_password(&self, password: &str) -> Result<String, PasswordError> {
        let length = password.chars().count();
        if length < *PASSWORD_LENGTH.start() {
            return Err(PasswordError::Rejected(format!(
                "a password needs at least {} characters",
                PASSWORD_LENGTH.start()
            )));
        }
        if length > *PASSWORD_LENGTH.end() {
            return Err(PasswordError::Rejected(format!(
                "a password may have at most {} characters",
                PASSWORD_LENGTH.end()
            )));
        }
        hash(password).map_err(|err| PasswordError::Failed(err.into()))
    }

    fn verify_password(&self, password: &str, credential: &str) -> Result<bool, PluginError> {
        let credential = PasswordHash::new(credential)?;
        match Argon2::default().verify_password(password.as_bytes(), &credential) {
            Ok(()) => Ok(true),
            Err(password_hash::Error::PasswordInvalid) => Ok(false),
            Err(err) => Err(err.into()),
        }
    }

    fn verify_unknown_user(&self, password: &str) {
        // Only the time spent matters; the answer is always "no such User".
        let _checked: Result<bool, PluginError> =
            self.verify_password(password, &self.dummy_credential);
    }
}

/// Hashes `password` with argon2id and a fresh random salt, as a PHC string.
fn hash(password: &str) -> Result<String, password_hash::Error> {
    Ok(Argon2::default()
        .hash_password(password.as_bytes())?
        .to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Timing can't be observed reliably in a test, so check what decides it: the unknown-User
    /// check verifies against a real argon2id credential made with the default parameters.
    #[test]
    fn the_unknown_user_check_verifies_against_a_real_argon2id_credential() {
        let argon2id = Argon2id::new().unwrap();

        assert!(
            argon2id
                .verify_password(DUMMY_PASSWORD, &argon2id.dummy_credential)
                .unwrap()
        );
        assert!(
            argon2id
                .dummy_credential
                .starts_with("$argon2id$v=19$m=19456,t=2,p=1$")
        );
    }
}
