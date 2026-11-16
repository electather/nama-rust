use std::sync::Arc;

use nama_plugin_api::{
    Manifest, Plugin, PluginError, PluginId, PointKey, Registrar, recogniser::Recogniser,
};

use crate::plugins::{ExtensionPoints, Registry};

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
