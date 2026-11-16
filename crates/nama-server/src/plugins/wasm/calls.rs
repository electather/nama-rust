//! The calls running in an enabled Installed Plugin, each under a watchdog that ends it once the
//! Plugin's own time passes its method's limit. Extism's own timeout counts wall-clock time,
//! the Kernel's answers to its `nama` imports included, so an enabled Plugin's calls run without
//! one and under this instead.

use std::{
    collections::HashMap,
    sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError},
    thread,
    time::{Duration, Instant},
};

use extism::{CancelHandle, CompiledPlugin};
use thiserror::Error;

/// How often a watchdog cancels its call again until the call ends: Extism drops a cancel that
/// reaches its timer before the call has started there, as one may right after the instance is
/// made.
const CANCEL_AGAIN_EVERY: Duration = Duration::from_millis(10);

/// How a call [`Calls::stop`] interrupted, or that started after it, fails: its caller gets a
/// plain failure, and it isn't a crash.
#[derive(Debug, Error)]
#[error("the Plugin was disabled")]
pub(super) struct Interrupted;

/// Whether a call failed as [`Interrupted`].
pub(super) fn is_interrupted(err: &extism::Error) -> bool {
    err.is::<Interrupted>()
}

/// How a call fails when another call on its compile ran past its limit: Extism ends that one by
/// bumping the epoch of the engine they share, which interrupts this one too. Its caller gets a
/// plain failure, and it isn't a crash, as the Plugin did nothing wrong in it.
#[derive(Debug, Error)]
#[error("another call of the Plugin ran past its time limit, and ending it ended this one too")]
pub(super) struct Collateral;

/// Whether a call failed as [`Collateral`].
pub(super) fn is_collateral(err: &extism::Error) -> bool {
    err.is::<Collateral>()
}

/// The calls running in one enablement, by their instance's id, which a `nama` import reads from
/// its `CurrentPlugin`. Every compile of the enablement shares one, so stopping them reaches
/// every call of the Plugin, whichever point it is at.
#[derive(Clone, Default)]
pub(super) struct Calls(Arc<Mutex<Running>>);

#[derive(Default)]
struct Running {
    calls: HashMap<u128, Arc<Call>>,
    /// Whether [`Calls::stop`] was called, after which no call starts.
    stopped: bool,
}

impl Calls {
    fn lock(&self) -> MutexGuard<'_, Running> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Calls the export `name` with `input` on a fresh instance of `compiled`, cancelling it as a
    /// timeout once the Plugin's own time passes `limit`, or as [`Interrupted`] once
    /// [`Self::stop`] is called. One that another call's timeout interrupts fails as
    /// [`Collateral`]. Blocks while the Plugin runs.
    // ponytail: one watchdog thread per call; one thread for every call, as Extism's own timer
    // is, if calls get so many that the threads cost.
    pub(super) fn run(
        &self,
        compiled: &CompiledPlugin,
        name: &str,
        input: Vec<u8>,
        limit: Duration,
    ) -> Result<Vec<u8>, extism::Error> {
        let mut plugin = extism::Plugin::new_from_compiled(compiled)?;
        let cancel = plugin.cancel_handle();
        let id = cancel.id.as_u128();
        let call = Arc::new(Call {
            cancel,
            limit,
            clock: Mutex::new(Clock {
                used: Duration::ZERO,
                since: Some(Instant::now()),
                cancelled: None,
                ended: false,
            }),
            changed: Condvar::new(),
        });
        {
            // Under the one lock `stop` takes, so a call either is stopped or never starts.
            let mut running = self.lock();
            if running.stopped {
                return Err(Interrupted.into());
            }
            running.calls.insert(id, Arc::clone(&call));
        }
        let _registered = Registered { calls: self, id };
        let called = thread::scope(|scope| {
            scope.spawn(|| call.watch());
            // Ends the watchdog however the call ends, a panic included, so the scope can join it.
            let _ended = Ended(&call);
            plugin.call(name, input)
        });
        // Extism words a stop, and another call's timeout, as it does this call's own timeout.
        // Any failure once stopped is put down to the stop: the enablement is ending, so it no
        // longer counts as a crash.
        let clock = call.lock();
        match called {
            Err(_) if clock.cancelled == Some(Cancelled::Stopped) => Err(Interrupted.into()),
            Err(err) if clock.cancelled.is_none() && super::is_timeout(&err) => {
                Err(Collateral.into())
            }
            called => called,
        }
    }

