use std::sync::Arc;

use buffa::Message;

use nama_plugin_api::{
    Plugin, PluginId, PointKey, Registrar,
    fulfiller::{Fulfiller, MovieDeleted, Movies, StopMonitoring, TitleId, stop_monitoring::Title},
    notifier::Notifier,
    settings::{SettingField, SettingKind, SettingValue, Settings, SettingsSchema},
};
use nama_plugin_proto::{
    CONTRACT_VERSION,
    nama::plugin::v1::{self as wire, Manifest, ObservePoint, ProvidePoint, setting_value},
};

use super::{
    BUNDLED, Running, allowed_hosts, check_allowed_hosts, check_exports, compile, decode,
    extism_config, is_crash, judge, load, points, settings_schema, write_cache_config,
};

// The errors as Extism 1.41 gives them for the test Plugin's Observer.
#[test]
fn a_trap_or_timeout_is_a_crash_and_a_returned_or_host_error_is_not() {
    // As Extism builds it: the panic's message, with the trap as context.
    let trap = wasmtime::Error::new(wasmtime::Trap::UnreachableCodeReached);
    let panicked = extism::Error::from(
        wasmtime::Error::msg("panicked at src/lib.rs:73:30: oops").context(trap),
    );
    let refused = extism::Error::msg("HTTP request to http://sonarr.lan/ is not allowed")
        .context("error while executing at wasm backtrace");

    assert!(is_crash(&panicked));
    assert!(is_crash(&extism::Error::msg("timeout")));
    assert!(is_crash(&extism::Error::msg("oom")));
    assert!(!is_crash(&refused));
    assert!(!is_crash(&extism::Error::msg("the service is down")));
}

fn field(key: &str, kind: SettingKind) -> SettingField {
    SettingField {
        key: key.to_owned(),
        label: String::new(),
        description: String::new(),
        kind,
        required: false,
        default: None,
    }
}

fn text(text: &str) -> SettingValue {
    SettingValue::Text(text.to_owned())
}

#[test]
fn a_plugin_may_reach_its_manifests_hosts_and_the_hosts_of_its_url_settings_only() {
    let manifest = Manifest {
        allowed_hosts: vec!["*.themoviedb.org".to_owned()],
        ..Default::default()
    };
    let schema = SettingsSchema {
        fields: vec![
            field("sonarr", SettingKind::Url),
            field("radarr", SettingKind::Url),
            field("note", SettingKind::Text),
        ],
    };
    let settings = Settings::new(
        [
            ("sonarr".to_owned(), text("http://sonarr.lan:8989/api")),
            ("note".to_owned(), text("http://not-a-url-setting.lan/")),
        ]
        .into(),
    );

    let hosts = allowed_hosts(&manifest, &schema, &settings);

    assert_eq!(hosts, ["*.themoviedb.org", "sonarr.lan"]);
}

#[test]
fn a_url_settings_host_names_only_itself_not_a_glob() {
    let schema = SettingsSchema {
        fields: vec![
            field("any", SettingKind::Url),
            field("ipv6", SettingKind::Url),
        ],
    };
    let settings = Settings::new(
        [
            ("any".to_owned(), text("http://*/")),
            ("ipv6".to_owned(), text("http://[::1]:8989/")),
        ]
        .into(),
    );

    let hosts = allowed_hosts(&Manifest::default(), &schema, &settings);

    // Matched as Extism matches them.
    let reaches = |host: &str| {
        hosts
            .iter()
            .any(|allowed| glob::Pattern::new(allowed).is_ok_and(|glob| glob.matches(host)))
    };
    assert!(reaches("*"));
    assert!(reaches("[::1]"));
    assert!(!reaches("example.com"));
}

#[test]
fn an_allowed_host_with_a_port_refuses_the_plugin() {
    let hosts = ["*.themoviedb.org".to_owned(), "sonarr.lan:8989".to_owned()];

    let refused = check_allowed_hosts(&hosts);

    assert_eq!(
        refused.err().map(|refused| refused.to_string()),
        Some(
            "its allowed host sonarr.lan:8989 has a port, and allowed hosts are matched without one"
                .to_owned()
        )
    );
    let without_port = check_allowed_hosts(&["*.themoviedb.org".to_owned()]);
    assert_eq!(without_port.ok(), Some(()));
}

