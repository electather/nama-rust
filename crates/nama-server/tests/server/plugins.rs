//! The plugin registry: enabling and disabling Plugins and calling them at Extension Points.
#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test helpers and test Plugins panic to fail a test or to test panic containment"
)]

use crate::support::log;

use std::{collections::BTreeMap, error::Error, fmt, sync::Arc, time::Duration};

use log::Log;
use nama_plugin_api::{
    BoxFuture, Manifest, ObservePoint, Observer, Plugin, PluginError, PluginId, PointKey, Priority,
    Registrar,
    fulfiller::{Fulfiller, Movies},
    settings::{SettingField, SettingKind, SettingValue, Settings, SettingsSchema},
};
use nama_plugin_test_choices as test_choices;
use nama_server::plugins::{
    Answer, CheckError, EnableError, Failure, Registry, SETTINGS_CHECK_TIMEOUT,
};
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
            version: "0.0.0".to_owned(),
            extension_points: self.extension_points.clone(),
            ..Manifest::new(PluginId::from_static(self.id))
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        (self.register)(registrar);
        Ok(())
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
    fn observe<'a>(&'a self, event: &'a u32) -> BoxFuture<'a, Result<(), PluginError>> {
        Box::pin(async move {
            let _: Result<(), _> = self.1.send((self.0, *event));
            Ok(())
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

    assert!(registry.disable(&PluginId::from_static("a")).await);
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

/// An Observer that panics on every event.
struct PanicOnEvent;

impl Observer<Numbers> for PanicOnEvent {
    fn observe<'a>(&'a self, _event: &'a u32) -> BoxFuture<'a, Result<(), PluginError>> {
        Box::pin(async { panic!("observer bug") })
    }
}

/// An Observer that panics before it returns a future.
struct PanicAtOnce;

impl Observer<Numbers> for PanicAtOnce {
    fn observe<'a>(&'a self, _event: &'a u32) -> BoxFuture<'a, Result<(), PluginError>> {
        panic!("observer bug")
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
async fn a_handler_that_panics_before_returning_a_future_disables_its_plugin_and_all_it_registered()
{
    let (tx, mut rx) = mpsc::unbounded_channel();
    let registry = Registry::new();
    registry
        .enable(plugin(
            "buggy",
            &[
                PointKey::observe::<Numbers>(),
                PointKey::provide::<dyn Greeter>(),
            ],
            |r| {
                r.observe::<Numbers>(PanicAtOnce);
                r.provide::<dyn Greeter>(Priority::DEFAULT, Arc::new(Greeting("hello")));
            },
        ))
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
    // Disabling removed the panicking Plugin's Provider too.
    assert_eq!(greet(&registry).await, None);
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

    let result = registry.enable(plugin(
        "buggy",
        &[PointKey::provide::<dyn Greeter>()],
        |r| {
            r.provide::<dyn Greeter>(Priority::DEFAULT, Arc::new(Greeting("half registered")));
            panic!("register bug");
        },
    ));

    assert!(
        matches!(result, Err(EnableError::Panicked { .. })),
        "{result:?}"
    );
    assert_eq!(registry.enabled(), Vec::<PluginId>::new());
    assert_eq!(greet(&registry).await, None);
}

struct PanicsInManifest;

impl Plugin for PanicsInManifest {
    fn manifest(&self) -> Manifest {
        panic!("manifest bug")
    }

    fn register(&self, _registrar: &mut Registrar) -> Result<(), PluginError> {
        Ok(())
    }
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

/// An Observer at [`Numbers`] that panics when dropped.
struct PanicOnDrop;

impl Observer<Numbers> for PanicOnDrop {
    fn observe<'a>(&'a self, _event: &'a u32) -> BoxFuture<'a, Result<(), PluginError>> {
        Box::pin(async { Ok(()) })
    }
}

impl Drop for PanicOnDrop {
    fn drop(&mut self) {
        panic!("drop bug");
    }
}

#[tokio::test]
async fn a_plugin_that_panics_while_being_dropped_on_disable_is_still_disabled() {
    let registry = Registry::new();
    registry
        .enable(plugin("buggy", &[PointKey::observe::<Numbers>()], |r| {
            r.observe::<Numbers>(PanicOnDrop);
        }))
        .unwrap();

    assert!(registry.disable(&PluginId::from_static("buggy")).await);
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

async fn greeting(greeter: Arc<dyn Greeter>) -> Result<&'static str, PluginError> {
    Ok(greeter.greet())
}

async fn greet(registry: &Registry) -> Option<&'static str> {
    registry.ask_first(greeting).await?.result.ok()
}

/// The Plugin `plugin`'s Greeter answering `greeting`.
fn greeted(plugin: &'static str, greeting: &'static str) -> Answer<&'static str> {
    Answer {
        plugin: PluginId::from_static(plugin),
        result: Ok(greeting),
    }
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
    registry.disable(&PluginId::from_static("de")).await;
    assert_eq!(greet(&registry).await, Some("bonjour"));

    // The admin's choice outlasts the Plugin being disabled and enabled again.
    registry
        .enable(greeter("de", Priority::LATE, Greeting("hallo")))
        .unwrap();
    assert_eq!(greet(&registry).await, Some("hallo"));
}

#[tokio::test]
async fn providers_of_equal_priority_answer_in_plugin_id_order() {
    let registry = Registry::new();
    registry
        .enable(greeter("m", Priority::DEFAULT, Greeting("from m")))
        .unwrap();
    registry
        .enable(greeter("a", Priority::DEFAULT, Greeting("from a")))
        .unwrap();

    assert_eq!(greet(&registry).await, Some("from a"));
}

#[tokio::test]
async fn a_panicking_provider_is_disabled_and_the_answer_names_the_next_plugin_which_answered() {
    let registry = Registry::new();
    registry
        .enable(greeter("buggy", Priority::EARLY, PanickingGreeter))
        .unwrap();
    registry
        .enable(greeter("en", Priority::DEFAULT, Greeting("hello")))
        .unwrap();

    let answer = registry.ask_first(greeting).await;

    assert_eq!(answer, Some(greeted("en", "hello")));
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
    registry.disable(&PluginId::from_static("gone")).await;

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

/// A Greeter that disables its own Plugin, enables a fresh copy of it, then panics.
struct ReenableThenPanic(Registry);

impl Greeter for ReenableThenPanic {
    fn greet(&self) -> &'static str {
        self.0.disable_now(&PluginId::from_static("flaky"));
        self.0
            .enable(greeter("flaky", Priority::DEFAULT, Greeting("fresh")))
            .expect("the fresh copy enables");
        panic!("stale provider bug")
    }
}

#[tokio::test]
async fn a_panic_from_a_stale_call_does_not_disable_the_plugin_enabled_since() {
    let registry = Registry::new();
    let handle = registry.clone();
    registry
        .enable(plugin(
            "flaky",
            &[PointKey::provide::<dyn Greeter>()],
            move |r| {
                r.provide::<dyn Greeter>(
                    Priority::DEFAULT,
                    Arc::new(ReenableThenPanic(handle.clone())),
                );
            },
        ))
        .unwrap();

    greet(&registry).await;

    assert_eq!(registry.enabled(), [PluginId::from_static("flaky")]);
    assert_eq!(greet(&registry).await, Some("fresh"));
}

/// A Provider whose every call fails.
trait Fetcher: Send + Sync {
    fn fetch(&self) -> Result<(), PluginError>;
}

async fn fetch(fetcher: Arc<dyn Fetcher>) -> Result<(), PluginError> {
    fetcher.fetch()
}

/// Fails quoting the URL it fetched, which holds an API key.
struct QuotesItsUrl(String);

impl Fetcher for QuotesItsUrl {
    fn fetch(&self) -> Result<(), PluginError> {
        Err(format!("GET {} failed", self.0).into())
    }
}

/// Fails with the error `.0` makes.
struct FailsWith(fn() -> PluginError);

impl Fetcher for FailsWith {
    fn fetch(&self) -> Result<(), PluginError> {
        Err(self.0())
    }
}

/// A Plugin whose Fetcher fails with the error `error` makes.
fn failing(id: &'static str, error: fn() -> PluginError) -> Arc<dyn Plugin> {
    plugin(id, &[PointKey::provide::<dyn Fetcher>()], move |r| {
        r.provide::<dyn Fetcher>(Priority::DEFAULT, Arc::new(FailsWith(error)));
    })
}

/// An error that panics when shown.
#[derive(Debug)]
struct PanicsWhenShown;

impl fmt::Display for PanicsWhenShown {
    fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
        panic!("display bug")
    }
}

impl Error for PanicsWhenShown {}

/// An error that panics when dropped.
#[derive(Debug)]
struct PanicsWhenDropped;

impl fmt::Display for PanicsWhenDropped {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("fine until dropped")
    }
}

impl Drop for PanicsWhenDropped {
    fn drop(&mut self) {
        panic!("drop bug")
    }
}

impl Error for PanicsWhenDropped {}

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
            min: None,
            max: None,
            choices: Vec::new(),
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
            version: "0.0.0".to_owned(),
            settings,
            extension_points: vec![PointKey::provide::<dyn Fetcher>()],
            ..Manifest::new(PluginId::from_static("configurable"))
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        let settings = registrar.settings().clone();
        let url = format!(
            "/movies?key={}&language={}",
            settings.text("api_key").unwrap_or_default(),
            settings.text("language").unwrap_or_default()
        );
        registrar.provide::<dyn Fetcher>(Priority::DEFAULT, Arc::new(QuotesItsUrl(url)));
        let _: Result<(), _> = self.0.send(settings);
        Ok(())
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
async fn a_providers_failure_comes_without_its_plugins_secret_settings_from_every_call() {
    let log = Log::global();
    let registry = Registry::new();
    let (tx, _rx) = mpsc::unbounded_channel();
    registry
        .enable_with_settings(
            Arc::new(Configurable(tx)),
            settings(&[("api_key", SettingValue::Text("s3cr3t".to_owned()))]),
        )
        .unwrap();
    let blanked = "GET /movies?key=…&language=en failed";

    let first = registry.ask_first(fetch).await.unwrap();
    let named = registry
        .ask(&PluginId::from_static("configurable"), fetch)
        .await
        .unwrap();
    registry.tell_each(|_, fetcher: Arc<dyn Fetcher>| fetch(fetcher));

    let failed = Err(Failure {
        kind: (),
        message: blanked.to_owned(),
    });
    assert_eq!((first.result, named.result), (failed.clone(), failed));
    log.wait_for(&["configurable", "provider failed", blanked], 1)
        .await;
    assert!(!log.contents().contains("s3cr3t"), "{}", log.contents());
    let recorded = (registry.extension_points()).failing(
        &PluginId::from_static("configurable"),
        PointKey::provide::<dyn Fetcher>(),
    );
    assert_eq!(
        recorded.map(|failing| failing.last_error),
        Some(blanked.to_owned())
    );
}

#[tokio::test]
async fn a_provider_error_that_panics_when_shown_or_dropped_is_contained_and_disables_its_plugin() {
    let registry = Registry::new();
    registry
        .enable(failing("shown", || Box::new(PanicsWhenShown)))
        .unwrap();
    registry
        .enable(failing("dropped", || Box::new(PanicsWhenDropped)))
        .unwrap();

    let answer = registry.ask_first(fetch).await;

    assert_eq!(answer, None);
    assert_eq!(registry.enabled(), []);
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
async fn an_answer_names_the_plugin_that_gave_it_and_that_plugin_can_be_asked_again() {
    let registry = Registry::new();
    registry
        .enable(greeter("en", Priority::DEFAULT, Greeting("hello")))
        .unwrap();
    registry
        .enable(greeter("fr", Priority::EARLY, Greeting("bonjour")))
        .unwrap();
    let en = PluginId::from_static("en");

    let first = registry.ask_first(greeting).await;
    let from_en = registry.ask(&en, greeting).await;
    registry.disable(&en).await;
    let from_disabled = registry.ask(&en, greeting).await;

    assert_eq!(first, Some(greeted("fr", "bonjour")));
    assert_eq!(from_en, Some(greeted("en", "hello")));
    assert_eq!(from_disabled, None);
}

#[tokio::test]
async fn the_plugins_providing_a_capability_are_the_enabled_ones_that_provide_it() {
    let registry = Registry::new();
    registry
        .enable(greeter("en", Priority::DEFAULT, Greeting("hello")))
        .unwrap();
    registry
        .enable(greeter("fr", Priority::EARLY, Greeting("bonjour")))
        .unwrap();
    registry
        .enable(failing("fetcher", || "unused".into()))
        .unwrap();
    registry.disable(&PluginId::from_static("fr")).await;

    assert_eq!(
        registry.providers::<dyn Greeter>(),
        [PluginId::from_static("en")].into()
    );
}

#[tokio::test(start_paused = true)]
async fn a_settings_check_that_runs_past_its_timeout_has_failed_and_harms_nothing() {
    let registry = Registry::with_builtins();
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

#[test]
fn a_plugin_that_refuses_to_start_is_not_enabled_and_leaves_nothing_registered() {
    let registry = Registry::with_builtins();
    let id = PluginId::from_static(test_choices::ID);
    let settings = Settings::new(BTreeMap::from([
        (
            "api_key".to_owned(),
            SettingValue::Text(test_choices::API_KEY.to_owned()),
        ),
        ("profile".to_owned(), SettingValue::Text("1".to_owned())),
        (
            "fail_with".to_owned(),
            SettingValue::Text(test_choices::REFUSE.to_owned()),
        ),
    ]));

    let result = registry.enable_by_id(&id, settings);

    let Err(EnableError::RefusedToStart { plugin, reason }) = result else {
        panic!("expected the Plugin to refuse to start, got {result:?}");
    };
    assert_eq!(plugin, id);
    assert_eq!(reason, "the key … was turned down");
    assert_eq!(registry.enabled(), []);
    assert!(registry.providers::<dyn Fulfiller<Movies>>().is_empty());
}

/// A Built-in Plugin whose one Personal setting, `field`, is of the given kind.
struct PersonalKind(&'static str, SettingKind);

impl Plugin for PersonalKind {
    fn manifest(&self) -> Manifest {
        Manifest {
            version: "0.0.0".to_owned(),
            personal_settings: SettingsSchema {
                fields: vec![SettingField {
                    key: "field".to_owned(),
                    label: "Field".to_owned(),
                    description: String::new(),
                    kind: self.1,
                    required: false,
                    default: None,
                    min: None,
                    max: None,
                    choices: Vec::new(),
                }],
            },
            ..Manifest::new(PluginId::from_static(self.0))
        }
    }

    fn register(&self, _registrar: &mut Registrar) -> Result<(), PluginError> {
        Ok(())
    }
}

#[tokio::test]
async fn a_built_in_with_a_url_or_choice_personal_setting_is_refused_at_enable() {
    let registry = Registry::new();
    for (id, kind) in [("url", SettingKind::Url), ("choice", SettingKind::Choice)] {
        let result = registry.enable(Arc::new(PersonalKind(id, kind)));

        let Err(EnableError::Refused { plugin, reason }) = result else {
            panic!("expected {id} to be refused, got {result:?}");
        };
        assert_eq!(plugin, PluginId::from_static(id));
        assert!(
            reason.starts_with("its Personal setting field is"),
            "{reason}"
        );
        assert_eq!(registry.enabled(), []);
    }
}
