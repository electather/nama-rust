use std::time::Duration;

use nama_plugin_proto::{
    CONTRACT_VERSION,
    nama::plugin::v1::{
        Choice, Job, Manifest, Permission, Point, SettingField, SettingKind, SettingValue, Texts,
        setting_value,
    },
    services,
};

use nama_plugin_api::settings::{self, Settings, SettingsSchema};

use super::{extism_config, judge};

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
        vec![personal("ratings", SettingKind::SETTING_KIND_CHOICES)],
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
            "its Personal setting ratings is Choices, and a Personal setting may only be Text, \
             Secret, Bool or Integer",
            "its Personal setting topic repeats a key",
            "its Personal setting server has a Plugin setting's key",
        ]
        .map(|refusal| Some(refusal.to_owned()))
    );
    assert_eq!(taken, None);
}

fn days(min_value: Option<i64>, max_value: Option<i64>, default: Option<i64>) -> SettingField {
    SettingField {
        min_value,
        max_value,
        default_value: default
            .map(|days| SettingValue {
                value: Some(setting_value::Value::IntegerValue(days)),
                ..Default::default()
            })
            .into(),
        ..personal("days", SettingKind::SETTING_KIND_INTEGER)
    }
}

fn judged_settings(setting: SettingField) -> Result<Vec<settings::SettingField>, String> {
    let mut package = Manifest {
        settings: vec![setting],
        ..title_source("cleanup")
    };
    judge(&mut package, &[])
        .map(|manifest| manifest.settings.fields)
        .map_err(|refused| refused.to_string())
}

#[test]
fn bounds_that_cross_sit_on_another_kind_or_leave_out_the_default_are_refused() {
    let refusals = [
        days(Some(10), Some(1), None),
        SettingField {
            min_value: Some(0),
            ..personal("api_key", SettingKind::SETTING_KIND_TEXT)
        },
        days(Some(0), None, Some(-1)),
    ]
    .map(|setting| judged_settings(setting).err());

    assert_eq!(
        refusals,
        [
            "its setting days's min_value 10 is over its max_value 1",
            "its setting api_key has min_value or max_value, which only an Integer setting may",
            "its setting days's default is outside its bounds",
        ]
        .map(|refusal| Some(refusal.to_owned()))
    );
}

#[test]
fn an_integer_settings_bounds_reach_its_schema() {
    let fields = judged_settings(days(Some(0), Some(365), Some(30))).unwrap();

    assert_eq!(
        fields
            .iter()
            .map(|field| (field.min, field.max))
            .collect::<Vec<_>>(),
        [(Some(0), Some(365))]
    );
}

fn ratings(choices: &[&str], default: Option<&[&str]>) -> SettingField {
    SettingField {
        choices: (choices.iter())
            .map(|&value| Choice {
                value: value.to_owned(),
                label: value.to_owned(),
                ..Default::default()
            })
            .collect(),
        default_value: default
            .map(|values| SettingValue {
                value: Some(setting_value::Value::Texts(Box::new(Texts {
                    values: values.iter().map(|&value| value.to_owned()).collect(),
                    ..Default::default()
                }))),
                ..Default::default()
            })
            .into(),
        ..personal("ratings", SettingKind::SETTING_KIND_CHOICES)
    }
}

#[test]
fn a_choices_settings_manifest_choices_reach_its_schema() {
    let fields = judged_settings(ratings(&["G", "PG"], Some(&["PG"]))).unwrap();

    assert_eq!(
        (fields.iter())
            .map(|field| field
                .choices
                .iter()
                .map(|choice| choice.value.as_str())
                .collect())
            .collect::<Vec<Vec<_>>>(),
        [["G", "PG"]]
    );
}

#[test]
fn a_choices_settings_default_keeps_each_value_once_in_the_order_its_choices_are_listed() {
    let fields = judged_settings(ratings(&["G", "PG", "R"], Some(&["R", "G", "R"]))).unwrap();

    let default = match fields.first().and_then(|field| field.default.as_ref()) {
        Some(settings::SettingValue::Texts(values)) => Some(values.as_slice()),
        _ => None,
    };
    assert_eq!(default, Some(["G".to_owned(), "R".to_owned()].as_slice()));
}

#[test]
fn a_choices_setting_without_manifest_choices_loads_only_with_a_settings_check() {
    let judged = |exported: &[&str]| {
        let mut package = Manifest {
            settings: vec![ratings(&[], None)],
            ..title_source("cleanup")
        };
        judge(&mut package, exported).map_err(|refused| refused.to_string())
    };

    assert_eq!(
        judged(&[]).err().as_deref(),
        Some(
            "its setting ratings is Choices with no choices, and it has no check_settings export \
             to offer some"
        )
    );
    assert_eq!(judged(&["check_settings"]).err(), None);
}

