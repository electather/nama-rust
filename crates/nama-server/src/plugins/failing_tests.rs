use std::{collections::BTreeMap, error::Error, sync::Arc, time::Duration};

use nama_plugin_api::{
    Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    fulfiller::TryAgainIn,
    recogniser::Recogniser,
    settings::{SettingField, SettingKind, SettingValue, Settings, SettingsSchema},
};

use crate::plugins::{ExtensionPoints, Failing, FailureKind, ProviderError, Registry, Retryable};

const PLUGIN: PluginId = PluginId::from_static("flaky");

fn recogniser() -> PointKey {
    PointKey::provide::<dyn Recogniser>()
}

/// A Registry with [`Idle`] enabled, and its Extension Points.
fn enabled() -> (Registry, ExtensionPoints) {
    let plugins = Registry::new();
    plugins.enable(Arc::new(Idle)).unwrap();
    let points = plugins.extension_points();
    (plugins, points)
}

#[test]
fn a_failure_is_recorded_and_counted_in_a_row() {
    let (_plugins, points) = enabled();

    let first = points
        .provider_failed(&PLUGIN, recogniser(), "timed out")
        .unwrap();
    let second = points
        .provider_failed(&PLUGIN, recogniser(), "refused")
        .unwrap();

    assert_eq!((first.in_a_row, second.in_a_row), (1, 2));
    let failing = points.failing(&PLUGIN, recogniser()).unwrap();
    assert_eq!(
        (failing.last_error.as_str(), failing.in_a_row),
        ("refused", 2)
    );
    assert!(failing.at_ms > 0);
}

#[test]
fn a_success_clears_the_failure_and_ends_the_run() {
    let (_plugins, points) = enabled();
    points.provider_failed(&PLUGIN, recogniser(), "timed out");

    points.provider_answered(&PLUGIN, recogniser());

    assert_eq!(points.failing(&PLUGIN, recogniser()), None);
    let again = points
        .provider_failed(&PLUGIN, recogniser(), "timed out")
        .unwrap();
    assert_eq!(again.in_a_row, 1);
}

#[test]
fn failures_are_kept_apart_by_plugin_and_point() {
    let (_plugins, points) = enabled();
    let other = PluginId::from_static("other");

    points.provider_failed(&PLUGIN, recogniser(), "timed out");

    assert_eq!(points.failing(&other, recogniser()), None);
    let observer = PointKey::provide::<dyn Plugin>();
    assert_eq!(points.failing(&PLUGIN, observer), None);
}

/// A Plugin that registers nothing.
struct Idle;

impl Plugin for Idle {
    fn manifest(&self) -> Manifest {
        Manifest {
            version: "0.0.0".to_owned(),
            ..Manifest::new(PLUGIN)
        }
    }

    fn register(&self, _registrar: &mut Registrar) -> Result<(), PluginError> {
        Ok(())
    }
}

/// A Provider whose call alone decides whether it fails.
trait Told: Send + Sync {}

/// A Plugin that provides a [`Told`], with a secret `api_key` setting.
struct Listens;

impl Told for Listens {}

