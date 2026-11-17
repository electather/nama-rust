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

use super::{Ticked, once};
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

/// A Registry whose Kernel already serves, as after startup. The Counter never calls the Kernel.
fn served() -> Registry {
    let registry = Registry::new();
    serve(&registry);
    registry
}

fn serve(registry: &Registry) {
    registry.serve_kernel(Arc::new(|_, _| {
        panic!("the Counter never calls the Kernel")
    }));
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
    let registry = served();
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
    let registry = served();
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
    let registry = served();
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
    let registry = served();
    let ticks = enabled(&registry, None);

    tokio::time::advance(EVERY * 10).await;
    settle().await;

    assert_eq!(count(&ticks), 0);
}

#[cfg(feature = "testing")]
#[tokio::test(start_paused = true)]
async fn a_registry_set_to_tick_on_demand_ticks_only_when_asked() {
    let registry = served();
    registry.tick_only_on_demand();
    let ticks = enabled(&registry, Some(EVERY));

    tokio::time::advance(EVERY * 10).await;
    settle().await;
    assert_eq!(count(&ticks), 0);

    registry.tick_now(&ID).await.unwrap();
    assert_eq!(count(&ticks), 1);
}

#[cfg(feature = "testing")]
#[tokio::test(start_paused = true)]
async fn tick_now_ticks_a_plugin_its_schedule_never_ticks() {
    let registry = Registry::new();
    let ticks = enabled(&registry, None);

    registry.tick_now(&ID).await.unwrap();

    assert_eq!(count(&ticks), 1);
}

#[cfg(feature = "testing")]
#[tokio::test]
async fn tick_now_refuses_a_plugin_that_is_not_enabled() {
    let registry = Registry::new();

    let refused = registry.tick_now(&ID).await.unwrap_err();

    assert_eq!(refused, "ticked isn't enabled or provides no Automation");
}

/// A Built-in Plugin whose Automation panics when ticked.
#[cfg(feature = "testing")]
struct Panicking;

#[cfg(feature = "testing")]
impl Plugin for Panicking {
    fn manifest(&self) -> Manifest {
        Manifest {
            version: "0.0.0".to_owned(),
            extension_points: vec![PointKey::provide::<dyn Automation>()],
            ..Manifest::new(ID)
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        registrar.provide::<dyn Automation>(Priority::DEFAULT, Arc::new(Panics));
        Ok(())
    }
}

#[cfg(feature = "testing")]
struct Panics;

#[cfg(feature = "testing")]
impl Automation for Panics {
    fn tick<'a>(&'a self, _: &'a Tick) -> BoxFuture<'a, Result<(), PluginError>> {
        panic!("the tick panicked")
    }
}

#[cfg(feature = "testing")]
#[tokio::test]
async fn tick_now_reports_a_tick_that_panicked() {
    let registry = Registry::new();
    registry.enable(Arc::new(Panicking)).unwrap();

    let refused = registry.tick_now(&ID).await.unwrap_err();

    assert_eq!(refused, "ticked's tick panicked, so it is disabled");
    assert!(!registry.enabled().contains(&ID));
}

#[tokio::test]
async fn a_tick_after_its_plugin_was_disabled_finds_no_automation() {
    let registry = Registry::new();
    let ticks = enabled(&registry, None);
    assert!(registry.disable(&ID).await);

    let ticked = once(&registry.extension_points(), &ID).await;

    assert_eq!(ticked, Ticked::NoAutomation);
    assert_eq!(count(&ticks), 0);
}

#[cfg(feature = "testing")]
#[tokio::test]
async fn a_tick_that_panicked_is_told_from_one_with_no_automation() {
    let registry = Registry::new();
    registry.enable(Arc::new(Panicking)).unwrap();

    let ticked = once(&registry.extension_points(), &ID).await;

    assert_eq!(ticked, Ticked::Panicked);
    assert!(!registry.enabled().contains(&ID));
}

#[tokio::test(start_paused = true)]
async fn an_automation_enabled_before_the_kernel_serves_ticks_once_it_does() {
    let registry = Registry::new();
    let ticks = enabled(&registry, Some(EVERY));
    settle().await;
    tokio::time::advance(EVERY * 3).await;
    settle().await;
    assert_eq!(count(&ticks), 0);

    serve(&registry);
    settle().await;
    assert_eq!(count(&ticks), 1);

    tokio::time::advance(EVERY).await;
    settle().await;
    assert_eq!(count(&ticks), 2);
}

#[tokio::test(start_paused = true)]
async fn disabling_while_waiting_for_the_kernel_ends_the_loop() {
    let registry = Registry::new();
    let ticks = enabled(&registry, Some(EVERY));
    settle().await;

    assert!(registry.disable(&ID).await);
    serve(&registry);
    tokio::time::advance(EVERY * 10).await;
    settle().await;

    assert_eq!(count(&ticks), 0);
}
