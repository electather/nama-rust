use std::time::Duration;

use nama_plugin_proto::{
    CONTRACT_VERSION,
    nama::plugin::v1::{Manifest, Permission, Point, SettingField, SettingKind},
    services,
};

use super::judge;

#[cfg(feature = "testing")]
#[expect(
    clippy::duplicate_mod,
    reason = "`automation_tests.rs` loads it too, and keeps its copy private"
)]
#[path = "../../../tests/support/test_plugin.rs"]
mod test_plugin;

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

fn personal(key: &str, kind: SettingKind) -> SettingField {
    SettingField {
        key: key.to_owned(),
        kind: kind.into(),
        ..Default::default()
    }
}

#[test]
fn personal_settings_a_user_could_widen_hosts_by_or_whose_keys_clash_are_refused() {
    let judged = |personal_settings: Vec<SettingField>| {
        let mut package = Manifest {
            settings: vec![personal("server", SettingKind::SETTING_KIND_TEXT)],
            personal_settings,
            ..title_source("ntfy")
        };
        judge(&mut package, &[])
            .err()
            .map(|refused| refused.to_string())
    };

    let refusals = [
        vec![personal("webhook", SettingKind::SETTING_KIND_URL)],
        vec![personal("language", SettingKind::SETTING_KIND_CHOICE)],
        vec![
            personal("topic", SettingKind::SETTING_KIND_TEXT),
            personal("topic", SettingKind::SETTING_KIND_SECRET),
        ],
        vec![personal("server", SettingKind::SETTING_KIND_TEXT)],
    ]
    .map(judged);
    let taken = judged(vec![
        personal("topic", SettingKind::SETTING_KIND_TEXT),
        personal("token", SettingKind::SETTING_KIND_SECRET),
        personal("quiet", SettingKind::SETTING_KIND_BOOL),
        personal("priority", SettingKind::SETTING_KIND_INTEGER),
    ]);

    assert_eq!(
        refusals,
        [
            "its Personal setting webhook is Url, and a Personal setting may only be Text, \
             Secret, Bool or Integer",
            "its Personal setting language is Choice, and a Personal setting may only be Text, \
             Secret, Bool or Integer",
            "its Personal setting topic repeats a key",
            "its Personal setting server has a Plugin setting's key",
        ]
        .map(|refusal| Some(refusal.to_owned()))
    );
    assert_eq!(taken, None);
}

fn automation(tick_every_seconds: u32, permissions: Vec<buffa::EnumValue<Permission>>) -> Manifest {
    Manifest {
        points: vec![Point {
            service: services::AUTOMATION.to_owned(),
            version: "^1.0.0".to_owned(),
            ..Default::default()
        }],
        permissions,
        tick_every_seconds,
        ..title_source("cleanup")
    }
}

#[test]
fn an_automation_reads_its_permissions_and_tick() {
    let mut package = automation(
        3600,
        vec![
            Permission::PERMISSION_STORE.into(),
            Permission::PERMISSION_DELETE_MEDIA.into(),
        ],
    );

    let manifest = judge(&mut package, &["tick"]).unwrap();

    assert_eq!(
        manifest.permissions,
        [
            Permission::PERMISSION_STORE,
            Permission::PERMISSION_DELETE_MEDIA
        ]
    );
    assert_eq!(manifest.tick_every, Some(Duration::from_secs(3600)));
}

#[test]
fn an_unknown_or_unspecified_permission_is_refused() {
    let refusals = [
        buffa::EnumValue::Unknown(99),
        Permission::PERMISSION_UNSPECIFIED.into(),
    ]
    .map(|permission| {
        judge(&mut automation(60, vec![permission]), &["tick"])
            .err()
            .map(|refused| refused.to_string())
    });

    assert_eq!(
        refusals,
        [
            "its Manifest requests the Permission 99, which this Server doesn't know",
            "its Manifest requests the Permission PERMISSION_UNSPECIFIED, which this Server \
             doesn't know",
        ]
        .map(|refusal| Some(refusal.to_owned()))
    );
}

#[test]
fn an_automation_ticking_under_a_minute_or_naming_no_tick_is_refused() {
    let refusals = [0, 59].map(|seconds| {
        judge(&mut automation(seconds, Vec::new()), &["tick"])
            .err()
            .map(|refused| refused.to_string())
    });

    assert_eq!(
        refusals,
        [0, 59].map(|seconds| Some(format!(
            "its Manifest names nama.plugin.v1.AutomationService, and ticks every {seconds} \
             seconds, under the minimum of 60"
        )))
    );
}

#[test]
fn an_automation_without_a_tick_export_is_refused() {
    let refused = judge(&mut automation(60, Vec::new()), &[])
        .err()
        .map(|refused| refused.to_string());

    assert_eq!(
        refused.as_deref(),
        Some("its Manifest names nama.plugin.v1.AutomationService, and it has no tick export")
    );
}

#[test]
fn tick_every_is_ignored_unless_the_manifest_names_an_automation() {
    let mut package = Manifest {
        tick_every_seconds: 5,
        ..title_source("tvdb")
    };

    let manifest = judge(&mut package, &[]).unwrap();

    assert_eq!(manifest.tick_every, None);
}

#[cfg(feature = "testing")]
#[test]
fn an_installed_plugins_clock_follows_the_servers_advanced_clock() {
    use std::sync::Arc;

    use super::{compile, write_cache_config};
    use crate::db;

    let data_dir = tempfile::tempdir().unwrap();
    let cache_config = write_cache_config(data_dir.path()).unwrap();
    // The test Plugin's Observer logs what `nama_pdk::now` reads when its `observer` is `clock`.
    let config = vec![("observer".to_owned(), "clock".to_owned())];
    let compiled = compile(&test_plugin::build(""), &cache_config, Vec::new(), config).unwrap();
    let log = tempfile::NamedTempFile::new().unwrap();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(Arc::new(log.reopen().unwrap()))
        .with_ansi(false)
        .finish();

    db::advance_clock(Duration::from_hours(30 * 24));
    let before = db::now_ms();
    tracing::subscriber::with_default(subscriber, || {
        extism::Plugin::new_from_compiled(&compiled)
            .unwrap()
            .call::<&[u8], &[u8]>("observe_events", &[])
            .map(<[u8]>::to_vec)
    })
    .unwrap();
    let after = db::now_ms();

    let logged = std::fs::read_to_string(log.path()).unwrap();
    let (_, read) = logged.split_once("test-plugin's clock reads ").unwrap();
    let read: i64 = read
        .split(|c: char| !c.is_ascii_digit())
        .next()
        .unwrap()
        .parse()
        .unwrap();
    assert!(
        (before..=after).contains(&read),
        "{before} <= {read} <= {after}"
    );
}
