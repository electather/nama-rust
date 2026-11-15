use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    automation::{Automation, Tick},
};

use crate::plugins::Registry;

const EVERY: Duration = Duration::from_secs(60);
const ID: PluginId = PluginId::from_static("ticked");

/// A Plugin whose Automation counts its ticks.
struct Counting {
    ticks: Arc<AtomicUsize>,
    tick_every: Option<Duration>,
}

impl Plugin for Counting {
    fn manifest(&self) -> Manifest {
        Manifest {
            version: "0.0.0".to_owned(),
            extension_points: vec![PointKey::provide::<dyn Automation>()],
            tick_every: self.tick_every,
            ..Manifest::new(ID)
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        registrar.provide::<dyn Automation>(
            Priority::DEFAULT,
            Arc::new(Counter(Arc::clone(&self.ticks))),
        );
        Ok(())
    }
}

struct Counter(Arc<AtomicUsize>);

impl Automation for Counter {
    fn tick<'a>(&'a self, _: &'a Tick) -> BoxFuture<'a, Result<(), PluginError>> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Ok(()) })
    }
}

/// Enables a [`Counting`] Plugin ticked every `tick_every`, returning its tick count.
fn enabled(registry: &Registry, tick_every: Option<Duration>) -> Arc<AtomicUsize> {
    let ticks = Arc::default();
    registry
        .enable(Arc::new(Counting {
            ticks: Arc::clone(&ticks),
            tick_every,
        }))
        .unwrap();
    ticks
}

/// Lets every ready task run, without moving the paused clock.
async fn settle() {
    for _ in 0..10 {
        tokio::task::yield_now().await;
    }
}

fn count(ticks: &AtomicUsize) -> usize {
    ticks.load(Ordering::SeqCst)
}

#[tokio::test(start_paused = true)]
async fn ticks_at_enable_then_once_per_interval() {
    let registry = Registry::new();
    let ticks = enabled(&registry, Some(EVERY));
    settle().await;
    assert_eq!(count(&ticks), 1);

    tokio::time::advance(EVERY).await;
    settle().await;
    assert_eq!(count(&ticks), 2);

    tokio::time::advance(EVERY).await;
    settle().await;
    assert_eq!(count(&ticks), 3);
}

#[tokio::test(start_paused = true)]
async fn no_tick_after_disable_returns() {
    let registry = Registry::new();
    let ticks = enabled(&registry, Some(EVERY));
    settle().await;

    assert!(registry.disable(&ID).await);
    let before = count(&ticks);
    tokio::time::advance(EVERY * 10).await;
    settle().await;

    assert_eq!(count(&ticks), before);
}

#[tokio::test(start_paused = true)]
async fn enabling_again_starts_a_fresh_loop() {
    let registry = Registry::new();
    let first = enabled(&registry, Some(EVERY));
    settle().await;
    assert!(registry.disable(&ID).await);

    let second = enabled(&registry, Some(EVERY));
    settle().await;
    assert_eq!(count(&second), 1);

    tokio::time::advance(EVERY).await;
    settle().await;
    assert_eq!(count(&second), 2);
    assert_eq!(count(&first), 1);
}

#[tokio::test(start_paused = true)]
async fn a_plugin_without_tick_every_is_never_ticked() {
    let registry = Registry::new();
    let ticks = enabled(&registry, None);

    tokio::time::advance(EVERY * 10).await;
    settle().await;

    assert_eq!(count(&ticks), 0);
}