#[test]
fn a_plugin_reads_each_setting_as_text() {
    let schema = SettingsSchema {
        fields: vec![
            field("on", SettingKind::Bool),
            field("count", SettingKind::Integer),
            field("name", SettingKind::Text),
            field("unset", SettingKind::Text),
        ],
    };
    let settings = Settings::new(
        [
            ("on".to_owned(), SettingValue::Bool(true)),
            ("count".to_owned(), SettingValue::Integer(-3)),
            ("name".to_owned(), text("Nama")),
        ]
        .into(),
    );

    let config = extism_config(&schema, &settings);

    let expected = [("on", "true"), ("count", "-3"), ("name", "Nama")]
        .map(|(key, value)| (key.to_owned(), value.to_owned()));
    assert_eq!(config, expected);
}

fn manifest(kind: wire::SettingKind) -> Manifest {
    let field = wire::SettingField {
        key: "api_key".to_owned(),
        label: "API key".to_owned(),
        description: "The key.".to_owned(),
        kind: kind.into(),
        required: true,
        default_value: wire::SettingValue {
            value: Some(setting_value::Value::TextValue("none".to_owned())),
            ..Default::default()
        }
        .into(),
        ..Default::default()
    };
    Manifest {
        settings: vec![field],
        ..Default::default()
    }
}

#[test]
fn a_manifests_settings_become_its_settings_schema() {
    let schema = settings_schema(&manifest(wire::SettingKind::SETTING_KIND_SECRET)).ok();

    let expected = SettingField {
        key: "api_key".to_owned(),
        label: "API key".to_owned(),
        description: "The key.".to_owned(),
        kind: SettingKind::Secret,
        required: true,
        default: Some(SettingValue::Text("none".to_owned())),
    };
    assert_eq!(schema.map(|schema| schema.fields), Some(vec![expected]));
}

#[test]
fn a_default_of_another_kind_refuses_the_plugin() {
    let refused = settings_schema(&manifest(wire::SettingKind::SETTING_KIND_BOOL));

    assert_eq!(
        refused.err().map(|refused| refused.to_string()),
        Some("its setting api_key's default isn't Bool".to_owned())
    );
}

#[test]
fn a_setting_of_no_known_kind_refuses_the_plugin() {
    let refused = settings_schema(&manifest(wire::SettingKind::SETTING_KIND_UNSPECIFIED));

    assert_eq!(
        refused.err().map(|refused| refused.to_string()),
        Some("its setting api_key is of a kind this Server doesn't know".to_owned())
    );
}

/// A Manifest this Server takes, for a Notifier with a logo, before each test changes it.
fn notifier() -> Manifest {
    Manifest {
        id: "test-plugin".to_owned(),
        name: "Test Plugin".to_owned(),
        description: "Stands in for an Installed Plugin.".to_owned(),
        version: "0.1.0".to_owned(),
        contract: format!("^{CONTRACT_VERSION}"),
        providers: vec![ProvidePoint::PROVIDE_POINT_NOTIFIER.into()],
        logo_svg: r#"<svg xmlns="http://www.w3.org/2000/svg"/>"#.to_owned(),
        ..Default::default()
    }
}

/// `package` judged as from a Package exporting `deliver`, with what it logged.
fn judged(mut package: Manifest) -> (Result<nama_plugin_api::Manifest, String>, String) {
    let log = tempfile::NamedTempFile::new().unwrap();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(Arc::new(log.reopen().unwrap()))
        .with_ansi(false)
        .finish();
    let judged = tracing::subscriber::with_default(subscriber, || {
        judge(&mut package, &["deliver"]).map_err(|refused| refused.to_string())
    });
    (judged, std::fs::read_to_string(log.path()).unwrap())
}

