//! Slowing down guesses at passwords, the Setup Code and Sign-in Codes, by refusing attempts after
//! too many recent failures: Setup, signing in and Reauthenticate from one client IP or for one User
//! name, and Sign-in Code lookups and approvals from one Device. Also refusing Sign-in Code starts
//! from one client IP after too many recent ones, so one client can't fill the pool of waiting
//! codes for everyone.
//!
//! Counts live in memory only. They expire as the window slides past them and a restart forgets
//! them, so nobody is ever locked out for good.

use std::{
    collections::{HashMap, VecDeque},
    hash::Hash,
    net::IpAddr,
    sync::{Mutex, MutexGuard, PoisonError},
    time::{Duration, Instant},
};

use super::{
    NAME_LENGTH,
    names::{name_key, normalised},
};

const WINDOW: Duration = Duration::from_mins(15);

/// How many failures for one User name the window may hold before attempts for it are refused.
const FAILURES_PER_NAME: usize = 10;

/// How many failures from one client IP, Setup, signing in and Reauthenticate together, the window
/// may hold before its attempts are refused.
const FAILURES_PER_IP: usize = 50;

/// How many failed Sign-in Code lookups and approvals from one Device the window may hold before
/// its lookups and approvals are refused.
const FAILURES_PER_DEVICE: usize = 10;

/// How many Sign-in Code starts from one client IP the window may hold before its starts are
/// refused: far more than a household of Devices behind one NAT needs, each starting a fresh code
/// when one expires.
const STARTS_PER_IP: usize = 20;

/// The recent failed Setup, signing in and Reauthenticate attempts, by client IP and by User name.
#[derive(Debug, Default)]
pub(super) struct SignInLimits {
    failures: Mutex<Failures>,
}

#[derive(Debug, Default)]
struct Failures {
    by_ip: Window<IpAddr>,
    by_name: Window<String>,
}

/// An attempt let through. It counts as a failure unless it is reported as
/// [`succeeded`](SignInLimits::succeeded), so one that fails in any way, or a burst of them
/// still running, can't slip past the limits.
#[derive(Debug)]
pub(crate) struct Attempt {
    ip: IpAddr,
    name_key: Option<String>,
    at: Instant,
}

#[derive(Debug, thiserror::Error)]
#[error("too many failed attempts; try again in {} seconds", self.retry_after_secs())]
pub(crate) struct Blocked {
    /// How long until the oldest count in the way leaves the window.
    retry_after: Duration,
}

impl SignInLimits {
    /// Lets an attempt from `ip`, signing in as `name` if it names a User, through at `now` unless
    /// too many have failed recently, and counts it as failed until it succeeds.
    pub(super) fn admit(
        &self,
        ip: IpAddr,
        name: Option<&str>,
        now: Instant,
    ) -> Result<Attempt, Blocked> {
        let name_key = name.map(counted_name_key);
        let mut failures = self.lock();
        let ip_wait = failures.by_ip.wait(&ip, FAILURES_PER_IP, now);
        let name_wait = name_key
            .as_ref()
            .and_then(|key| failures.by_name.wait(key, FAILURES_PER_NAME, now));
        if let Some(retry_after) = ip_wait.max(name_wait) {
            return Err(Blocked { retry_after });
        }
        failures.by_ip.record(ip, now);
        if let Some(key) = &name_key {
            failures.by_name.record(key.clone(), now);
        }
        Ok(Attempt {
            ip,
            name_key,
            at: now,
        })
    }

    /// Un-counts `attempt`: it succeeded, or failed for a reason a guess can't get, such as Setup
    /// on a Server already set up. A successful sign-in also forgets every failure for its name.
    pub(super) fn succeeded(&self, attempt: &Attempt) {
        let mut failures = self.lock();
        failures.by_ip.forget(&attempt.ip, attempt.at);
        if let Some(key) = &attempt.name_key {
            failures.by_name.clear(key);
        }
    }

    fn lock(&self) -> MutexGuard<'_, Failures> {
        // Every change leaves the counts consistent, so a panic elsewhere doesn't spoil them.
        self.failures.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The recent failed Sign-in Code lookups and approvals, lookups and approvals together, by the id
/// of the Device that made them.
#[derive(Debug, Default)]
pub(super) struct SignInCodeLimits {
    failures: Mutex<Window<i64>>,
}

/// A Sign-in Code lookup or approval let through. Like an [`Attempt`], it counts as a failure
/// unless it is reported as [`wasnt_a_guess`](SignInCodeLimits::wasnt_a_guess).
#[derive(Debug)]
pub(super) struct SignInCodeAttempt {
    device: i64,
    at: Instant,
}

impl SignInCodeLimits {
    /// Lets a lookup or approval by the Device `device` at `now` through unless too many of its
    /// own have failed recently, and counts it as failed until it turns out not to be a guess.
    pub(super) fn admit(&self, device: i64, now: Instant) -> Result<SignInCodeAttempt, Blocked> {
        let mut failures = self.lock();
        if let Some(retry_after) = failures.wait(&device, FAILURES_PER_DEVICE, now) {
            return Err(Blocked { retry_after });
        }
        failures.record(device, now);
        Ok(SignInCodeAttempt { device, at: now })
    }

