//! The plugin registry: enabling and disabling Plugins and calling them at Extension Points.
#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test helpers and test Plugins panic to fail a test or to test panic containment"
)]

use std::{sync::Arc, time::Duration};

use nama_plugin_api::{
    BoxFuture, InterceptPoint, Interceptor, ObservePoint, Observer, OnFailure, Plugin, PluginError,
    PluginId, Priority, Registrar, Verdict,
};
use nama_server::plugins::{EnableError, Outcome, Registry};
use tokio::sync::mpsc::{self, error::TryRecvError};

const WAIT: Duration = Duration::from_secs(5);

/// A test Plugin whose `register` is the given closure.
struct TestPlugin<F> {
    id: &'static str,
    register: F,
}

impl<F: Fn(&mut Registrar) + Send + Sync + 'static> Plugin for TestPlugin<F> {
    fn id(&self) -> PluginId {
        PluginId::from_static(self.id)
    }

    fn version(&self) -> &'static str {
        "0.0.0"
    }

    fn register(&self, registrar: &mut Registrar) {
        (self.register)(registrar);
    }
}

fn plugin(
    id: &'static str,
    register: impl Fn(&mut Registrar) + Send + Sync + 'static,
) -> Arc<dyn Plugin> {
    Arc::new(TestPlugin { id, register })
}

/// Observe: a number happened.
struct Numbers;

impl ObservePoint for Numbers {
    const NAME: &'static str = "numbers";
    type Event = u32;
}

/// Forwards each number it observes, tagged with its name.
#[derive(Clone)]
struct Forward(&'static str, mpsc::UnboundedSender<(&'static str, u32)>);

impl Observer<Numbers> for Forward {
    fn observe<'a>(&'a self, event: &'a u32) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            let _: Result<(), _> = self.1.send((self.0, *event));
        })
    }
}

async fn next<T>(rx: &mut mpsc::UnboundedReceiver<T>) -> T {
    tokio::time::timeout(WAIT, rx.recv())
        .await
        .expect("timed out waiting for an Observer")
        .expect("channel closed")
}

#[tokio::test]
async fn observers_are_told_events_until_their_plugin_is_disabled() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let registry = Registry::new();
    let forward_a = Forward("a", tx.clone());
    registry
        .enable(plugin("a", move |r| {
            r.observe::<Numbers>(forward_a.clone());
        }))
        .unwrap();
    assert_eq!(registry.enabled(), [PluginId::from_static("a")]);

    registry.notify::<Numbers>(1);
    assert_eq!(next(&mut rx).await, ("a", 1));

    assert!(registry.disable(&PluginId::from_static("a")));
    assert_eq!(registry.enabled(), Vec::<PluginId>::new());
    registry.notify::<Numbers>(2);
    // A second, still enabled Plugin shows notify has run by the time its event arrives.
    let forward_b = Forward("b", tx);
    registry
        .enable(plugin("b", move |r| {
            r.observe::<Numbers>(forward_b.clone());
        }))
        .unwrap();
    registry.notify::<Numbers>(3);
    assert_eq!(next(&mut rx).await, ("b", 3));
    assert_eq!(rx.try_recv(), Err(TryRecvError::Empty));
}

/// Intercept: a list of words about to be saved. Fails open.
struct Words;

impl InterceptPoint for Words {
    const NAME: &'static str = "words";
    const ON_FAILURE: OnFailure = OnFailure::Allow;
    type Action = Vec<&'static str>;
}

/// What an Interceptor at `P` returns.
type Decision<P> = Result<Verdict<<P as InterceptPoint>::Action>, PluginError>;

/// An Interceptor at `P` that decides with a plain function.
struct Decide<P: InterceptPoint>(fn(&P::Action) -> Decision<P>);

