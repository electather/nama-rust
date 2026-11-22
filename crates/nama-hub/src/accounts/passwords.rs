//! Hashing and checking Accounts' passwords with argon2id, the `argon2` crate's default parameters
//! (OWASP's recommendation), as the Server's `local-auth` Plugin does. Each runs on a blocking
//! thread: one takes tens of milliseconds of CPU and 19 MiB of memory.

use std::{num::NonZero, ops::RangeInclusive, sync::LazyLock, thread};

use argon2::{
    Argon2,
    password_hash::{self, PasswordHasher, PasswordVerifier, phc::PasswordHash},
};
use tokio::{
    sync::{AcquireError, Semaphore},
    task::{self, JoinError},
};

/// How many characters a password may have. The top bounds the work one hash costs.
pub(crate) const PASSWORD_LENGTH: RangeInclusive<usize> = 8..=1024;

/// A hash, of "a password no Account has", that signing in with an unknown email checks against,
/// so that it takes as long as signing in with a wrong password.
const UNKNOWN_EMAIL_HASH: &str = "$argon2id$v=19$m=19456,t=2,p=1$m0VZ6FN5YSmKzZ9Lyjpmjg$w3ykI1C+6RY3wAc4+ixCRYRHFitBlyPbAu6RiK77XQw";

/// One permit per core: creating an Account isn't rate limited, so without a cap a flood of them
/// would fill tokio's blocking pool, at 19 MiB a hash. Callers past the cap wait their turn.
static HASHING: LazyLock<Semaphore> =
    LazyLock::new(|| Semaphore::new(thread::available_parallelism().map_or(1, NonZero::get)));

/// Why a password couldn't be hashed or checked.
#[derive(Debug, thiserror::Error)]
pub(crate) enum PasswordError {
    #[error(transparent)]
    Hash(#[from] password_hash::Error),
    #[error("a stored hash isn't a PHC string: {0}")]
    Stored(#[from] password_hash::phc::Error),
    #[error("the hashing thread failed: {0}")]
    Thread(#[from] JoinError),
    #[error("the hashing permits closed: {0}")]
    Permits(#[from] AcquireError),
}

/// `password` hashed with a fresh random salt, as a PHC string.
pub(super) async fn hash(password: String) -> Result<String, PasswordError> {
    on_hashing_thread(move || {
        Ok(Argon2::default()
            .hash_password(password.as_bytes())?
            .to_string())
    })
    .await
}

/// Whether `password` is the one `hash` was made from.
pub(super) async fn verify(password: String, hash: String) -> Result<bool, PasswordError> {
    on_hashing_thread(move || {
        match Argon2::default().verify_password(password.as_bytes(), &PasswordHash::new(&hash)?) {
            Ok(()) => Ok(true),
            Err(password_hash::Error::PasswordInvalid) => Ok(false),
            Err(err) => Err(err.into()),
        }
    })
    .await
}

/// Runs `work` on a blocking thread once [`HASHING`] has a permit for it.
async fn on_hashing_thread<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, PasswordError> + Send + 'static,
) -> Result<T, PasswordError> {
    let permit = HASHING.acquire().await?;
    // The thread holds the permit, not this future: a client hanging up drops the future, but the
    // hash runs on.
    task::spawn_blocking(move || {
        let _permit = permit;
        work()
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
