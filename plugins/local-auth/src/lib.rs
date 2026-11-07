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

const PASSWORD_LENGTH: RangeInclusive<usize> = 8..=1024;

/// What [`Authenticator::verify_unknown_user`] checks against: a password no User has, hashed with
/// the same parameters as every new credential, so checking against it costs the same as checking a
/// real User's. It is a constant so that setting up `local-auth` can't fail.
const DUMMY_CREDENTIAL: &str = "$argon2id$v=19$m=19456,t=2,p=1$gVFAYI9SXil+7t4cbyaaww$cT+AzDWaNGwSRE/BPXjRgIy41T5KNjs7p69V5uq36BU";

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
        let provider: Arc<dyn Authenticator> = Arc::new(Argon2id);
        registrar.provide::<dyn Authenticator>(Priority::DEFAULT, provider);
    }
}

struct Argon2id;

impl Authenticator for Argon2id {
    fn hash_password(&self, password: &str) -> Result<String, PasswordError> {
        if !PASSWORD_LENGTH.contains(&password.chars().count()) {
            return Err(PasswordError::Rejected(format!(
                "a password needs {} to {} characters",
                PASSWORD_LENGTH.start(),
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
        let _checked: Result<bool, PluginError> = self.verify_password(password, DUMMY_CREDENTIAL);
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

    /// The password [`DUMMY_CREDENTIAL`] was made from.
    const DUMMY_PASSWORD: &str = "a password no User has";

    /// The unknown-User check costs what a real one does only while [`DUMMY_CREDENTIAL`] is a real
    /// argon2id credential with the parameters new credentials get, so a change to the `argon2`
    /// crate's defaults fails here.
    #[test]
    fn the_dummy_credential_is_a_real_credential_with_the_parameters_of_a_new_one() {
        let new = hash(DUMMY_PASSWORD).unwrap();
        let parameters = |credential: &str| credential.rsplitn(3, '$').nth(2).map(str::to_owned);

        assert_eq!(parameters(DUMMY_CREDENTIAL), parameters(&new));
        assert!(
            Argon2id
                .verify_password(DUMMY_PASSWORD, DUMMY_CREDENTIAL)
                .unwrap()
        );
    }
}
