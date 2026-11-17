//! The Automation Tick: the Kernel ticks a Plugin's [`Automation`] on the schedule its Manifest
//! sets in `tick_every`, in the Plugin's scope (ADR 0047), so disabling it ends the loop.

use std::{sync::Arc, time::Duration};

use nama_plugin_api::{
    Manifest, PluginId, PointKey, Registration,
    automation::{Automation, Tick},
};
use tokio::time::MissedTickBehavior;
use tracing::warn;

use super::{ExtensionPoints, Failure, points::Asked, scope::Scope};

/// A new scope for an enablement of `manifest`'s Plugin, ticking its Automation every
/// `tick_every` if it has one, `registrations` provide one and `on_schedule` is set: a Registry
/// set to tick on demand passes it unset and never starts the loop. The first tick comes once the
/// Kernel serves: at once for a Plugin enabled after that, so an Admin enabling it sees it act
/// without waiting an interval and tests need no wait, and once serving starts for one enabled at
/// startup, whose tick the Kernel would otherwise refuse.
///
/// The Registry calls it under its lock, so the loop is in the enablement's scope before a
/// disable can find it, and reads `on_schedule` from the state it holds: reading it through the
/// Registry would retake that lock. Starts nothing outside a Tokio runtime, where no task can run.
pub(super) fn scope(
    points: &ExtensionPoints,
    manifest: &Manifest,
    registrations: &[Registration],
    on_schedule: bool,
) -> Scope {
    let scope = Scope::default();
    let automation = PointKey::provide::<dyn Automation>();
    let ticks = registrations.iter().any(|it| it.point() == automation);
    // `interval` panics on zero; a Built-in's Manifest isn't checked as an Installed one's is.
    if let Some(every) = manifest
        .tick_every
        .filter(|every| on_schedule && ticks && !every.is_zero())
        && tokio::runtime::Handle::try_current().is_ok()
    {
        scope.spawn(ticking(points.clone(), manifest.id.clone(), every));
    }
    scope
}

/// Ticks `plugin` every `every`, from when the Kernel serves, until its scope is cancelled or it
/// no longer provides an Automation. `Delay` keeps a slow tick from stacking the ones it missed.
async fn ticking(points: ExtensionPoints, plugin: PluginId, every: Duration) {
    points.kernel_served().await;
    let mut interval = tokio::time::interval(every);
    interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        interval.tick().await;
        match once(&points, &plugin).await {
            // Disabled, or panicked, which disabled it.
            Ticked::NoAutomation | Ticked::Panicked => return,
            Ticked::Ran(Err(failure)) => {
                warn!(plugin = %plugin, error = failure.message, "automation tick failed");
            }
            Ticked::Ran(Ok(())) => {}
        }
    }
}

/// What ticking a Plugin's Automation once came to.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Ticked {
    /// The tick ran, and succeeded or failed.
    Ran(Result<(), Failure>),
    /// The Plugin isn't enabled or provides no Automation, such as one disabled since.
    NoAutomation,
    /// The tick panicked, which disabled the Plugin.
    Panicked,
}

/// Ticks `plugin`'s Automation once, as the schedule does.
pub(super) async fn once(points: &ExtensionPoints, plugin: &PluginId) -> Ticked {
    // db's clock, not the system's, so a test's clock reaches Plugins.
    let tick = Tick {
        now_ms: crate::db::now().saturating_mul(1000),
        ..Tick::default()
    };
    let tick = &tick;
    let asked = points
        .ask_one(plugin, |automation: Arc<dyn Automation>| async move {
            automation.tick(tick).await
        })
        .await;
    match asked {
        Asked::Answered(answer) => Ticked::Ran(answer.result),
        Asked::Absent => Ticked::NoAutomation,
        Asked::Panicked => Ticked::Panicked,
    }
}

#[cfg(test)]
#[path = "tick_tests.rs"]
mod tests;