#[test]
fn a_manifest_this_server_takes_is_read_as_the_kernel_reads_it() {
    let (judged, _) = judged(notifier());

    let manifest = judged.unwrap();
    assert_eq!(manifest.id, PluginId::from_static("test-plugin"));
    assert_eq!(
        (manifest.name.as_str(), manifest.description.as_str()),
        ("Test Plugin", "Stands in for an Installed Plugin.")
    );
    assert_eq!(manifest.version, "0.1.0");
    assert_eq!(
        manifest.extension_points,
        [PointKey::provide::<dyn Notifier>()]
    );
    assert!(manifest.logo_svg.unwrap().contains("<svg"));
}

#[test]
fn a_logo_is_kept_without_its_scripts() {
    let (judged, _) = judged(Manifest {
        logo_svg: r#"<svg xmlns="http://www.w3.org/2000/svg" onload="alert(1)">
                <script>alert(2)</script><circle r="4" onclick="alert(3)"/></svg>"#
            .to_owned(),
        ..notifier()
    });

    let logo = judged.unwrap().logo_svg.unwrap();
    assert!(logo.contains("<circle"), "{logo}");
    for gone in ["onload", "script", "onclick", "alert"] {
        assert!(!logo.contains(gone), "{logo}");
    }
}

#[test]
fn a_manifest_from_before_plugins_had_a_name_or_a_logo_is_taken_without_them() {
    let (judged, _) = judged(Manifest {
        name: String::new(),
        description: String::new(),
        logo_svg: String::new(),
        ..notifier()
    });

    let manifest = judged.unwrap();
    assert_eq!(
        (manifest.name.as_str(), manifest.description.as_str()),
        ("", "")
    );
    assert_eq!(manifest.logo_svg, None);
}

#[test]
fn a_manifest_for_another_contract_is_refused_for_that_before_the_rest_is_decoded() {
    let wanting = |contract: &str| {
        Manifest {
            contract: contract.to_owned(),
            ..notifier()
        }
        .encode_to_vec()
    };
    // A setting whose default is text, as Plugin contract 0.1 encoded it: its `SettingValue.text`
    // was field 3, now the varint `integer_value`.
    let mut built_for_0_1 = wanting("^0.1.0");
    built_for_0_1.extend([0x22, 0x05, 0x32, 0x03, 0x1a, 0x01, b'x']);
    Manifest::decode_from_slice(&built_for_0_1).unwrap_err();

    let refusals = [built_for_0_1, wanting("one")]
        .map(|bytes| decode(&bytes).err().map(|refused| refused.to_string()));

    assert_eq!(
        refusals,
        [
            format!("it wants Plugin contract ^0.1.0, and this Server has {CONTRACT_VERSION}"),
            r#"its Plugin contract requirement "one" isn't a semver requirement: unexpected character 'o' while parsing major version number"#.to_owned(),
        ]
        .map(Some)
    );
}

#[test]
fn each_thing_a_manifest_can_get_wrong_refuses_it_saying_what() {
    let text = |text: &str| wire::SettingValue {
        value: Some(setting_value::Value::TextValue(text.to_owned())),
        ..Default::default()
    };
    let wrong = [
        Manifest {
            allowed_hosts: vec!["sonarr.lan:8989".to_owned()],
            ..notifier()
        },
        Manifest {
            observers: vec![99.into()],
            ..notifier()
        },
        Manifest {
            providers: vec![ProvidePoint::PROVIDE_POINT_CONNECTOR.into()],
            ..notifier()
        },
        Manifest {
            settings: vec![wire::SettingField {
                key: "count".to_owned(),
                ..Default::default()
            }],
            ..notifier()
        },
        Manifest {
            settings: vec![wire::SettingField {
                key: "count".to_owned(),
                kind: wire::SettingKind::SETTING_KIND_INTEGER.into(),
                default_value: text("three").into(),
                ..Default::default()
            }],
            ..notifier()
        },
    ];

    let refusals = wrong.map(|package| judged(package).0.err());

    assert_eq!(
        refusals,
        [
            "its allowed host sonarr.lan:8989 has a port, and allowed hosts are matched without one".to_owned(),
            "its Manifest names observer 99, an Extension Point this Server doesn't know".to_owned(),
            "its Manifest names PROVIDE_POINT_CONNECTOR, and it has no start_connection export".to_owned(),
            "its setting count is of a kind this Server doesn't know".to_owned(),
            "its setting count's default isn't Integer".to_owned(),
        ]
        .map(Some)
    );
}

