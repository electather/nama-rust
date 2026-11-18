//! Which Providers' and Observers' last calls failed, so the Admin sees a Plugin that keeps
//! failing instead of it failing quietly on every call (ADR 0050).

use std::{
    collections::HashMap,
    sync::{Mutex, MutexGuard, PoisonError},
};

use nama_plugin_api::{PluginId, PointKey};

use crate::db;

/// A Provider or Observer whose last call failed: why, when, and how many calls in a row have.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failing {
    /// What the last failure said, with the Plugin's secret settings blanked.
    pub last_error: String,
    /// When it failed, in milliseconds since the Unix epoch.
    pub at_ms: i64,
    /// How many calls in a row have failed, the last included: 1 for the first.
    pub in_a_row: u32,
}

/// Each Provider's and Observer's [`Failing`], by Plugin and Extension Point. Held in memory
/// only, so a restart forgets it.
#[derive(Default)]
pub(super) struct Failures(Mutex<HashMap<(PluginId, PointKey), Failing>>);

impl Failures {
    /// Records that the Plugin `plugin`'s handler at `point` failed with `message`, and returns
    /// its run of failures so far.
    pub(super) fn failed(&self, plugin: &PluginId, point: PointKey, message: &str) -> Failing {
        let key = (plugin.clone(), point);
        let mut failures = self.lock();
        let before = failures.get(&key).map_or(0, |failing| failing.in_a_row);
        let failing = Failing {
            last_error: message.to_owned(),
            at_ms: db::now_ms(),
            in_a_row: before.saturating_add(1),
        };
        failures.insert(key, failing.clone());
        failing
    }

    /// Records that the Plugin `plugin`'s handler at `point` answered, which ends its run.
    pub(super) fn answered(&self, plugin: &PluginId, point: PointKey) {
        self.lock().remove(&(plugin.clone(), point));
    }

    /// The Plugin `plugin`'s handler at `point`, if its last call failed.
    pub(super) fn get(&self, plugin: &PluginId, point: PointKey) -> Option<Failing> {
        self.lock().get(&(plugin.clone(), point)).cloned()
    }

    /// Forgets every failure of the Plugin `plugin`, such as when it is disabled.
    pub(super) fn forget(&self, plugin: &PluginId) {
        self.lock().retain(|(failed, _), _| failed != plugin);
    }

    // Nothing panics while the lock is held, so a poisoned map is still whole.
    fn lock(&self) -> MutexGuard<'_, HashMap<(PluginId, PointKey), Failing>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
#[path = "failing_tests.rs"]
mod tests;
