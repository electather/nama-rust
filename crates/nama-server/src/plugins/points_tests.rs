use std::{sync::Arc, time::Duration};

use nama_plugin_api::{
    BoxFuture, Manifest, ObservePoint, Observer, Plugin, PluginError, PluginId, PointKey, Priority,
    Registrar,
};
use tokio::{sync::Notify, time::timeout};
use tracing::info;

use crate::{log_capture::CapturedLog, plugins::Registry};

const GATED: PluginId = PluginId::from_static("gated");
const WAIT: Duration = Duration::from_secs(5);

/// Observe: someone knocked.
struct Knocks;

impl ObservePoint for Knocks {
    const NAME: &'static str = "knocks";
    type Event = ();
}

/// A Provider whose one call waits until it is let through.
trait Gate: Send + Sync {
    fn pass(&self) -> BoxFuture<'_, ()>;
}

/// Tells a test that a [`Gated`] call started, or its handler was dropped, and lets the call end.
#[derive(Default)]
struct Signals {
    called: Notify,
    release: Notify,
    dropped: Notify,
}

/// What a [`Gated`] handler panics on.
#[derive(Clone, Copy, PartialEq)]
enum Panics {
    Never,
    /// As the handler is dropped.
    OnDrop,
    /// As a call that hasn't ended is dropped.
    OnCallDropped,
}

/// Panics as it is dropped, if armed.
struct Bomb(bool);

impl Drop for Bomb {
    fn drop(&mut self) {
        assert!(!self.0, "a pending call panicked as it was dropped");
    }
}

/// A handler whose calls wait for [`Signals::release`], and which logs as it is dropped.
struct Gated {
    signals: Arc<Signals>,
    panics: Panics,
}

impl Gated {
    async fn wait(&self) {
        let _bomb = Bomb(self.panics == Panics::OnCallDropped);
        self.signals.called.notify_one();
        self.signals.release.notified().await;
    }
}

impl Observer<Knocks> for Gated {
    fn observe<'a>(&'a self, (): &'a ()) -> BoxFuture<'a, ()> {
        Box::pin(self.wait())
    }
}

impl Gate for Gated {
    fn pass(&self) -> BoxFuture<'_, ()> {
        Box::pin(self.wait())
    }
}

impl Drop for Gated {
    fn drop(&mut self) {
        info!("gated dropped");
        self.signals.dropped.notify_one();
        assert!(
            self.panics != Panics::OnDrop,
            "gated panicked as it was dropped"
        );
    }
}

/// A Built-in Plugin with one [`Gated`] handler, at `point`: [`Knocks`] or a [`Gate`].
struct Gating {
    point: PointKey,
    signals: Arc<Signals>,
    panics: Panics,
}

