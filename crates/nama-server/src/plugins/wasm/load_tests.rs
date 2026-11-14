use nama_plugin_proto::{
    CONTRACT_VERSION,
    nama::plugin::v1::{Manifest, Point, SettingField},
    services,
};

use super::judge;

#[test]
fn a_manifest_naming_no_points_is_refused_whatever_its_settings() {
    let mut package = Manifest {
        id: "old-plugin".to_owned(),
        version: "0.1.0".to_owned(),
        contract: format!("^{CONTRACT_VERSION}"),
        settings: vec![SettingField {
            key: "api_key".to_owned(),
            ..Default::default()
        }],
        ..Default::default()
    };

    let refused = judge(&mut package, &[])
        .err()
        .map(|refused| refused.to_string());

    assert_eq!(
        refused.as_deref(),
        Some(
            "its Manifest names no Extension Points, so it would do nothing; a Plugin built \
             before Extension Points became services must name them in points"
        )
    );
}

fn title_source(title_namespace: &str) -> Manifest {
    Manifest {
        id: "tvdb".to_owned(),
        version: "0.1.0".to_owned(),
        contract: format!("^{CONTRACT_VERSION}"),
        points: vec![Point {
            service: services::TITLE_SOURCE.to_owned(),
            version: "^1.0.0".to_owned(),
            ..Default::default()
        }],
        title_namespace: title_namespace.to_owned(),
        ..Default::default()
    }
}

#[test]
fn a_title_source_without_a_well_formed_namespace_is_refused() {
    let refusals = ["", "TV-DB"].map(|namespace| {
        judge(&mut title_source(namespace), &[])
            .err()
            .map(|refused| refused.to_string())
    });

    assert_eq!(
        refusals,
        [
            r#"its title_namespace names "", which isn't a namespace"#,
            r#"its title_namespace names "TV-DB", which isn't a namespace"#,
        ]
        .map(|refusal| Some(refusal.to_owned()))
    );
}

#[test]
fn a_title_source_with_a_malformed_lookup_namespace_is_refused() {
    let mut package = Manifest {
        title_lookup_namespaces: vec!["imdb".to_owned(), "TV-DB".to_owned()],
        ..title_source("tmdb")
    };

    let refused = judge(&mut package, &[])
        .err()
        .map(|refused| refused.to_string());

    assert_eq!(
        refused.as_deref(),
        Some(r#"its title_lookup_namespaces names "TV-DB", which isn't a namespace"#)
    );
}

#[test]
fn a_title_source_without_any_title_export_loads() {
    let judged = judge(&mut title_source("tvdb"), &[]).map(|manifest| manifest.id.to_string());

    assert_eq!(judged.ok().as_deref(), Some("tvdb"));
}

#[test]
fn title_namespace_is_ignored_unless_the_manifest_names_a_title_source() {
    let mut notifier = Manifest {
        points: vec![Point {
            service: services::NOTIFIER.to_owned(),
            version: "^1.0.0".to_owned(),
            ..Default::default()
        }],
        title_namespace: "TV-DB".to_owned(),
        ..title_source("tvdb")
    };

    let judged = judge(&mut notifier, &["deliver"]).map(|manifest| manifest.id.to_string());

    assert_eq!(judged.ok().as_deref(), Some("tvdb"));
}