impl Plugin for Listens {
    fn manifest(&self) -> Manifest {
        let api_key = SettingField {
            key: "api_key".to_owned(),
            label: "API key".to_owned(),
            description: String::new(),
            kind: SettingKind::Secret,
            required: false,
            default: None,
            min: None,
            max: None,
        };
        Manifest {
            extension_points: vec![PointKey::provide::<dyn Told>()],
            settings: SettingsSchema {
                fields: vec![api_key],
            },
            ..Idle.manifest()
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        registrar.provide::<dyn Told>(Priority::DEFAULT, Arc::new(Self));
        Ok(())
    }
}

/// Tells `points`' [`Told`]s, each call ending in `result`, and gives the call a while to end.
async fn tell(points: &ExtensionPoints, result: Result<(), &'static str>) -> Option<Failing> {
    let told = PointKey::provide::<dyn Told>();
    let before = points.failing(&PLUGIN, told);
    points.tell_each(move |_, _: Arc<dyn Told>| async move {
        result.map_err(Box::<dyn Error + Send + Sync>::from)
    });
    // The call runs as its own task, which records its outcome when it ends.
    for _ in 0..100 {
        tokio::task::yield_now().await;
        let now = points.failing(&PLUGIN, told);
        if now != before {
            return now;
        }
    }
    points.failing(&PLUGIN, told)
}

#[tokio::test]
async fn a_told_providers_failure_is_recorded_until_a_call_answers() {
    let plugins = Registry::new();
    plugins.enable(Arc::new(Listens)).unwrap();
    let points = plugins.extension_points();

    let failed = (tell(&points, Err("the topic is gone")).await).expect("the failure is recorded");
    let answered = tell(&points, Ok(())).await;

    assert_eq!(
        (failed.last_error.as_str(), failed.in_a_row),
        ("the topic is gone", 1)
    );
    assert_eq!(answered, None);
}

/// A Registry with [`Listens`] enabled, and its Extension Points.
fn listening() -> (Registry, ExtensionPoints) {
    let plugins = Registry::new();
    plugins.enable(Arc::new(Listens)).unwrap();
    let points = plugins.extension_points();
    (plugins, points)
}

/// Asks `points`' [`Told`], the call ending in `result`, and returns what is recorded after.
async fn ask<E: ProviderError<K>, K: FailureKind>(
    points: &ExtensionPoints,
    result: impl Fn() -> Result<(), E>,
) -> Option<Failing> {
    let answered = points
        .ask(&PLUGIN, |_: Arc<dyn Told>| std::future::ready(result()))
        .await;
    assert!(answered.is_some(), "the Provider was asked");
    points.failing(&PLUGIN, PointKey::provide::<dyn Told>())
}

fn failed(message: &'static str) -> Result<(), PluginError> {
    Err(message.into())
}

#[tokio::test]
async fn an_asked_providers_failure_is_recorded_at_its_point_until_a_call_answers() {
    let (_plugins, points) = listening();

    let failing = ask(&points, || failed("the service is down")).await;
    let answered = ask(&points, || Ok::<_, PluginError>(())).await;

    let failing = failing.expect("the failure is recorded");
    assert_eq!(
        (failing.last_error.as_str(), failing.in_a_row),
        ("the service is down", 1)
    );
    assert_eq!(answered, None);
}

#[tokio::test]
async fn reading_what_a_provider_says_of_itself_keeps_its_failure() {
    let (_plugins, points) = listening();
    ask(&points, || failed("the service is down")).await;

    let read = points
        .read_provider::<dyn Told, _>(&PLUGIN, |_| "told")
        .await;

    assert_eq!(read, Some("told"));
    let failing = points.failing(&PLUGIN, PointKey::provide::<dyn Told>());
    assert_eq!(failing.map(|failing| failing.in_a_row), Some(1));
}

#[tokio::test]
async fn a_fulfillers_try_again_in_records_no_failure_and_clears_an_earlier_one() {
    let (_plugins, points) = listening();
    let down = || Err::<(), _>(Retryable("the service is down".into()));
    let try_again = || Err::<(), _>(Retryable(TryAgainIn(Duration::from_secs(5)).into()));

    let failing = ask(&points, down).await;
    let waiting = ask(&points, try_again).await;

    assert!(failing.is_some());
    assert_eq!(waiting, None);
}

#[tokio::test]
async fn a_failures_recorded_message_has_the_plugins_secret_settings_blanked() {
    let plugins = Registry::new();
    let settings = BTreeMap::from([(
        "api_key".to_owned(),
        SettingValue::Text("key_secret".to_owned()),
    )]);
    plugins
        .enable_with_settings(Arc::new(Listens), Settings::new(settings))
        .unwrap();
    let points = plugins.extension_points();

    let failing = ask(&points, || failed("GET /movie?api_key=key_secret: 401")).await;

    assert_eq!(failing.unwrap().last_error, "GET /movie?api_key=…: 401");
}

#[test]
fn disabling_the_plugin_forgets_its_failures() {
    let (plugins, points) = enabled();
    points.provider_failed(&PLUGIN, recogniser(), "timed out");

    assert!(plugins.disable_now(&PLUGIN));

    assert_eq!(points.failing(&PLUGIN, recogniser()), None);
}

#[test]
fn a_failure_after_the_plugin_is_disabled_is_not_recorded() {
    let (plugins, points) = enabled();
    assert!(plugins.disable_now(&PLUGIN));

    let failing = points.provider_failed(&PLUGIN, recogniser(), "interrupted");

    assert_eq!(failing, None);
    assert_eq!(points.failing(&PLUGIN, recogniser()), None);
}

#[test]
fn a_plugin_enabled_again_starts_without_failures() {
    let (plugins, points) = enabled();
    points.provider_failed(&PLUGIN, recogniser(), "timed out");
    assert!(plugins.disable_now(&PLUGIN));
    points.provider_failed(&PLUGIN, recogniser(), "interrupted");

    plugins.enable(Arc::new(Idle)).unwrap();

    assert_eq!(points.failing(&PLUGIN, recogniser()), None);
}

#[test]
fn a_plugin_never_enabled_records_no_failure() {
    let points = ExtensionPoints::default();

    assert_eq!(
        points.provider_failed(&PLUGIN, recogniser(), "timed out"),
        None
    );
}