    /// Pauses the Plugin's clock of the call on instance `id`, if it is running, until the
    /// returned guard drops: the Kernel's time answering a `nama` import isn't the Plugin's.
    pub(super) fn pause(&self, id: u128) -> Option<impl Drop> {
        let call = Arc::clone(self.lock().calls.get(&id)?);
        call.update(|clock| {
            if let Some(since) = clock.since.take() {
                clock.used += since.elapsed();
            }
        });
        Some(Paused(call))
    }

    /// Interrupts every call running now, and refuses every call after, each failing as
    /// [`Interrupted`]: the enablement is ending.
    pub(super) fn stop(&self) {
        let mut running = self.lock();
        running.stopped = true;
        for call in running.calls.values() {
            call.update(|clock| clock.cancelled = Some(Cancelled::Stopped));
        }
    }
}

/// One running call and its watchdog's state.
struct Call {
    cancel: CancelHandle,
    /// How long the Plugin's own time may run.
    limit: Duration,
    clock: Mutex<Clock>,
    /// Wakes the watchdog when [`Self::clock`] changes.
    changed: Condvar,
}

struct Clock {
    /// The Plugin's own time before `since`.
    used: Duration,
    /// When the Plugin's clock last started; `None` while the Kernel answers an import.
    since: Option<Instant>,
    /// Why its watchdog cancelled it, if it did.
    cancelled: Option<Cancelled>,
    ended: bool,
}

/// Why a call's watchdog cancelled it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Cancelled {
    /// Its Plugin's own time passed its limit.
    AtLimit,
    /// [`Calls::stop`] interrupted it, whether or not it was past its limit too.
    Stopped,
}

impl Call {
    fn lock(&self) -> MutexGuard<'_, Clock> {
        self.clock.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn update(&self, change: impl FnOnce(&mut Clock)) {
        change(&mut self.lock());
        self.changed.notify_all();
    }

    /// Waits until the call ends, cancelling it once it is past its limit or cancelled.
    fn watch(&self) {
        let mut clock = self.lock();
        while !clock.ended {
            let used = clock.used + clock.since.map_or(Duration::ZERO, |since| since.elapsed());
            if clock.cancelled.is_none() && used >= self.limit {
                clock.cancelled = Some(Cancelled::AtLimit);
            }
            let wait = if clock.cancelled.is_some() {
                // An error means Extism's timer is gone, and with it every call it could end.
                drop(self.cancel.cancel());
                Some(CANCEL_AGAIN_EVERY)
            } else {
                clock.since.map(|_| self.limit.saturating_sub(used))
            };
            clock = match wait {
                Some(wait) => {
                    self.changed
                        .wait_timeout(clock, wait)
                        .unwrap_or_else(PoisonError::into_inner)
                        .0
                }
                None => self
                    .changed
                    .wait(clock)
                    .unwrap_or_else(PoisonError::into_inner),
            };
        }
    }
}

/// Takes a call out of [`Calls`] when it ends.
struct Registered<'a> {
    calls: &'a Calls,
    id: u128,
}

impl Drop for Registered<'_> {
    fn drop(&mut self) {
        self.calls.lock().calls.remove(&self.id);
    }
}

/// Ends a call's watchdog when dropped.
struct Ended<'a>(&'a Call);

impl Drop for Ended<'_> {
    fn drop(&mut self) {
        self.0.update(|clock| clock.ended = true);
    }
}

/// Starts a call's clock again when dropped.
struct Paused(Arc<Call>);

impl Drop for Paused {
    fn drop(&mut self) {
        self.0.update(|clock| clock.since = Some(Instant::now()));
    }
}