    /// Un-counts `attempt`, whose code turned out to exist: it succeeded, or failed for a reason
    /// a guess can't get, such as the code being approved already.
    pub(super) fn wasnt_a_guess(&self, attempt: &SignInCodeAttempt) {
        self.lock().forget(&attempt.device, attempt.at);
    }

    fn lock(&self) -> MutexGuard<'_, Window<i64>> {
        // As for `SignInLimits`: every change leaves the counts consistent.
        self.failures.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The recent Sign-in Code starts, by client IP. Kept apart from [`SignInLimits`], so starting
/// codes and failing to sign in never use up each other's allowance.
#[derive(Debug, Default)]
pub(super) struct SignInCodeStartLimits {
    starts: Mutex<Window<IpAddr>>,
}

impl SignInCodeStartLimits {
    /// Lets a Sign-in Code start from `ip` at `now` through unless it started too many recently,
    /// and counts it. A start is never un-counted: unlike a guess, it holds a waiting code whatever
    /// happens next.
    pub(super) fn admit(&self, ip: IpAddr, now: Instant) -> Result<(), Blocked> {
        // As for `SignInLimits`: every change leaves the counts consistent.
        let mut starts = self.starts.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(retry_after) = starts.wait(&ip, STARTS_PER_IP, now) {
            return Err(Blocked { retry_after });
        }
        starts.record(ip, now);
        Ok(())
    }
}

impl Blocked {
    /// The retry delay in whole seconds, rounded up so a client that waits it is let through.
    pub(crate) fn retry_after_secs(&self) -> u64 {
        self.retry_after.as_secs() + u64::from(self.retry_after.subsec_nanos() > 0)
    }
}

/// What failures for the User name `name` are counted under: the key sign-in looks it up by, of
/// the name cut short past the longest a User can have, so a request can't have a huge name kept
/// for the whole window. Names that long are no User's, so cutting them short loses nothing. It is
/// cut once in NFC, as names are measured, so a name typed decomposed is counted whole.
fn counted_name_key(name: &str) -> String {
    let capped: String = normalised(name)
        .chars()
        .take(NAME_LENGTH.end() + 1)
        .collect();
    name_key(&capped)
}

/// When each key's counts still in the window happened, oldest first: failures, or Sign-in Code
/// starts.
#[derive(Debug)]
struct Window<K> {
    times: HashMap<K, VecDeque<Instant>>,
    /// When keys whose counts had all left the window were last dropped.
    swept_at: Option<Instant>,
}

impl<K> Default for Window<K> {
    fn default() -> Self {
        Self {
            times: HashMap::new(),
            swept_at: None,
        }
    }
}

impl<K: Hash + Eq> Window<K> {
    /// How long `key` must wait, as of `now`, before it has fewer than `limit` counts in the
    /// window, if it has that many now.
    fn wait(&mut self, key: &K, limit: usize, now: Instant) -> Option<Duration> {
        let times = self.times.get_mut(key)?;
        while times
            .front()
            .is_some_and(|&at| now.saturating_duration_since(at) >= WINDOW)
        {
            times.pop_front();
        }
        // Attempts are only recorded below the limit, so at the limit the oldest is the one that
        // has to leave.
        let oldest = *times.front()?;
        (times.len() >= limit).then(|| WINDOW.saturating_sub(now.saturating_duration_since(oldest)))
    }

    /// Counts one for `key` at `at`. Once a window, it also drops the keys whose counts have all
    /// left it: sweeping on every count would make each one slower the more peers were counted
    /// recently.
    fn record(&mut self, key: K, at: Instant) {
        if self
            .swept_at
            .is_none_or(|swept| at.saturating_duration_since(swept) >= WINDOW)
        {
            self.times.retain(|_, times| {
                times
                    .back()
                    .is_some_and(|&last| at.saturating_duration_since(last) < WINDOW)
            });
            self.swept_at = Some(at);
        }
        self.times.entry(key).or_default().push_back(at);
    }

    fn forget(&mut self, key: &K, at: Instant) {
        if let Some(times) = self.times.get_mut(key)
            && let Some(index) = times.iter().position(|&time| time == at)
        {
            times.remove(index);
        }
    }

    fn clear(&mut self, key: &K) {
        self.times.remove(key);
    }
}

#[cfg(test)]
#[path = "rate_limits_tests.rs"]
mod tests;
