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

use crate::users::{self, User};

/// How long a Sign-in Code and its poll handle last.
const LIFETIME: Duration = Duration::from_mins(10);

/// How many characters a Sign-in Code has: 40 random bits.
const CODE_LENGTH: usize = 8;

/// The most sign-ins kept at once, expired ones included, so anonymous starts can't fill the
/// Server's memory: a few tens of megabytes.
// ponytail: one global cap, so a sustained flood of starts blocks everyone's; limit starts per
// client if that happens.
const MAX_PENDING: usize = 100_000;

/// The sign-ins waiting on a Sign-in Code.
#[derive(Default)]
pub(crate) struct SignInCodes {
    /// By poll handle.
    pending: Mutex<HashMap<String, Pending>>,
}

/// One sign-in waiting on a Sign-in Code.
struct Pending {
    /// As [`users::normalized_code`] makes it.
    code: String,
    device_name: String,
    wants_token: bool,
    expires_at: Instant,
    approved_by: Option<User>,
}

/// A sign-in just started.
pub(crate) struct Started {
    /// The Sign-in Code to show, as `XXXX-XXXX`.
    pub(crate) code: String,
    /// What the new Device polls with.
    pub(crate) handle: String,
}

/// Where a sign-in has got to.
#[derive(Debug)]
pub(crate) enum Poll {
    /// Not approved yet.
    Waiting,
    /// Not approved, or not polled once approved, within [`LIFETIME`].
    Expired,
    /// Approved by `user`, so the Device `device_name` signs in as them, given its session as a
    /// token if it `wants_token`. Answered once: the sign-in is forgotten.
    Approved {
        user: User,
        device_name: String,
        wants_token: bool,
    },
}

/// Why a Sign-in Code couldn't be started, looked up or approved.
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    /// No unexpired code matches. An unknown code and an expired one are refused alike.
    #[error("no unexpired Sign-in Code matches")]
    NotFound,
    /// The code was approved already, so it can't be again.
    #[error("that Sign-in Code was already approved")]
    AlreadyApproved,
    /// [`MAX_PENDING`] sign-ins are pending.
    #[error("too many sign-ins are waiting, try again later")]
    TooManyPending,
    /// The operating system's random number generator failed.
    #[error("could not generate a Sign-in Code: {0}")]
    Random(#[from] getrandom::Error),
}

impl SignInCodes {
    /// Starts signing in the Device `device_name`, which has been checked, at `now`.
    pub(crate) fn start(
        &self,
        device_name: String,
        wants_token: bool,
        now: Instant,
    ) -> Result<Started, Error> {
        let handle = users::random_secret()?;
        let mut pending = self.lock();
        // Kept a lifetime past expiry, so a Device polling late still hears it expired.
        pending.retain(|_, sign_in| now < sign_in.expires_at + LIFETIME);
        if pending.len() >= MAX_PENDING {
            return Err(Error::TooManyPending);
        }
        let code = loop {
            let code = users::random_code(CODE_LENGTH)?;
            if pending.values().all(|sign_in| sign_in.code != code) {
                break code;
            }
        };
        let started = Started {
            code: users::code_in_groups(&code),
            handle: handle.clone(),
        };
        pending.insert(
            handle,
            Pending {
                code,
                device_name,
                wants_token,
                expires_at: now + LIFETIME,
                approved_by: None,
            },
        );
        Ok(started)
    }

    /// The name of the Device that the code `entered` would sign in, at `now`. Approves nothing.
    pub(crate) fn look_up(&self, entered: &str, now: Instant) -> Result<String, Error> {
        let mut pending = self.lock();
        Ok(awaiting_approval(&mut pending, entered, now)?
            .device_name
            .clone())
    }

    /// Approves the code `entered` at `now`, so its Device signs in as `user`.
    pub(crate) fn approve(&self, entered: &str, user: &User, now: Instant) -> Result<(), Error> {
        let mut pending = self.lock();
        awaiting_approval(&mut pending, entered, now)?.approved_by = Some(user.clone());
        Ok(())
    }

