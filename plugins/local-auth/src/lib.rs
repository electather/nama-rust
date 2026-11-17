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
    Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    auth::{Authenticator, PasswordError},
    settings::SettingsSchema,
};

const PASSWORD_LENGTH: RangeInclusive<usize> = 8..=1024;

/// The password behind the credential [`Authenticator::verify_unknown_user`] checks against.
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
    fn manifest(&self) -> Manifest {
        Manifest {
            id: Self::ID,
            name: "Passwords".to_owned(),
            description: "Lets Users sign in with a password.".to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            settings: SettingsSchema::default(),
            personal_settings: SettingsSchema::default(),
            extension_points: vec![PointKey::provide::<dyn Authenticator>()],
            logo_svg: Some(include_str!("../logo.svg").to_owned()),
            permissions: Vec::new(),
            tick_every: None,
            title_source: None,
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        let provider: Arc<dyn Authenticator> = Arc::new(Argon2id::new()?);
        registrar.provide::<dyn Authenticator>(Priority::DEFAULT, provider);
        Ok(())
    }
}

struct Argon2id {
    /// What [`Authenticator::verify_unknown_user`] checks against: [`DUMMY_PASSWORD`] hashed by
    /// [`hash`], like every new credential, so checking against it costs the same as checking a
    /// real User's.
    dummy_credential: String,
}

impl Argon2id {
    /// The Authenticator, with its credential for unknown Users freshly made.
    fn new() -> Result<Self, PluginError> {
        let dummy_credential = hash(DUMMY_PASSWORD)
            .map_err(|err| format!("could not make the credential for unknown Users: {err}"))?;
        Ok(Self { dummy_credential })
    }
}

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

    #[test]
    fn the_credential_made_at_registration_verifies_against_the_dummy_password() {
        let authenticator = Argon2id::new().unwrap();

        assert!(
            authenticator
                .verify_password(DUMMY_PASSWORD, &authenticator.dummy_credential)
                .unwrap()
        );
    }
}