impl<P: InterceptPoint> Clone for Decide<P> {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

impl<P: InterceptPoint> Interceptor<P> for Decide<P> {
    fn intercept<'a>(&'a self, action: &'a P::Action) -> BoxFuture<'a, Decision<P>> {
        let verdict = (self.0)(action);
        Box::pin(async move { verdict })
    }
}

/// A Plugin with one Interceptor at `P`.
fn interceptor<P: InterceptPoint>(
    id: &'static str,
    priority: Priority,
    decide: fn(&P::Action) -> Decision<P>,
) -> Arc<dyn Plugin> {
    plugin(id, move |r| r.intercept::<P>(priority, Decide::<P>(decide)))
}

/// Appends `word` to the action.
fn append(action: &[&'static str], word: &'static str) -> Verdict<Vec<&'static str>> {
    let mut words = action.to_vec();
    words.push(word);
    Verdict::Modify(words)
}

#[tokio::test]
async fn interceptors_run_in_priority_order_then_plugin_id_and_see_earlier_modifications() {
    let registry = Registry::new();
    registry
        .enable(interceptor::<Words>("late", Priority::LATE, |a| {
            Ok(append(a, "late"))
        }))
        .unwrap();
    registry
        .enable(interceptor::<Words>("m", Priority::DEFAULT, |a| {
            Ok(append(a, "m"))
        }))
        .unwrap();
    registry
        .enable(interceptor::<Words>("a", Priority::DEFAULT, |a| {
            Ok(append(a, "a"))
        }))
        .unwrap();
    registry
        .enable(interceptor::<Words>("z", Priority::EARLY, |a| {
            Ok(append(a, "z"))
        }))
        .unwrap();
    registry
        .enable(interceptor::<Words>("pass", Priority(50), |_| {
            Ok(Verdict::Continue)
        }))
        .unwrap();

    let outcome = registry.intercept::<Words>(vec!["start"]).await;

    assert_eq!(
        outcome,
        Outcome::Proceed(vec!["start", "z", "a", "m", "late"])
    );
}

#[tokio::test]
async fn a_veto_stops_the_action_and_later_interceptors() {
    let registry = Registry::new();
    registry
        .enable(interceptor::<Words>("first", Priority::EARLY, |a| {
            Ok(append(a, "first"))
        }))
        .unwrap();
    registry
        .enable(interceptor::<Words>("guard", Priority::DEFAULT, |a| {
            if a.contains(&"forbidden") {
                Ok(Verdict::Veto("no forbidden words".to_owned()))
            } else {
                Ok(Verdict::Continue)
            }
        }))
        .unwrap();
    registry
        .enable(interceptor::<Words>("after", Priority::LATE, |_| {
            Err("must not run".into())
        }))
        .unwrap();

    let outcome = registry.intercept::<Words>(vec!["forbidden"]).await;

    assert_eq!(
        outcome,
        Outcome::Vetoed {
            by: PluginId::from_static("guard"),
            reason: "no forbidden words".to_owned(),
        }
    );
}

#[tokio::test]
async fn with_no_interceptors_the_action_proceeds_unchanged() {
    let outcome = Registry::new().intercept::<Words>(vec!["as", "is"]).await;

    assert_eq!(outcome, Outcome::Proceed(vec!["as", "is"]));
}

/// Intercept: a list of words about to be deleted. Fails closed, and gives up on an Interceptor
/// after one second.
struct Deletes;

impl InterceptPoint for Deletes {
    const NAME: &'static str = "deletes";
    const ON_FAILURE: OnFailure = OnFailure::Block;
    const TIMEOUT: Duration = Duration::from_secs(1);
    type Action = Vec<&'static str>;
}

/// An Interceptor that never decides.
struct Hang;

impl<P: InterceptPoint> Interceptor<P> for Hang {
    fn intercept<'a>(&'a self, _action: &'a P::Action) -> BoxFuture<'a, Decision<P>> {
        Box::pin(std::future::pending())
    }
}

#[tokio::test]
async fn at_a_fail_open_point_a_failing_interceptor_is_skipped_and_the_action_goes_through() {
    let registry = Registry::new();
    registry
        .enable(interceptor::<Words>("before", Priority::EARLY, |a| {
            Ok(append(a, "before"))
        }))
        .unwrap();
    registry
        .enable(interceptor::<Words>("broken", Priority::DEFAULT, |_| {
            Err("database is down".into())
        }))
        .unwrap();
    registry
        .enable(interceptor::<Words>("after", Priority::LATE, |a| {
            Ok(append(a, "after"))
        }))
        .unwrap();

    let outcome = registry.intercept::<Words>(vec![]).await;

    assert_eq!(outcome, Outcome::Proceed(vec!["before", "after"]));
    // Failing isn't panicking: the Plugin stays enabled.
    assert!(
        registry
            .enabled()
            .contains(&PluginId::from_static("broken"))
    );
}

#[tokio::test]
async fn at_a_fail_closed_point_a_failing_interceptor_blocks_the_action() {
    let registry = Registry::new();
    registry
        .enable(interceptor::<Deletes>("broken", Priority::DEFAULT, |_| {
            Err("database is down".into())
        }))
        .unwrap();
    registry
        .enable(interceptor::<Deletes>("after", Priority::LATE, |a| {
            Ok(append(a, "after"))
        }))
        .unwrap();

    let outcome = registry.intercept::<Deletes>(vec!["x"]).await;

    let Outcome::Failed { by, error } = outcome else {
        panic!("expected the action to be blocked, got {outcome:?}");
    };
    assert_eq!(by, PluginId::from_static("broken"));
    assert!(error.contains("database is down"), "{error}");
}

#[tokio::test(start_paused = true)]
async fn an_interceptor_that_takes_longer_than_the_points_timeout_has_failed() {
    let registry = Registry::new();
    registry
        .enable(plugin("slow", |r| {
            r.intercept::<Deletes>(Priority::DEFAULT, Hang);
        }))
        .unwrap();
    registry
        .enable(plugin("slow-too", |r| {
            r.intercept::<Words>(Priority::DEFAULT, Hang);
        }))
        .unwrap();

    let outcome = registry.intercept::<Deletes>(vec!["x"]).await;
    let Outcome::Failed { by, error } = outcome else {
        panic!("expected the action to be blocked, got {outcome:?}");
    };
    assert_eq!(by, PluginId::from_static("slow"));
    assert!(error.contains("timed out"), "{error}");

    // At a fail-open point the same lets the action through.
    assert_eq!(
        registry.intercept::<Words>(vec!["y"]).await,
        Outcome::Proceed(vec!["y"])
    );
}

/// An Observer that panics on every event.
struct PanicOnEvent;

impl Observer<Numbers> for PanicOnEvent {
    fn observe<'a>(&'a self, _event: &'a u32) -> BoxFuture<'a, ()> {
        Box::pin(async { panic!("observer bug") })
    }
}

/// An Interceptor that panics before it returns a future.
struct PanicAtOnce;

impl<P: InterceptPoint> Interceptor<P> for PanicAtOnce {
    fn intercept<'a>(&'a self, _action: &'a P::Action) -> BoxFuture<'a, Decision<P>> {
        panic!("interceptor bug")
    }
}

/// Waits until the Plugin `id` is no longer enabled.
async fn disabled(registry: &Registry, id: &'static str) {
    let id = PluginId::from_static(id);
    tokio::time::timeout(WAIT, async {
        while registry.enabled().contains(&id) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("plugin was never disabled");
}

#[tokio::test]
async fn a_panicking_interceptor_is_disabled_and_the_action_carries_on_without_it() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let registry = Registry::new();
    registry
        .enable(interceptor::<Deletes>("before", Priority::EARLY, |a| {
            Ok(append(a, "before"))
        }))
        .unwrap();
    let forward_buggy = Forward("buggy", tx.clone());
    registry
        .enable(plugin("buggy", move |r| {
            r.intercept::<Deletes>(
                Priority::DEFAULT,
                Decide::<Deletes>(|_| panic!("interceptor bug")),
            );
            r.observe::<Numbers>(forward_buggy.clone());
        }))
        .unwrap();
    registry
        .enable(plugin("buggy-at-once", |r| {
            r.intercept::<Deletes>(Priority::DEFAULT, PanicAtOnce);
        }))
        .unwrap();
    registry
        .enable(interceptor::<Deletes>("after", Priority::LATE, |a| {
            Ok(append(a, "after"))
        }))
        .unwrap();

    // Deletes fails closed, but a panic acts as if the Plugin weren't there rather than as a
    // failure.
    let outcome = registry.intercept::<Deletes>(vec![]).await;

    assert_eq!(outcome, Outcome::Proceed(vec!["before", "after"]));
    assert_eq!(
        registry.enabled(),
        [
            PluginId::from_static("after"),
            PluginId::from_static("before")
        ]
    );
    // Disabling removed the panicking Plugin's other registrations too.
    let forward_witness = Forward("witness", tx);
    registry
        .enable(plugin("witness", move |r| {
            r.observe::<Numbers>(forward_witness.clone());
        }))
        .unwrap();
    registry.notify::<Numbers>(7);
    assert_eq!(next(&mut rx).await, ("witness", 7));
    assert_eq!(rx.try_recv(), Err(TryRecvError::Empty));
}

#[tokio::test]
async fn a_panicking_observer_is_disabled_and_other_observers_still_hear_the_event() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let registry = Registry::new();
    registry
        .enable(plugin("buggy", |r| r.observe::<Numbers>(PanicOnEvent)))
        .unwrap();
    let forward = Forward("fine", tx);
    registry
        .enable(plugin("fine", move |r| {
            r.observe::<Numbers>(forward.clone());
        }))
        .unwrap();

    registry.notify::<Numbers>(1);

    assert_eq!(next(&mut rx).await, ("fine", 1));
    disabled(&registry, "buggy").await;
    assert_eq!(registry.enabled(), [PluginId::from_static("fine")]);
}

#[tokio::test]
async fn a_plugin_that_panics_while_registering_is_not_enabled() {
    let registry = Registry::new();

    let result = registry.enable(plugin("buggy", |r| {
        r.intercept::<Words>(
            Priority::DEFAULT,
            Decide::<Words>(|_| Ok(Verdict::Veto("half registered".to_owned()))),
        );
        panic!("register bug");
    }));

    assert!(
        matches!(result, Err(EnableError::Panicked { .. })),
        "{result:?}"
    );
    assert_eq!(registry.enabled(), Vec::<PluginId>::new());
    assert_eq!(
        registry.intercept::<Words>(vec!["ok"]).await,
        Outcome::Proceed(vec!["ok"])
    );
}

#[tokio::test]
async fn a_plugin_id_can_only_be_enabled_once() {
    let registry = Registry::new();
    registry.enable(plugin("twice", |_| {})).unwrap();

    let result = registry.enable(plugin("twice", |_| {}));

    assert!(
        matches!(result, Err(EnableError::AlreadyEnabled(_))),
        "{result:?}"
    );
}

/// A capability: says hello.
trait Greeter: Send + Sync {
    fn greet(&self) -> &'static str;
}

struct Greeting(&'static str);

impl Greeter for Greeting {
    fn greet(&self) -> &'static str {
        self.0
    }
}

struct PanickingGreeter;

impl Greeter for PanickingGreeter {
    fn greet(&self) -> &'static str {
        panic!("provider bug")
    }
}