#[test]
fn a_logo_over_64_kib_or_not_svg_is_dropped_saying_why_and_the_plugin_still_taken() {
    let padding = " ".repeat(64 * 1024);
    let dropped = [
        (
            format!(r#"<svg xmlns="http://www.w3.org/2000/svg">{padding}</svg>"#),
            "it is over 65536 bytes",
        ),
        (
            r#"<html xmlns="http://www.w3.org/1999/xhtml"/>"#.to_owned(),
            "No acceptable SVG elements found",
        ),
        (
            r#"<svg xmlns="http://www.w3.org/2000/svg">"#.to_owned(),
            "it isn't SVG",
        ),
    ];

    for (logo_svg, why) in dropped {
        let (judged, log) = judged(Manifest {
            logo_svg,
            ..notifier()
        });

        assert_eq!(judged.map(|manifest| manifest.logo_svg), Ok(None), "{why}");
        assert!(log.contains("test-plugin"), "{log}");
        assert!(log.contains("dropped its logo"), "{log}");
        assert!(log.contains(why), "{log}");
    }
}

#[test]
fn every_bundled_plugin_has_the_exports_its_manifests_points_need() {
    let data_dir = tempfile::tempdir().unwrap();
    let cache_config = write_cache_config(data_dir.path()).unwrap();

    for (file, wasm) in BUNDLED {
        let refused = load(wasm.to_vec(), &cache_config).err();

        assert_eq!(refused.map(|refused| refused.to_string()), None, "{file}");
    }
}

#[test]
fn every_bundled_plugin_declares_its_settings_kinds_and_defaults() {
    let data_dir = tempfile::tempdir().unwrap();
    let cache_config = write_cache_config(data_dir.path()).unwrap();
    let servarr = [
        ("url", SettingKind::Url, None),
        ("api_key", SettingKind::Secret, None),
        ("quality_profile", SettingKind::Choice, None),
        ("root_folder", SettingKind::Choice, None),
    ];
    let telegram = [
        ("bot_token", SettingKind::Secret, None),
        ("channel", SettingKind::Text, None),
        (
            "api_url",
            SettingKind::Url,
            Some(text("https://api.telegram.org")),
        ),
    ];

    let expected = |file: &str| match file {
        "radarr.wasm" | "sonarr.wasm" => &servarr[..],
        "telegram.wasm" => &telegram[..],
        _ => &[],
    };

    for (file, wasm) in BUNDLED {
        let fields = load(wasm.to_vec(), &cache_config)
            .unwrap()
            .manifest
            .settings
            .fields;
        let declared: Vec<_> = fields
            .iter()
            .map(|field| (field.key.as_str(), field.kind, field.default.clone()))
            .collect();

        assert_eq!(declared, expected(file), "{file}");
    }
}

fn refused_for_lacking(named: ProvidePoint, export: &str) -> Option<String> {
    let package = Manifest {
        providers: vec![named.into()],
        ..Default::default()
    };
    let points = points(&package).unwrap();
    let refused = check_exports(&points, |exported| exported != export);
    refused.err().map(|refused| refused.to_string())
}

#[test]
fn a_point_missing_one_of_its_exports_refuses_the_plugin_naming_the_export() {
    let refusals = [
        (ProvidePoint::PROVIDE_POINT_NOTIFIER, "deliver"),
        (ProvidePoint::PROVIDE_POINT_SHOW_FULFILLER, "unfollow"),
        (ProvidePoint::PROVIDE_POINT_CONNECTOR, "poll_connection"),
    ]
    .map(|(named, export)| refused_for_lacking(named, export));

    assert_eq!(
        refusals,
        [
            "its Manifest names PROVIDE_POINT_NOTIFIER, and it has no deliver export",
            "its Manifest names PROVIDE_POINT_SHOW_FULFILLER, and it has no unfollow export",
            "its Manifest names PROVIDE_POINT_CONNECTOR, and it has no poll_connection export",
        ]
        .map(|refusal| Some(refusal.to_owned()))
    );
}

// The Kernel asks only a Show Fulfiller to unfollow.
#[test]
fn a_movie_fulfiller_needs_no_unfollow_export() {
    let refused = refused_for_lacking(ProvidePoint::PROVIDE_POINT_MOVIE_FULFILLER, "unfollow");

    assert_eq!(refused, None);
}

#[test]
fn a_fulfiller_needs_no_stop_monitoring_export() {
    let refused = [
        ProvidePoint::PROVIDE_POINT_MOVIE_FULFILLER,
        ProvidePoint::PROVIDE_POINT_SHOW_FULFILLER,
    ]
    .map(|named| refused_for_lacking(named, "stop_monitoring"));

    assert_eq!(refused, [None, None]);
}

#[tokio::test]
async fn a_fulfiller_without_the_stop_monitoring_export_isnt_asked() {
    let data_dir = tempfile::tempdir().unwrap();
    let cache_config = write_cache_config(data_dir.path()).unwrap();
    let (_, wasm) = BUNDLED
        .iter()
        .find(|(file, _)| *file == "radarr.wasm")
        .unwrap();
    let radarr = load(wasm.to_vec(), &cache_config).unwrap();
    // Without its settings Radarr fails when asked, so only one not asked succeeds.
    let running_with = |stops_monitoring| Running {
        package: Arc::clone(&radarr.package),
        compiled: Arc::new(compile(wasm, &cache_config, Vec::new(), Vec::new()).unwrap()),
        secrets: Arc::default(),
        crashes: Arc::default(),
        stops_monitoring,
    };
    let alien = StopMonitoring {
        title: Some(Title::from(MovieDeleted {
            ids: vec![TitleId {
                namespace: "tmdb".to_owned(),
                value: "348".to_owned(),
                ..Default::default()
            }],
            ..Default::default()
        })),
        ..Default::default()
    };
    let stop = async |running: Running| {
        Fulfiller::<Movies>::stop_monitoring(&running, &alien)
            .await
            .is_ok()
    };

    assert!(radarr.stops_monitoring);
    assert!(!stop(running_with(true)).await);
    assert!(stop(running_with(false)).await);
}

#[test]
fn a_point_this_server_doesnt_know_refuses_the_plugin() {
    let unknown_provider = Manifest {
        providers: vec![ProvidePoint::PROVIDE_POINT_NOTIFIER.into(), 99.into()],
        ..Default::default()
    };
    let unspecified_observer = Manifest {
        observers: vec![ObservePoint::OBSERVE_POINT_UNSPECIFIED.into()],
        ..Default::default()
    };

    let refusals = [unknown_provider, unspecified_observer]
        .map(|package| points(&package).err().map(|refused| refused.to_string()));

    assert_eq!(
        refusals,
        [
            "its Manifest names provider 99, an Extension Point this Server doesn't know",
            "its Manifest names observer OBSERVE_POINT_UNSPECIFIED, an Extension Point this Server doesn't know",
        ]
        .map(|refusal| Some(refusal.to_owned()))
    );
}

#[test]
fn register_says_when_the_package_wont_compile_with_its_settings() {
    let data_dir = tempfile::tempdir().unwrap();
    let cache_config = write_cache_config(data_dir.path()).unwrap();
    let (_, wasm) = BUNDLED
        .iter()
        .find(|(file, _)| *file == "radarr.wasm")
        .unwrap();
    let radarr = load(wasm.to_vec(), &cache_config).unwrap();
    let mut broken = radarr.clone();
    broken.wasm = Arc::from(&b"not a module"[..]);
    let register = |plugin: &dyn Plugin| {
        Plugin::register(plugin, &mut Registrar::with_settings(Settings::default()))
    };

    // The intact Package registers, so only compiling the broken one fails.
    register(&radarr).unwrap();
    let err = register(&broken).unwrap_err().to_string();
    assert!(
        err.starts_with("could not compile it with its settings"),
        "{err}"
    );
}
