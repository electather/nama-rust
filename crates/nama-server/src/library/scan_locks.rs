//! Each Library's scan lock, which the Scanner and Matching both take.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, PoisonError},
};

/// Each Library's scan lock, shared by the Scanner and its Matcher.
#[derive(Debug, Clone)]
pub(crate) struct ScanLocks(Arc<Mutex<HashMap<i64, Arc<tokio::sync::Mutex<()>>>>>);

impl ScanLocks {
    /// No Library's lock yet. Only `Scanner::new` calls it (and the test-only `for_tests`): it
    /// hands the same locks to its Matcher, so no Match is fixed under locks a scan doesn't take.
    pub(crate) fn new() -> Self {
        Self(Arc::default())
    }

    /// No Library's lock yet, for a Matching or Requests test that builds a Matcher without a
    /// Scanner. The one other maker of `ScanLocks`, and test-only.
    #[cfg(test)]
    pub(crate) fn for_tests() -> Self {
        Self::new()
    }

    /// Waits for any scan of the Library `id` to finish, then takes its scan lock.
    pub(crate) async fn lock(&self, id: i64) -> ScanLock {
        let mutex = Arc::clone(lock(&self.0).entry(id).or_default());
        ScanLock {
            library: id,
            _guard: mutex.lock_owned().await,
        }
    }
}

/// A Library's scan lock: while it's held, no scan of the Library runs or starts, so the
/// Library's items don't change. Only [`ScanLocks`] makes one.
#[derive(Debug)]
pub(crate) struct ScanLock {
    library: i64,
    _guard: tokio::sync::OwnedMutexGuard<()>,
}

impl ScanLock {
    /// The Library this lock covers.
    pub(crate) const fn library(&self) -> i64 {
        self.library
    }
}

/// Locks `mutex`, which no code panics while holding.
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
