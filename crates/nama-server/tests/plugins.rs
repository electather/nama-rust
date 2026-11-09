//! The plugin registry: enabling and disabling Plugins and calling them at Extension Points.
#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test helpers and test Plugins panic to fail a test or to test panic containment"
)]

use std::{collections::BTreeMap, sync::Arc, time::Duration};

use nama_plugin_api::{
    BoxFuture, InterceptPoint, Interceptor, Manifest, ObservePoint, Observer, OnFailure, Plugin,
    PluginError, PluginId, PointKey, Priority, Registrar, Verdict,
    settings::{SettingField, SettingKind, SettingValue, Settings, SettingsSchema},
};
use nama_plugin_test_choices as test_choices;
use nama_server::plugins::{CheckError, EnableError, Outcome, Registry, SETTINGS_CHECK_TIMEOUT};
use tokio::sync::mpsc::{self, error::TryRecvError};

const WAIT: Duration = Duration::from_secs(5);

/// A test Plugin whose `register` is the given closure.
struct TestPlugin<F> {
    id: &'static str,
    extension_points: Vec<PointKey>,
    register: F,
}

impl<F: Fn(&mut Registrar) + Send + Sync + 'static> Plugin for TestPlugin<F> {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: PluginId::from_static(self.id),
            version: "0.0.0".to_owned(),
            settings: SettingsSchema::default(),
            extension_points: self.extension_points.clone(),
        }
    }

    fn register(&self, registrar: &mut Registrar) {
        (self.register)(registrar);
    }
}

