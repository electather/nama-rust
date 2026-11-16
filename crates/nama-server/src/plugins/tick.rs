//! The Automation Tick: the Kernel ticks a Plugin's [`Automation`] on the schedule its Manifest
//! sets in `tick_every`, in the Plugin's scope (ADR 0047), so disabling it ends the loop.

use std::{sync::Arc, time::Duration};

use nama_plugin_api::{
    Manifest, PluginId, PointKey, Registration,
    automation::{Automation, Tick},
};
use tokio::time::MissedTickBehavior;
use tracing::warn;

use super::{ExtensionPoints, Failure, scope::Scope};

/// A new scope for an enablement of `manifest`'s Plugin, ticking its Automation every
/// `tick_every` if it has one and `registrations` provide one. The first tick comes at once, so an
/// Admin enabling the Plugin sees it act without waiting an interval, and tests need no wait.
///
/// The Registry calls it under its lock, so the loop is in the enablement's scope before a
/// disable can find it. Starts nothing outside a Tokio runtime, where no task can run.
pub(super) fn scope(
    points: &ExtensionPoints,
    manifest: &Manifest,
    registrations: &[Registration],
) -> Scope {
    let scope = Scope::default();
    let automation = PointKey::provide::<dyn Automation>();
    let ticks = registrations.iter().any(|it| it.point() == automation);
    // `interval` panics on zero; a Built-in's Manifest isn't checked as an Installed one's is.
    if let Some(every) = manifest
        .tick_every
        .filter(|every| ticks && !every.is_zero())
        && tokio::runtime::Handle::try_current().is_ok()
    {
        scope.spawn(ticking(points.clone(), manifest.id.clone(), every));
    }
    scope
}

/// Ticks `plugin` every `every` until its scope is cancelled or it no longer provides an
/// Automation. `Delay` keeps a slow tick from stacking the ones it missed.
async fn ticking(points: ExtensionPoints, plugin: PluginId, every: Duration) {
    let mut interval = tokio::time::interval(every);
    interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        interval.tick().await;
        match once(&points, &plugin).await {
            // Disabled, or panicked, which disabled it.
            None => return,
            Some(Err(failure)) => {
                warn!(plugin = %plugin, error = failure.message, "automation tick failed");
            }
            Some(Ok(())) => {}
        }
    }
}

/// Ticks `plugin`'s Automation once, as the schedule does. `None` if it isn't enabled, provides no
/// Automation, or panicked, which disabled it.
pub(super) async fn once(
    points: &ExtensionPoints,
    plugin: &PluginId,
) -> Option<Result<(), Failure>> {
    // db's clock, not the system's, so a test's clock reaches Plugins.
    let tick = Tick {
        now_ms: crate::db::now().saturating_mul(1000),
        ..Tick::default()
    };
    let tick = &tick;
    let answer = points
        .ask(plugin, |automation: Arc<dyn Automation>| async move {
            automation.tick(tick).await
        })
        .await?;
    Some(answer.result)
}

#[cfg(test)]
#[path = "tick_tests.rs"]
mod tests;