    /// Where the sign-in polled with `handle` has got to at `now`, or `None` for a handle never
    /// given out, already answered [`Poll::Approved`], or forgotten.
    pub(crate) fn poll(&self, handle: &str, now: Instant) -> Option<Poll> {
        let mut pending = self.lock();
        match pending.get(handle)? {
            sign_in if now >= sign_in.expires_at => Some(Poll::Expired),
            Pending {
                approved_by: None, ..
            } => Some(Poll::Waiting),
            Pending {
                approved_by: Some(_),
                ..
            } => {
                let sign_in = pending.remove(handle)?;
                Some(Poll::Approved {
                    user: sign_in.approved_by?,
                    device_name: sign_in.device_name,
                    wants_token: sign_in.wants_token,
                })
            }
        }
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
    let code = users::normalized_code(entered);
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
mod tests {
    use super::*;
    use crate::users::Role;

    /// Ten minutes, counted independently of the code under test.
    const TEN_MINUTES: Duration = Duration::from_mins(10);

    fn ada() -> User {
        User {
            id: 1,
            name: "Ada".to_owned(),
            role: Role::Member,
            created_at: 0,
        }
    }

    /// A sign-in started at `now` for the Device "Living room TV", wanting a token.
    fn start(codes: &SignInCodes, now: Instant) -> Started {
        codes.start("Living room TV".to_owned(), true, now).unwrap()
    }

    #[test]
    fn a_code_is_eight_crockford_characters_shown_in_two_groups() {
        let started = start(&SignInCodes::default(), Instant::now());

        let (first, second) = started.code.split_once('-').unwrap();
        assert_eq!((first.len(), second.len()), (4, 4), "{}", started.code);
        assert!(
            started
                .code
                .chars()
                .all(|c| c == '-' || "0123456789ABCDEFGHJKMNPQRSTVWXYZ".contains(c)),
            "{}",
            started.code
        );
    }

    #[test]
    fn approving_then_polling_signs_in_as_the_approver_once() {
        let codes = SignInCodes::default();
        let now = Instant::now();
        let started = start(&codes, now);

        codes.approve(&started.code, &ada(), now).unwrap();
        let first = codes.poll(&started.handle, now);
        let again = codes.poll(&started.handle, now);

        let Some(Poll::Approved {
            user,
            device_name,
            wants_token,
        }) = first
        else {
            panic!("not approved: {first:?}");
        };
        assert_eq!(
            (user.id, device_name.as_str(), wants_token),
            (1, "Living room TV", true)
        );
        assert!(again.is_none(), "{again:?}");
    }

    #[test]
    fn looking_up_a_code_shows_the_device_name_without_approving_it() {
        let codes = SignInCodes::default();
        let now = Instant::now();
        let started = start(&codes, now);

        let device_name = codes.look_up(&started.code, now).unwrap();

        assert_eq!(device_name, "Living room TV");
        assert!(matches!(
            codes.poll(&started.handle, now),
            Some(Poll::Waiting)
        ));
    }

    #[test]
    fn a_code_matches_in_lower_case_and_without_its_dash() {
        let codes = SignInCodes::default();
        let now = Instant::now();
        let started = start(&codes, now);
        let typed = started.code.replace('-', "").to_lowercase();

        codes.approve(&typed, &ada(), now).unwrap();

        assert!(matches!(
            codes.poll(&started.handle, now),
            Some(Poll::Approved { .. })
        ));
    }

    #[test]
    fn an_expired_code_cant_be_looked_up_or_approved() {
        let codes = SignInCodes::default();
        let started_at = Instant::now();
        let started = start(&codes, started_at);
        let expired_at = started_at + TEN_MINUTES;

        let looked_up = codes.look_up(&started.code, expired_at);
        let approved = codes.approve(&started.code, &ada(), expired_at);

        assert!(matches!(looked_up, Err(Error::NotFound)), "{looked_up:?}");
        assert!(matches!(approved, Err(Error::NotFound)), "{approved:?}");
    }

    #[test]
    fn an_unknown_code_cant_be_looked_up_or_approved() {
        let codes = SignInCodes::default();
        let now = Instant::now();
        let started = start(&codes, now);
        // Differs from the real code wherever it has its last character.
        let last = started.code.chars().last().unwrap();
        let unknown = started
            .code
            .replace(last, if last == '0' { "1" } else { "0" });

        let looked_up = codes.look_up(&unknown, now);
        let approved = codes.approve(&unknown, &ada(), now);

        assert!(matches!(looked_up, Err(Error::NotFound)), "{looked_up:?}");
        assert!(matches!(approved, Err(Error::NotFound)), "{approved:?}");
    }

    #[test]
    fn an_approved_code_cant_be_approved_again() {
        let codes = SignInCodes::default();
        let now = Instant::now();
        let started = start(&codes, now);
        codes.approve(&started.code, &ada(), now).unwrap();

        let again = codes.approve(&started.code, &ada(), now);

        assert!(matches!(again, Err(Error::AlreadyApproved)), "{again:?}");
    }

    #[test]
    fn an_unapproved_handle_stays_waiting_until_it_expires() {
        let codes = SignInCodes::default();
        let started_at = Instant::now();
        let started = start(&codes, started_at);
        let just_before = started_at + Duration::from_secs(9 * 60 + 59);

        let waiting = codes.poll(&started.handle, just_before);
        let expired = codes.poll(&started.handle, started_at + TEN_MINUTES);

        assert!(matches!(waiting, Some(Poll::Waiting)), "{waiting:?}");
        assert!(matches!(expired, Some(Poll::Expired)), "{expired:?}");
    }

    #[test]
    fn an_approved_handle_polled_too_late_reports_expired() {
        let codes = SignInCodes::default();
        let started_at = Instant::now();
        let started = start(&codes, started_at);
        codes.approve(&started.code, &ada(), started_at).unwrap();

        let polled = codes.poll(&started.handle, started_at + TEN_MINUTES);

        assert!(matches!(polled, Some(Poll::Expired)), "{polled:?}");
    }

    #[test]
    fn an_unknown_handle_polls_as_nothing() {
        let codes = SignInCodes::default();

        assert!(codes.poll("made-up", Instant::now()).is_none());
    }

    #[test]
    fn starting_forgets_sign_ins_a_lifetime_after_they_expired() {
        let codes = SignInCodes::default();
        let started_at = Instant::now();
        let old = start(&codes, started_at);
        let much_later = started_at + TEN_MINUTES * 2;

        start(&codes, much_later);

        assert!(codes.poll(&old.handle, much_later).is_none());
    }
}
