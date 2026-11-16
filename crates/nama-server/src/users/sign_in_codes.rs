//! Sign-in Codes: a new Device shows one, a User signed in on another Device approves it, and the
//! new Device polls with its handle until it is signed in as that User.
//!
//! Pending sign-ins live in memory only, never in the database, so a restart cancels them. Every
//! call takes the time it happens at, so tests can move the clock.

use std::{
    collections::HashMap,
    sync::{Mutex, MutexGuard, PoisonError},
    time::{Duration, Instant},
};

use super::{
    ApprovingDevice,
    codes::{code_in_groups, normalized_code, random_code, random_secret},
};

/// How long a Sign-in Code and its poll handle last.
const LIFETIME: Duration = Duration::from_mins(10);

/// How many characters a Sign-in Code has: 40 random bits.
const CODE_LENGTH: usize = 8;

/// The most sign-ins kept at once, expired ones included, so anonymous starts can't fill the
/// Server's memory: a few tens of megabytes. The limit on starts per client IP (`rate_limits.rs`)
/// is the first defence against a flood; this cap is only the last.
const MAX_PENDING: usize = 100_000;

#[derive(Default)]
pub(super) struct SignInCodes {
    /// By poll handle.
    pending: Mutex<HashMap<String, Pending>>,
}

struct Pending {
    /// As [`normalized_code`] makes it.
    code: String,
    device_name: String,
    delivery: Delivery,
    expires_at: Instant,
    approved_by: Option<ApprovingDevice>,
}

pub(crate) struct Started {
    /// The Sign-in Code to show, as `XXXX-XXXX`.
    pub(crate) code: String,
    pub(crate) handle: String,
}

/// How the Device that started a Sign-in Code asked to be handed its session. Kept with the code
/// and given back when it signs in; only the API module acts on it (ADR 0015).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Delivery {
    /// In a cookie.
    Cookie,
    /// In the response body.
    Token,
}

#[derive(Debug)]
pub(super) enum Poll {
    Waiting,
    /// Not approved, or not polled once approved, within [`LIFETIME`].
    Expired,
    /// Approved from `approving`, so the Device `device_name` signs in as its User, given its
    /// session by `delivery`. Answered once: the sign-in is forgotten.
    Approved {
        approving: ApprovingDevice,
        device_name: String,
        delivery: Delivery,
    },
}

/// Why a Sign-in Code couldn't be started, looked up, approved or polled.
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    /// No unexpired code matches. An unknown code and an expired one are refused alike.
    #[error("no unexpired Sign-in Code matches")]
    NotFound,
    /// The code was approved already, so it can't be again.
    #[error("that Sign-in Code was already approved")]
    AlreadyApproved,
    /// No sign-in is waiting on the poll handle: it was never given out, already answered
    /// [`Poll::Approved`], or forgotten.
    #[error("no sign-in is waiting on that handle")]
    HandleNotFound,
    /// [`MAX_PENDING`] sign-ins are pending.
    #[error("too many sign-ins are waiting, try again later")]
    TooManyPending,
    /// The operating system's random number generator failed.
    #[error("could not generate a Sign-in Code: {0}")]
    Random(#[from] getrandom::Error),
}

impl SignInCodes {
    /// Starts signing in the Device `device_name`, which has been checked, at `now`.
    pub(super) fn start(
        &self,
        device_name: String,
        delivery: Delivery,
        now: Instant,
    ) -> Result<Started, Error> {
        let handle = random_secret()?;
        let mut pending = self.lock();
        // Kept a lifetime past expiry, so a Device polling late still hears it expired.
        pending.retain(|_, sign_in| now < sign_in.expires_at + LIFETIME);
        if pending.len() >= MAX_PENDING {
            return Err(Error::TooManyPending);
        }
        let code = loop {
            let code = random_code(CODE_LENGTH)?;
            if pending.values().all(|sign_in| sign_in.code != code) {
                break code;
            }
        };
        let started = Started {
            code: code_in_groups(&code),
            handle: handle.clone(),
        };
        pending.insert(
            handle,
            Pending {
                code,
                device_name,
                delivery,
                expires_at: now + LIFETIME,
                approved_by: None,
            },
        );
        Ok(started)
    }

    /// The name of the Device that the code `entered` would sign in, at `now`. Approves nothing.
    pub(super) fn look_up(&self, entered: &str, now: Instant) -> Result<String, Error> {
        let mut pending = self.lock();
        Ok(awaiting_approval(&mut pending, entered, now)?
            .device_name
            .clone())
    }

    /// Approves the code `entered` from `approving` at `now`, so its Device signs in as their User.
    pub(super) fn approve(
        &self,
        entered: &str,
        approving: ApprovingDevice,
        now: Instant,
    ) -> Result<(), Error> {
        let mut pending = self.lock();
        awaiting_approval(&mut pending, entered, now)?.approved_by = Some(approving);
        Ok(())
    }

    /// Where the sign-in polled with `handle` has got to at `now`.
    pub(super) fn poll(&self, handle: &str, now: Instant) -> Result<Poll, Error> {
        let mut pending = self.lock();
        let sign_in = pending.get(handle).ok_or(Error::HandleNotFound)?;
        if now >= sign_in.expires_at {
            return Ok(Poll::Expired);
        }
        if sign_in.approved_by.is_none() {
            return Ok(Poll::Waiting);
        }
        let Some(Pending {
            approved_by: Some(approving),
            device_name,
            delivery,
            ..
        }) = pending.remove(handle)
        else {
            return Err(Error::HandleNotFound);
        };
        Ok(Poll::Approved {
            approving,
            device_name,
            delivery,
        })
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<String, Pending>> {
        self.pending.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The sign-in in `pending` whose code is `entered`, if it is unexpired at `now` and unapproved.
fn awaiting_approval<'a>(
    pending: &'a mut HashMap<String, Pending>,
    entered: &str,
    now: Instant,
) -> Result<&'a mut Pending, Error> {
    let code = normalized_code(entered);
    let sign_in = pending
        .values_mut()
        .find(|sign_in| sign_in.code == code && now < sign_in.expires_at)
        .ok_or(Error::NotFound)?;
    if sign_in.approved_by.is_some() {
        return Err(Error::AlreadyApproved);
    }
    Ok(sign_in)
}

#[cfg(test)]
#[path = "sign_in_codes_tests.rs"]
mod tests;