/// A Plugin that provides `greeter` at `priority`.
fn greeter(
    id: &'static str,
    priority: Priority,
    greeter: impl Greeter + 'static,
) -> Arc<dyn Plugin> {
    let greeter: Arc<dyn Greeter> = Arc::new(greeter);
    plugin(id, move |r| {
        r.provide::<dyn Greeter>(priority, Arc::clone(&greeter));
    })
}

async fn greet(registry: &Registry) -> Option<&'static str> {
    registry
        .call_provider::<dyn Greeter, _, _>(|greeter| async move { greeter.greet() })
        .await
}

#[tokio::test]
async fn providers_are_looked_up_by_trait_in_priority_order_unless_the_admin_sets_precedence() {
    let registry = Registry::new();
    assert_eq!(greet(&registry).await, None);
    registry
        .enable(greeter("en", Priority::DEFAULT, Greeting("hello")))
        .unwrap();
    registry
        .enable(greeter("fr", Priority::EARLY, Greeting("bonjour")))
        .unwrap();
    registry
        .enable(greeter("de", Priority::LATE, Greeting("hallo")))
        .unwrap();

    assert_eq!(greet(&registry).await, Some("bonjour"));

    registry.set_provider_precedence::<dyn Greeter>(vec![
        PluginId::from_static("not-installed"),
        PluginId::from_static("de"),
    ]);
    assert_eq!(greet(&registry).await, Some("hallo"));

    // Plugins the admin didn't rank follow in priority order.
    registry.disable(&PluginId::from_static("de"));
    assert_eq!(greet(&registry).await, Some("bonjour"));

    // The admin's choice outlasts the Plugin being disabled and enabled again.
    registry
        .enable(greeter("de", Priority::LATE, Greeting("hallo")))
        .unwrap();
    assert_eq!(greet(&registry).await, Some("hallo"));
}

#[tokio::test]
async fn a_panicking_provider_is_disabled_and_the_next_provider_answers() {
    let registry = Registry::new();
    registry
        .enable(greeter("buggy", Priority::EARLY, PanickingGreeter))
        .unwrap();
    registry
        .enable(greeter("en", Priority::DEFAULT, Greeting("hello")))
        .unwrap();

    assert_eq!(greet(&registry).await, Some("hello"));
    assert_eq!(registry.enabled(), [PluginId::from_static("en")]);
}
