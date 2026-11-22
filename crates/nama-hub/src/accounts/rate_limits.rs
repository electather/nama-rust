//! Slowing down guesses at passwords, by refusing sign-ins after too many recent failures from one
//! client IP or for one email, as the Server's `users/rate_limits.rs` does for User names; and
//! bounding the argon2 work, throwaway Accounts and probing for taken emails one client can cause,
//! by refusing Account creations after too many recent ones from its IP. A client IP is counted by
//! its /64 if it is IPv6.
//!
//! ponytail: counts live in this process's memory (ADR 0096); they move to PostgreSQL once the Hub
//! runs as replicas. They expire as the window slides past them and a restart forgets them, so
//! nobody is ever locked out for good.

use std::{
    collections::{HashMap, VecDeque},
    hash::Hash,
    net::IpAddr,
    sync::{Mutex, MutexGuard, PoisonError},
    time::{Duration, Instant},
};

use super::EMAIL_LENGTH;

const WINDOW: Duration = Duration::from_mins(15);

/// How many failures for one email the window may hold before sign-ins with it are refused.
const FAILURES_PER_EMAIL: usize = 10;

/// How many failures from one client IP the window may hold before its sign-ins are refused.
const FAILURES_PER_IP: usize = 50;

/// How many Account creations from one client IP, whether or not they succeeded, the window may
/// hold before its creations are refused.
const CREATIONS_PER_IP: usize = 10;

/// The recent failed sign-ins, by client IP and by email.
#[derive(Debug, Default)]
pub(super) struct SignInLimits {
    failures: Mutex<Failures>,
}

#[derive(Debug, Default)]
struct Failures {
    by_ip: Window<IpAddr>,
    by_email: Window<String>,
}

/// A sign-in let through. It counts as a failure unless it is reported as
/// [`succeeded`](SignInLimits::succeeded), so one that fails in any way, or a burst of them still
/// running, can't slip past the limits.
#[derive(Debug)]
pub(super) struct Attempt {
    ip: IpAddr,
    email: String,
    at: Instant,
}

/// The recent Account creations, by client IP.
#[derive(Debug, Default)]
pub(super) struct CreationLimits {
    creations: Mutex<Window<IpAddr>>,
}

/// A sign-in or an Account creation refused for too many recent attempts.
#[derive(Debug, thiserror::Error)]
#[error("too many attempts; try again in {} seconds", self.retry_after_secs())]
pub(crate) struct Blocked {
    /// How long until the oldest attempt in the way leaves the window.
    retry_after: Duration,
}

impl Blocked {
    /// The retry delay in whole seconds, rounded up so a client that waits it is let through.
    pub(crate) fn retry_after_secs(&self) -> u64 {
        self.retry_after.as_secs() + u64::from(self.retry_after.subsec_nanos() > 0)
    }
}

impl SignInLimits {
    /// Lets a sign-in from `ip` with `email` through at `now` unless too many have failed
    /// recently, and counts it as failed until it succeeds.
    pub(super) fn admit(&self, ip: IpAddr, email: &str, now: Instant) -> Result<Attempt, Blocked> {
        let ip = counted_ip(ip);
        let email = counted_email(email);
        let mut failures = self.lock();
        let ip_wait = failures.by_ip.wait(&ip, FAILURES_PER_IP, now);
        let email_wait = failures.by_email.wait(&email, FAILURES_PER_EMAIL, now);
        if let Some(retry_after) = ip_wait.max(email_wait) {
            return Err(Blocked { retry_after });
        }
        failures.by_ip.record(ip, now);
        failures.by_email.record(email.clone(), now);
        Ok(Attempt { ip, email, at: now })
    }

    /// Un-counts `attempt`, which succeeded, and forgets every failure for its email.
    pub(super) fn succeeded(&self, attempt: &Attempt) {
        let mut failures = self.lock();
        failures.by_ip.forget(&attempt.ip, attempt.at);
        failures.by_email.clear(&attempt.email);
    }

    fn lock(&self) -> MutexGuard<'_, Failures> {
        lock(&self.failures)
    }
}

impl CreationLimits {
    /// Lets an Account creation from `ip` through at `now` unless too many have been let through
    /// recently, and counts it.
    pub(super) fn admit(&self, ip: IpAddr, now: Instant) -> Result<(), Blocked> {
        let ip = counted_ip(ip);
        let mut creations = lock(&self.creations);
        if let Some(retry_after) = creations.wait(&ip, CREATIONS_PER_IP, now) {
            return Err(Blocked { retry_after });
        }
        creations.record(ip, now);
        Ok(())
    }
}

fn lock<T>(counts: &Mutex<T>) -> MutexGuard<'_, T> {
    // Every change leaves the counts consistent, so a panic elsewhere doesn't spoil them.
    counts.lock().unwrap_or_else(PoisonError::into_inner)
}

/// What failures for `email` are counted under: lowercased, as emails are compared, and cut short
/// past the longest an Account's can be, so a request can't have a huge one kept for the window.
fn counted_email(email: &str) -> String {
    email
        .chars()
        .take(EMAIL_LENGTH + 1)
        .collect::<String>()
        .to_lowercase()
}

/// What failures from `ip` are counted under: an IPv6 address by its /64, since one client
/// normally holds a whole /64 and could use a new address for each attempt.
fn counted_ip(ip: IpAddr) -> IpAddr {
    match ip.to_canonical() {
        IpAddr::V6(v6) => IpAddr::V6((u128::from(v6) & !u128::from(u64::MAX)).into()),
        v4 @ IpAddr::V4(_) => v4,
    }
}

/// When each key's attempts still in the window happened, oldest first.
#[derive(Debug)]
struct Window<K> {
    times: HashMap<K, VecDeque<Instant>>,
    /// When keys whose attempts had all left the window were last dropped.
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
    /// How long `key` must wait, as of `now`, before it has fewer than `limit` attempts in the
    /// window, if it has that many now.
    fn wait(&mut self, key: &K, limit: usize, now: Instant) -> Option<Duration> {
        let times = self.times.get_mut(key)?;
        while times
            .front()
            .is_some_and(|&at| now.saturating_duration_since(at) >= WINDOW)
        {
            times.pop_front();
        }
        // Attempts are only recorded below the limit, so at the limit the oldest has to leave.
        let oldest = *times.front()?;
        (times.len() >= limit).then(|| WINDOW.saturating_sub(now.saturating_duration_since(oldest)))
    }

    /// Counts one for `key` at `at`. Once a window, it also drops the keys whose attempts have all
    /// left it: sweeping on every count would make each one slower the more clients tried lately.
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

    fn clear(&mut self, key: &K) {
        self.times.remove(key);
    }

    fn forget(&mut self, key: &K, at: Instant) {
        if let Some(times) = self.times.get_mut(key)
            && let Some(index) = times.iter().position(|&time| time == at)
        {
            times.remove(index);
        }
    }
}

#[cfg(test)]
#[path = "rate_limits_tests.rs"]
mod tests;