/// A test Plugin whose Manifest names `extension_points` and whose `register` is `register`.
fn plugin(
    id: &'static str,
    extension_points: &[PointKey],
    register: impl Fn(&mut Registrar) + Send + Sync + 'static,
) -> Arc<dyn Plugin> {
    Arc::new(TestPlugin {
        id,
        extension_points: extension_points.to_vec(),
        register,
    })
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
        .enable(plugin("a", &[PointKey::observe::<Numbers>()], move |r| {
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
        .enable(plugin("b", &[PointKey::observe::<Numbers>()], move |r| {
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
    plugin(id, &[PointKey::intercept::<P>()], move |r| {
        r.intercept::<P>(priority, Decide::<P>(decide));
    })
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
        .enable(plugin("slow", &[PointKey::intercept::<Deletes>()], |r| {
            r.intercept::<Deletes>(Priority::DEFAULT, Hang);
        }))
        .unwrap();
    registry
        .enable(plugin("slow-too", &[PointKey::intercept::<Words>()], |r| {
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
        .enable(plugin(
            "buggy",
            &[
                PointKey::intercept::<Deletes>(),
                PointKey::observe::<Numbers>(),
            ],
            move |r| {
                r.intercept::<Deletes>(
                    Priority::DEFAULT,
                    Decide::<Deletes>(|_| panic!("interceptor bug")),
                );
                r.observe::<Numbers>(forward_buggy.clone());
            },
        ))
        .unwrap();
    registry
        .enable(plugin(
            "buggy-at-once",
            &[PointKey::intercept::<Deletes>()],
            |r| {
                r.intercept::<Deletes>(Priority::DEFAULT, PanicAtOnce);
            },
        ))
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
        .enable(plugin(
            "witness",
            &[PointKey::observe::<Numbers>()],
            move |r| {
                r.observe::<Numbers>(forward_witness.clone());
            },
        ))
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
        .enable(plugin("buggy", &[PointKey::observe::<Numbers>()], |r| {
            r.observe::<Numbers>(PanicOnEvent);
        }))
        .unwrap();
    let forward = Forward("fine", tx);
    registry
        .enable(plugin(
            "fine",
            &[PointKey::observe::<Numbers>()],
            move |r| {
                r.observe::<Numbers>(forward.clone());
            },
        ))
        .unwrap();

    registry.notify::<Numbers>(1);

    assert_eq!(next(&mut rx).await, ("fine", 1));
    disabled(&registry, "buggy").await;
    assert_eq!(registry.enabled(), [PluginId::from_static("fine")]);
}

#[tokio::test]
async fn a_plugin_that_panics_while_registering_is_not_enabled() {
    let registry = Registry::new();

    let result = registry.enable(plugin("buggy", &[PointKey::intercept::<Words>()], |r| {
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

struct PanicsInManifest;

impl Plugin for PanicsInManifest {
    fn manifest(&self) -> Manifest {
        panic!("manifest bug")
    }

    fn register(&self, _registrar: &mut Registrar) {}
}

#[tokio::test]
async fn a_plugin_that_panics_giving_its_manifest_is_not_enabled() {
    let registry = Registry::new();

    let result = registry.enable(Arc::new(PanicsInManifest));

    assert!(
        matches!(result, Err(EnableError::ManifestPanicked { .. })),
        "{result:?}"
    );
    assert_eq!(registry.enabled(), Vec::<PluginId>::new());
}

/// An Interceptor at [`Words`] that panics when dropped.
struct PanicOnDrop;

impl Interceptor<Words> for PanicOnDrop {
    fn intercept<'a>(&'a self, _action: &'a Vec<&'static str>) -> BoxFuture<'a, Decision<Words>> {
        Box::pin(async { Ok(Verdict::Continue) })
    }
}

impl Drop for PanicOnDrop {
    fn drop(&mut self) {
        panic!("drop bug");
    }
}

#[test]
fn a_plugin_that_panics_while_being_dropped_on_disable_is_still_disabled() {
    let registry = Registry::new();
    registry
        .enable(plugin("buggy", &[PointKey::intercept::<Words>()], |r| {
            r.intercept::<Words>(Priority::DEFAULT, PanicOnDrop);
        }))
        .unwrap();

    assert!(registry.disable(&PluginId::from_static("buggy")));
    assert_eq!(registry.enabled(), Vec::<PluginId>::new());
}

#[tokio::test]
async fn a_plugin_id_can_only_be_enabled_once() {
    let registry = Registry::new();
    registry.enable(plugin("twice", &[], |_| {})).unwrap();

    let result = registry.enable(plugin("twice", &[], |_| {}));

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
    plugin(id, &[PointKey::provide::<dyn Greeter>()], move |r| {
        r.provide::<dyn Greeter>(priority, Arc::clone(&greeter));
    })
}

async fn greet(registry: &Registry) -> Option<&'static str> {
    registry
        .call_provider::<dyn Greeter, _, _>(|greeter| async move { greeter.greet() })
        .await
}

#[tokio::test]
async fn a_plugin_registering_at_an_extension_point_its_manifest_does_not_name_is_not_enabled() {
    let registry = Registry::new();

    let result = registry.enable(plugin(
        "undeclared",
        &[PointKey::observe::<Numbers>()],
        |r| r.provide::<dyn Greeter>(Priority::DEFAULT, Arc::new(Greeting("hello"))),
    ));

    let Err(EnableError::Refused { reason, .. }) = result else {
        panic!("{result:?}");
    };
    assert!(reason.contains("Greeter"), "{reason}");
    assert_eq!(registry.enabled(), []);
    assert_eq!(greet(&registry).await, None);
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

#[tokio::test]
async fn an_observer_whose_plugin_is_disabled_before_the_event_reaches_it_is_not_told() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let registry = Registry::new();
    let forward_gone = Forward("gone", tx.clone());
    registry
        .enable(plugin(
            "gone",
            &[PointKey::observe::<Numbers>()],
            move |r| {
                r.observe::<Numbers>(forward_gone.clone());
            },
        ))
        .unwrap();

    // The test runtime has one thread, so the delivery task can't run before the disable.
    registry.notify::<Numbers>(1);
    registry.disable(&PluginId::from_static("gone"));

    let forward_witness = Forward("witness", tx);
    registry
        .enable(plugin(
            "witness",
            &[PointKey::observe::<Numbers>()],
            move |r| {
                r.observe::<Numbers>(forward_witness.clone());
            },
        ))
        .unwrap();
    registry.notify::<Numbers>(2);
    assert_eq!(next(&mut rx).await, ("witness", 2));
    assert_eq!(rx.try_recv(), Err(TryRecvError::Empty));
}

/// An Interceptor that disables its own Plugin, enables a fresh copy of it, then panics.
struct ReenableThenPanic(Registry);

impl Interceptor<Words> for ReenableThenPanic {
    fn intercept<'a>(&'a self, _action: &'a Vec<&'static str>) -> BoxFuture<'a, Decision<Words>> {
        self.0.disable(&PluginId::from_static("flaky"));
        self.0
            .enable(interceptor::<Words>("flaky", Priority::DEFAULT, |a| {
                Ok(append(a, "fresh"))
            }))
            .expect("the fresh copy enables");
        panic!("stale interceptor bug")
    }
}

#[tokio::test]
async fn a_panic_from_a_stale_call_does_not_disable_the_plugin_enabled_since() {
    let registry = Registry::new();
    let handle = registry.clone();
    registry
        .enable(plugin(
            "flaky",
            &[PointKey::intercept::<Words>()],
            move |r| {
                r.intercept::<Words>(Priority::DEFAULT, ReenableThenPanic(handle.clone()));
            },
        ))
        .unwrap();

    let outcome = registry.intercept::<Words>(vec![]).await;

    assert_eq!(outcome, Outcome::Proceed(vec![]));
    assert_eq!(registry.enabled(), [PluginId::from_static("flaky")]);
    assert_eq!(
        registry.intercept::<Words>(vec![]).await,
        Outcome::Proceed(vec!["fresh"])
    );
}

/// A Provider whose every call fails.
trait Fetcher: Send + Sync {
    fn fetch(&self) -> Result<(), String>;
}

/// Fails quoting the URL it fetched, which holds an API key.
struct QuotesItsUrl(String);

impl Fetcher for QuotesItsUrl {
    fn fetch(&self) -> Result<(), String> {
        Err(format!("GET {} failed", self.0))
    }
}

/// A Plugin taking a required `api_key` and a `language` that defaults to `en`, which tells the
/// test the settings it was enabled with and provides a [`Fetcher`] that quotes them.
struct Configurable(mpsc::UnboundedSender<Settings>);

impl Plugin for Configurable {
    fn manifest(&self) -> Manifest {
        let field = |key: &str, kind, required, default| SettingField {
            key: key.to_owned(),
            label: key.to_owned(),
            description: String::new(),
            kind,
            required,
            default,
        };
        let settings = SettingsSchema {
            fields: vec![
                field("api_key", SettingKind::Secret, true, None),
                field(
                    "language",
                    SettingKind::Text,
                    false,
                    Some(SettingValue::Text("en".to_owned())),
                ),
            ],
        };
        Manifest {
            id: PluginId::from_static("configurable"),
            version: "0.0.0".to_owned(),
            settings,
            extension_points: vec![PointKey::provide::<dyn Fetcher>()],
        }
    }

    fn register(&self, registrar: &mut Registrar) {
        let settings = registrar.settings().clone();
        let url = format!(
            "/movies?key={}&language={}",
            settings.text("api_key").unwrap_or_default(),
            settings.text("language").unwrap_or_default()
        );
        registrar.provide::<dyn Fetcher>(Priority::DEFAULT, Arc::new(QuotesItsUrl(url)));
        let _: Result<(), _> = self.0.send(settings);
    }
}

fn settings(values: &[(&str, SettingValue)]) -> Settings {
    Settings::new(
        values
            .iter()
            .map(|(key, value)| ((*key).to_owned(), value.clone()))
            .collect(),
    )
}

#[tokio::test]
async fn a_plugin_is_enabled_with_its_settings_and_their_defaults() {
    let registry = Registry::new();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let key = SettingValue::Text("secret".to_owned());

    registry
        .enable_with_settings(
            Arc::new(Configurable(tx)),
            settings(&[("api_key", key.clone())]),
        )
        .expect("enable");

    assert_eq!(
        next(&mut rx).await,
        settings(&[
            ("api_key", key),
            ("language", SettingValue::Text("en".to_owned()))
        ])
    );
}

#[tokio::test]
async fn a_providers_error_for_users_comes_without_its_plugins_secret_settings() {
    let registry = Registry::new();
    let (tx, _rx) = mpsc::unbounded_channel();
    registry
        .enable_with_settings(
            Arc::new(Configurable(tx)),
            settings(&[("api_key", SettingValue::Text("s3cr3t".to_owned()))]),
        )
        .unwrap();

    let fetched = registry
        .call_provider_blanking_secrets::<dyn Fetcher, _, _>(
            |fetcher| async move { fetcher.fetch() },
        )
        .await;

    assert_eq!(
        fetched,
        Some(Err("GET /movies?key=…&language=en failed".to_owned()))
    );
}

#[test]
fn a_plugin_whose_settings_do_not_fit_its_schema_is_not_enabled() {
    let registry = Registry::new();
    let (tx, _rx) = mpsc::unbounded_channel();

    let missing =
        registry.enable_with_settings(Arc::new(Configurable(tx.clone())), Settings::default());
    let wrong_kind = registry.enable_with_settings(
        Arc::new(Configurable(tx)),
        settings(&[("api_key", SettingValue::Integer(1))]),
    );

    assert!(
        matches!(missing, Err(EnableError::InvalidSettings { .. })),
        "{missing:?}"
    );
    assert!(
        matches!(wrong_kind, Err(EnableError::InvalidSettings { .. })),
        "{wrong_kind:?}"
    );
    assert_eq!(registry.enabled(), Vec::<PluginId>::new());
}

#[tokio::test]
async fn a_caller_can_learn_which_plugin_provided_and_ask_that_plugin_again() {
    let registry = Registry::new();
    registry
        .enable(greeter("en", Priority::DEFAULT, Greeting("hello")))
        .unwrap();
    registry
        .enable(greeter("fr", Priority::EARLY, Greeting("bonjour")))
        .unwrap();
    let en = PluginId::from_static("en");

    let first = registry
        .call_provider_with_id::<dyn Greeter, _, _>(|greeter| async move { greeter.greet() })
        .await;
    let from_en = registry
        .call_provider_of::<dyn Greeter, _, _>(&en, |greeter| async move { greeter.greet() })
        .await;
    registry.disable(&en);
    let from_disabled = registry
        .call_provider_of::<dyn Greeter, _, _>(&en, |greeter| async move { greeter.greet() })
        .await;

    assert_eq!(first, Some((PluginId::from_static("fr"), "bonjour")));
    assert_eq!(from_en, Some("hello"));
    assert_eq!(from_disabled, None);
}

#[tokio::test(start_paused = true)]
async fn a_settings_check_that_runs_past_its_timeout_has_failed_and_harms_nothing() {
    let registry = Registry::with_builtins(&BTreeMap::new());
    let id = PluginId::from_static(test_choices::ID);
    let settings = |fail_with: Option<&str>| {
        let mut settings = BTreeMap::from([(
            "api_key".to_owned(),
            SettingValue::Text(test_choices::API_KEY.to_owned()),
        )]);
        if let Some(fail_with) = fail_with {
            settings.insert(
                "fail_with".to_owned(),
                SettingValue::Text(fail_with.to_owned()),
            );
        }
        Settings::new(settings)
    };

    let hung = registry
        .check_settings(&id, settings(Some(test_choices::HANG)))
        .await;

    let Err(CheckError::TimedOut(timed_out, after)) = hung else {
        panic!("expected the check to time out, got {hung:?}");
    };
    assert_eq!(timed_out, id);
    assert_eq!(after, SETTINGS_CHECK_TIMEOUT);
    let checked = registry.check_settings(&id, settings(None)).await;
    assert!(checked.is_ok(), "{checked:?}");
}
