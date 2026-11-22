//! Hashing and checking Accounts' passwords with argon2id, the `argon2` crate's default parameters
//! (OWASP's recommendation), as the Server's `local-auth` Plugin does. Each runs on a blocking
//! thread: one takes tens of milliseconds of CPU.

use std::ops::RangeInclusive;

use argon2::{
    Argon2,
    password_hash::{self, PasswordHasher, PasswordVerifier, phc::PasswordHash},
};
use tokio::task::{self, JoinError};

use super::Error;

/// How many characters a password may have. The top bounds the work one hash costs.
pub(crate) const PASSWORD_LENGTH: RangeInclusive<usize> = 8..=1024;

/// A hash, of "a password no Account has", that signing in with an unknown email checks against,
/// so that it takes as long as signing in with a wrong password.
const UNKNOWN_EMAIL_HASH: &str = "$argon2id$v=19$m=19456,t=2,p=1$m0VZ6FN5YSmKzZ9Lyjpmjg$w3ykI1C+6RY3wAc4+ixCRYRHFitBlyPbAu6RiK77XQw";

/// Why a password couldn't be hashed or checked.
#[derive(Debug, thiserror::Error)]
pub(crate) enum PasswordError {
    #[error(transparent)]
    Hash(#[from] password_hash::Error),
    #[error("a stored hash isn't a PHC string: {0}")]
    Stored(#[from] password_hash::phc::Error),
    #[error("the hashing thread failed: {0}")]
    Thread(#[from] JoinError),
}

/// `password` hashed with a fresh random salt, as a PHC string, if its length is allowed.
pub(super) async fn hash(password: String) -> Result<String, Error> {
    if !PASSWORD_LENGTH.contains(&password.chars().count()) {
        return Err(Error::PasswordLength);
    }
    let hashed = task::spawn_blocking(move || {
        Argon2::default()
            .hash_password(password.as_bytes())
            .map(|hash| hash.to_string())
    })
    .await
    .map_err(PasswordError::from)?;
    Ok(hashed.map_err(PasswordError::from)?)
}

/// Whether `password` is the one `hash` was made from.
pub(super) async fn verify(password: String, hash: String) -> Result<bool, PasswordError> {
    task::spawn_blocking(move || {
        match Argon2::default().verify_password(password.as_bytes(), &PasswordHash::new(&hash)?) {
            Ok(()) => Ok(true),
            Err(password_hash::Error::PasswordInvalid) => Ok(false),
            Err(err) => Err(err.into()),
        }
    })
    .await?
}

/// Spends as long as [`verify`] would, for an email no Account has.
pub(super) async fn verify_unknown(password: String) {
    // Only the time spent matters; the answer is always "no such Account".
    let _checked: Result<bool, PasswordError> =
        verify(password, UNKNOWN_EMAIL_HASH.to_owned()).await;
}

#[cfg(test)]
#[path = "passwords_tests.rs"]
mod tests;