impl Plugin for Gating {
    fn manifest(&self) -> Manifest {
        Manifest {
            version: "0.0.0".to_owned(),
            extension_points: vec![self.point],
            ..Manifest::new(GATED)
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        let gated = Gated {
            signals: Arc::clone(&self.signals),
            panics: self.panics,
        };
        if self.point == PointKey::observe::<Knocks>() {
            registrar.observe::<Knocks>(gated);
        } else {
            registrar.provide::<dyn Gate>(Priority::DEFAULT, Arc::new(gated));
        }
        Ok(())
    }
}

/// A Registry with [`Gating`] enabled at `point`, and its handler's signals.
fn enabled(point: PointKey, panics: Panics) -> (Registry, Arc<Signals>) {
    let signals = Arc::new(Signals::default());
    let registry = Registry::new();
    registry
        .enable(Arc::new(Gating {
            point,
            signals: Arc::clone(&signals),
            panics,
        }))
        .unwrap();
    (registry, signals)
}

/// Asks the [`Gate`] first, disabling its Plugin while the call waits and then letting it through.
async fn ask_while_disabling(registry: &Registry, signals: &Signals) {
    let asking = registry.ask_first(|gate: Arc<dyn Gate>| async move {
        gate.pass().await;
        Ok::<_, PluginError>(())
    });
    let disabling = async {
        signals.called.notified().await;
        assert!(registry.disable(&GATED).await);
        signals.release.notify_one();
    };
    let (answer, ()) = tokio::join!(asking, disabling);
    assert!(answer.is_some_and(|answer| answer.result.is_ok()));
}

/// Asks the [`Gate`] first, disabling its Plugin while the call waits if `disabling`, then drops
/// the asking without letting the call through, as a client that hangs up does.
async fn drop_asking(registry: &Registry, signals: &Signals, disabling: bool) {
    let mut asking = Box::pin(registry.ask_first(|gate: Arc<dyn Gate>| async move {
        gate.pass().await;
        Ok::<_, PluginError>(())
    }));
    tokio::select! {
        _ = &mut asking => panic!("the call ended before it was let through"),
        () = signals.called.notified() => {}
    }
    if disabling {
        assert!(registry.disable(&GATED).await);
    }
    drop(asking);
}

fn assert_panicked_as_dropped(logged: &str) {
    assert!(
        logged.lines().any(|line| {
            line.contains("plugin panicked while being dropped") && line.contains("plugin=gated")
        }),
        "{logged}"
    );
}

fn assert_dropped_in_its_span(logged: &str) {
    let line = logged
        .lines()
        .find(|line| line.ends_with("gated dropped"))
        .unwrap_or_else(|| panic!("nothing dropped: {logged}"));
    assert!(line.contains("plugin{plugin=gated}:"), "{line}");
}

#[tokio::test]
async fn an_observer_disabled_mid_call_is_dropped_in_a_span_naming_its_plugin() {
    let log = CapturedLog::start();
    let (registry, signals) = enabled(PointKey::observe::<Knocks>(), Panics::Never);

    registry.notify::<Knocks>(());
    signals.called.notified().await;
    assert!(registry.disable(&GATED).await);
    signals.release.notify_one();
    timeout(WAIT, signals.dropped.notified())
        .await
        .expect("the Observer is dropped once its call ends");

    assert_dropped_in_its_span(&log.finish());
}

#[test]
fn an_observer_disabled_mid_call_is_dropped_in_a_span_naming_its_plugin_as_the_runtime_stops() {
    let log = CapturedLog::start();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let _registry = runtime.block_on(async {
        let (registry, signals) = enabled(PointKey::observe::<Knocks>(), Panics::Never);
        registry.notify::<Knocks>(());
        signals.called.notified().await;
        assert!(registry.disable(&GATED).await);
        registry
    });
    drop(runtime);

    assert_dropped_in_its_span(&log.finish());
}

#[tokio::test]
async fn a_provider_disabled_mid_call_is_dropped_in_a_span_naming_its_plugin() {
    let log = CapturedLog::start();
    let (registry, signals) = enabled(PointKey::provide::<dyn Gate>(), Panics::Never);

    ask_while_disabling(&registry, &signals).await;

    assert_dropped_in_its_span(&log.finish());
}

#[tokio::test]
async fn a_provider_disabled_mid_call_that_panics_as_it_is_dropped_is_contained() {
    let log = CapturedLog::start();
    let (registry, signals) = enabled(PointKey::provide::<dyn Gate>(), Panics::OnDrop);

    ask_while_disabling(&registry, &signals).await;

    assert_panicked_as_dropped(&log.finish());
}

#[tokio::test]
async fn a_provider_disabled_mid_call_whose_caller_hangs_up_is_dropped_in_a_span_naming_its_plugin()
{
    let log = CapturedLog::start();
    let (registry, signals) = enabled(PointKey::provide::<dyn Gate>(), Panics::Never);

    drop_asking(&registry, &signals, true).await;

    assert_dropped_in_its_span(&log.finish());
}

#[tokio::test]
async fn a_provider_disabled_mid_call_that_panics_as_its_caller_hangs_up_is_contained() {
    let log = CapturedLog::start();
    let (registry, signals) = enabled(PointKey::provide::<dyn Gate>(), Panics::OnDrop);

    drop_asking(&registry, &signals, true).await;

    assert_panicked_as_dropped(&log.finish());
}

#[tokio::test]
async fn a_pending_call_that_panics_as_its_caller_hangs_up_is_contained_and_stays_enabled() {
    let log = CapturedLog::start();
    let (registry, signals) = enabled(PointKey::provide::<dyn Gate>(), Panics::OnCallDropped);

    drop_asking(&registry, &signals, false).await;

    assert_panicked_as_dropped(&log.finish());
    assert_eq!(registry.enabled(), [GATED]);
    assert!(
        registry
            .failing(&GATED, PointKey::provide::<dyn Gate>())
            .is_none()
    );
}