#[test]
fn choices_on_another_kind_repeated_or_missing_the_default_are_refused() {
    let refusals = [
        SettingField {
            kind: SettingKind::SETTING_KIND_CHOICE.into(),
            ..ratings(&["G"], None)
        },
        ratings(&["G", "PG", "G"], None),
        ratings(&["G", "PG"], Some(&["G", "R"])),
    ]
    .map(|setting| judged_settings(setting).err());

    assert_eq!(
        refusals,
        [
            "its setting ratings has choices, which only a Choices setting may",
            r#"its setting ratings lists the choice "G" twice"#,
            "its setting ratings's default holds a value that isn't among its choices",
        ]
        .map(|refusal| Some(refusal.to_owned()))
    );
}

fn requesting(permissions: Vec<buffa::EnumValue<Permission>>) -> Manifest {
    Manifest {
        permissions,
        ..title_source("cleanup")
    }
}

#[test]
fn a_plugin_reads_its_permissions() {
    let mut package = requesting(vec![
        Permission::PERMISSION_STORE.into(),
        Permission::PERMISSION_DELETE_MEDIA.into(),
    ]);

    let manifest = judge(&mut package, &[]).unwrap();

    assert_eq!(
        manifest.permissions,
        [
            Permission::PERMISSION_STORE,
            Permission::PERMISSION_DELETE_MEDIA
        ]
    );
}

#[test]
fn an_unknown_or_unspecified_permission_is_refused() {
    let refusals = [
        buffa::EnumValue::Unknown(99),
        Permission::PERMISSION_UNSPECIFIED.into(),
    ]
    .map(|permission| {
        judge(&mut requesting(vec![permission]), &[])
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

fn job(id: &str, schedule: Option<&str>) -> Job {
    Job {
        id: id.to_owned(),
        name: format!("Job {id}"),
        description: format!("Does {id}."),
        schedule: schedule.map(str::to_owned),
        ..Default::default()
    }
}

fn job_runner(jobs: Vec<Job>) -> Manifest {
    Manifest {
        points: vec![Point {
            service: services::JOB.to_owned(),
            version: "^1.0.0".to_owned(),
            ..Default::default()
        }],
        jobs,
        ..title_source("chores")
    }
}

#[test]
fn a_job_runner_reads_its_jobs_with_and_without_a_schedule() {
    let mut package = job_runner(vec![job("tidy", Some("0 4 * * *")), job("on-ask", None)]);

    let manifest = judge(&mut package, &["run_job"]).unwrap();

    let jobs: Vec<_> = (manifest.jobs.iter())
        .map(|job| (job.id.as_str(), job.name.as_str(), job.schedule.as_deref()))
        .collect();
    assert_eq!(
        jobs,
        [
            ("tidy", "Job tidy", Some("0 4 * * *")),
            ("on-ask", "Job on-ask", None)
        ]
    );
    assert_eq!(manifest.jobs[0].description, "Does tidy.");
}

#[test]
fn a_job_runner_declaring_no_jobs_a_bad_id_or_a_bad_schedule_is_refused() {
    let refused = |jobs| {
        judge(&mut job_runner(jobs), &["run_job"])
            .err()
            .map(|refused| refused.to_string())
    };

    assert_eq!(
        refused(Vec::new()).as_deref(),
        Some("its Manifest names nama.plugin.v1.JobService and declares no Jobs")
    );
    assert_eq!(
        refused(vec![job("tidy", Some("0 0 4 * * *"))]).as_deref(),
        Some(
            "its Job tidy's schedule \"0 0 4 * * *\" has a seconds field; a schedule has 5 \
             fields and runs to the minute"
        )
    );
    assert_eq!(
        refused(vec![job("tidy", Some("0 25 * * *"))]).as_deref(),
        Some("its Job tidy's schedule \"0 25 * * *\" has hour 25, outside 0-23")
    );
    assert_eq!(
        refused(vec![job("tidy", None), job("tidy", None)]).as_deref(),
        Some("its Job tidy repeats an id")
    );
    assert_eq!(
        refused(vec![job("Tidy up", None)]).as_deref(),
        Some("its Job \"Tidy up\" has an id that isn't one or more of a-z, 0-9 and -")
    );
}

#[test]
fn jobs_are_ignored_unless_the_manifest_names_a_job_runner() {
    let mut package = Manifest {
        jobs: vec![job("tidy", Some("not a schedule"))],
        ..title_source("tvdb")
    };

    let manifest = judge(&mut package, &[]).unwrap();

    assert_eq!(manifest.jobs, []);
}

#[cfg(feature = "testing")]
#[test]
fn an_installed_plugins_clock_follows_the_servers_advanced_clock() {
    use std::sync::Arc;

    use nama_plugin_api::PluginId;

    use super::{compile_with_kernel, write_cache_config};
    use crate::{
        db,
        log_capture::CapturedLog,
        plugins::wasm::{calls::Calls, plugin_log::PluginLog},
    };

    let data_dir = tempfile::tempdir().unwrap();
    let cache_config = write_cache_config(data_dir.path()).unwrap();
    // The test Plugin's Observer logs what `nama_pdk::now` reads when its `observer` is `clock`.
    let config = vec![("observer".to_owned(), "clock".to_owned())];
    let plugin_log = PluginLog::new(PluginId::from("test-plugin".to_owned()), Arc::default());
    let compiled = compile_with_kernel(
        &test_plugin::build(""),
        &cache_config,
        Vec::new(),
        config,
        None,
        Arc::new(plugin_log),
        Calls::default(),
    )
    .unwrap();

    // The offset is process-wide: a step near `ASK_AGAIN_AFTER` races the Segments tests.
    db::advance_clock(Duration::from_secs(2));
    let before = db::now_ms();
    let log = CapturedLog::start();
    extism::Plugin::new_from_compiled(&compiled)
        .unwrap()
        .call::<&[u8], &[u8]>("observe_events", &[])
        .unwrap();
    let logged = log.finish();
    let after = db::now_ms();

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

#[cfg(feature = "testing")]
#[test]
fn a_call_with_no_enablement_logs_nothing() {
    use super::{compile, write_cache_config};
    use crate::log_capture::CapturedLog;

    let data_dir = tempfile::tempdir().unwrap();
    let cache_config = write_cache_config(data_dir.path()).unwrap();
    // The test Plugin's Observer logs what `nama_pdk::now` reads when its `observer` is `clock`.
    let config = vec![("observer".to_owned(), "clock".to_owned())];
    let compiled = compile(&test_plugin::build(""), &cache_config, Vec::new(), config).unwrap();

    let log = CapturedLog::start();
    extism::Plugin::new_from_compiled(&compiled)
        .unwrap()
        .call::<&[u8], &[u8]>("observe_events", &[])
        .unwrap();
    let logged = log.finish();

    assert!(!logged.contains("test-plugin's clock reads"), "{logged}");
}

#[test]
fn setup_text_links_and_a_licence_that_fit_are_kept_with_links_as_parsed() {
    let mut package = Manifest {
        setup_markdown: "Get an API key at **thetvdb.com**.".to_owned(),
        homepage_url: "https://thetvdb.com/api-information".to_owned(),
        // A browser would read this as a link relative to the page.
        source_url: " https:github.com/example/tvdb".to_owned(),
        license: "(MIT OR Apache-2.0) AND GPL-2.0+".to_owned(),
        ..title_source("tvdb")
    };

    let manifest = judge(&mut package, &[]).unwrap();

    assert_eq!(
        [
            manifest.setup_markdown,
            manifest.homepage_url,
            manifest.source_url,
            manifest.license
        ],
        [
            "Get an API key at **thetvdb.com**.",
            "https://thetvdb.com/api-information",
            "https://github.com/example/tvdb",
            "(MIT OR Apache-2.0) AND GPL-2.0+",
        ]
    );
}

#[test]
fn a_bad_setup_text_link_or_licence_is_dropped_saying_why_and_the_plugin_still_taken() {
    use crate::log_capture::CapturedLog;

    let url = |url: String| Manifest {
        homepage_url: url,
        ..title_source("tvdb")
    };
    let licensed = |license: String| Manifest {
        license,
        ..title_source("tvdb")
    };
    let dropped = [
        (
            url("javascript:alert(1)".to_owned()),
            "homepage_url: its scheme is javascript",
        ),
        (
            url(" FTP://example.com/".to_owned()),
            "homepage_url: its scheme is ftp",
        ),
        (
            url("example.com".to_owned()),
            "homepage_url: it isn't a URL",
        ),
        (
            url(format!("https://example.com/{}", "a".repeat(2 * 1024))),
            "homepage_url: it is over 2048 bytes",
        ),
        (
            url(format!("{}:x", "a".repeat(2 * 1024))),
            "homepage_url: it is over 2048 bytes",
        ),
        (
            Manifest {
                source_url: "ftp://example.com/".to_owned(),
                ..title_source("tvdb")
            },
            "source_url: its scheme is ftp",
        ),
        (
            Manifest {
                setup_markdown: "a".repeat(4 * 1024 + 1),
                ..title_source("tvdb")
            },
            "setup_markdown: it is over 4096 bytes",
        ),
        (
            licensed("MIT; rm -rf".to_owned()),
            "license: it isn't an SPDX expression",
        ),
        (
            licensed("   ".to_owned()),
            "license: it isn't an SPDX expression",
        ),
        (
            licensed("MIT ".repeat(33)),
            "license: it is over 128 characters",
        ),
    ];

    for (mut package, why) in dropped {
        let log = CapturedLog::start();
        let manifest = judge(&mut package, &[]).unwrap();
        let logged = log.finish();

        let kept = [
            manifest.setup_markdown,
            manifest.homepage_url,
            manifest.source_url,
            manifest.license,
        ];
        assert_eq!(kept, ["", "", "", ""], "{why}");
        assert!(logged.contains("tvdb"), "{logged}");
        assert!(logged.contains(&format!("dropped its {why}")), "{logged}");
    }
}

#[test]
fn a_plugin_reads_picked_choices_as_a_json_array() {
    let fields = judged_settings(ratings(&["PG-13", "a, \"b\""], None)).unwrap();
    let picked = vec!["PG-13".to_owned(), "a, \"b\"".to_owned()];
    let settings =
        Settings::new([("ratings".to_owned(), settings::SettingValue::Texts(picked))].into());

    let config = extism_config(&SettingsSchema { fields }, &settings);

    assert_eq!(
        config,
        [("ratings".to_owned(), r#"["PG-13","a, \"b\""]"#.to_owned())]
    );
}

#[cfg(feature = "testing")]
#[test]
fn a_plugin_is_compiled_again_only_once_its_file_changes_and_forgotten_once_it_is_gone() {
    use std::{fs, path::PathBuf, sync::Arc, time::SystemTime};

    use super::{Installed, Loaded, Refused, read_plugins};

    /// The `.wasm` each Plugin read holds, in the order read: the same `Arc` if it was reused.
    fn wasm(found: &[(PathBuf, Result<Installed, Refused>)]) -> Vec<Arc<[u8]>> {
        let held =
            |(_, plugin): &(_, Result<Installed, _>)| Arc::clone(&plugin.as_ref().unwrap().wasm);
        found.iter().map(held).collect()
    }
    let reused = |before: &[Arc<[u8]>], after: &[Arc<[u8]>]| -> Vec<bool> {
        before
            .iter()
            .zip(after)
            .map(|(a, b)| Arc::ptr_eq(a, b))
            .collect()
    };
    let data_dir = tempfile::tempdir().unwrap();
    let dir = data_dir.path().join("plugins");
    fs::create_dir(&dir).unwrap();
    let file = dir.join("test.wasm");
    fs::write(&file, test_plugin::build("")).unwrap();
    let mut loaded = Loaded::default();

    let first = wasm(&read_plugins(data_dir.path(), &mut loaded));
    let again = wasm(&read_plugins(data_dir.path(), &mut loaded));
    // The bundled Plugins, then the dropped-in one.
    assert!(reused(&first, &again).iter().all(|&reused| reused));

    let later = SystemTime::now() + Duration::from_secs(60);
    let opened = fs::File::options().write(true).open(&file).unwrap();
    opened.set_modified(later).unwrap();
    let changed = wasm(&read_plugins(data_dir.path(), &mut loaded));
    assert_eq!(reused(&again, &changed).last(), Some(&false));

    fs::remove_file(&file).unwrap();
    read_plugins(data_dir.path(), &mut loaded);
    assert!(!loaded.0.contains_key(&file));
}

#[test]
fn a_refused_plugin_is_read_again_only_once_its_file_changes() {
    use std::{fs, time::SystemTime};

    use super::{Loaded, Refused, read_plugins};

    let data_dir = tempfile::tempdir().unwrap();
    let dir = data_dir.path().join("plugins");
    fs::create_dir(&dir).unwrap();
    let file = dir.join("refused.wasm");
    fs::write(&file, "not wasm").unwrap();
    let mut loaded = Loaded::default();
    let refused = |read_before: &mut Loaded| {
        let mut found = read_plugins(data_dir.path(), read_before);
        found.pop().unwrap().1.err().unwrap()
    };

    assert!(matches!(refused(&mut loaded), Refused::NotWasm));
    let again = refused(&mut loaded);
    assert!(matches!(&again, Refused::Again(_)), "{again:?}");
    assert_eq!(again.to_string(), Refused::NotWasm.to_string());

    let later = SystemTime::now() + Duration::from_secs(60);
    let opened = fs::File::options().write(true).open(&file).unwrap();
    opened.set_modified(later).unwrap();
    assert!(matches!(refused(&mut loaded), Refused::NotWasm));
}
