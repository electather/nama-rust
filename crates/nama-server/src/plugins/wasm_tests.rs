use std::{collections::HashSet, fs, path::Path, sync::Arc};

use buffa::Message;

use nama_plugin_api::{
    Plugin, PluginId, PointKey, Registrar,
    fulfiller::{
        Fulfiller, MovieDeleted, Movies, Shows, StopMonitoring, TitleId, stop_monitoring::Title,
    },
    notifier::{Notification, Notifier, Recipient},
    settings::{SettingField, SettingKind, SettingValue, Settings, SettingsSchema},
};
use nama_plugin_proto::{
    CONTRACT_VERSION, SERVICES,
    nama::plugin::v1::{self as wire, Manifest, setting_value},
    services,
};

use super::{
    Running, allowed_hosts,
    bundled::{BUNDLED, bundling},
    check_exports, compile, delivery, extism_config, is_crash,
    load::{check_allowed_hosts, decode, judge, load, settings_schema, write_cache_config},
    points,
    services::POINTS,
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

/// The Extension Point `service` at the versions `version`, as a Manifest names it.
fn point(service: &str, version: &str) -> wire::Point {
    wire::Point {
        service: service.to_owned(),
        version: version.to_owned(),
        ..Default::default()
    }
}

/// A Manifest this Server takes, for a Notifier with a logo, before each test changes it.
fn notifier() -> Manifest {
    Manifest {
        id: "test-plugin".to_owned(),
        name: "Test Plugin".to_owned(),
        description: "Stands in for an Installed Plugin.".to_owned(),
        version: "0.1.0".to_owned(),
        contract: format!("^{CONTRACT_VERSION}"),
        points: vec![point(services::NOTIFIER, "^1.0.0")],
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
    (judged, fs::read_to_string(log.path()).unwrap())
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
fn a_manifest_naming_served_versions_and_well_formed_namespaces_is_taken() {
    let mut package = Manifest {
        points: vec![
            point(services::MOVIE_FULFILLER, "^1"),
            point(services::SHOW_FULFILLER, ">=1.0.0, <2"),
        ],
        movie_id_namespaces: vec!["tmdb".to_owned(), "anidb2".to_owned()],
        show_id_namespaces: vec!["tvdb".to_owned()],
        ..notifier()
    };

    let manifest = judge(&mut package, &["fulfill", "progress", "unfollow"]).unwrap();

    assert_eq!(
        manifest.extension_points,
        [
            PointKey::provide::<dyn Fulfiller<Movies>>(),
            PointKey::provide::<dyn Fulfiller<Shows>>(),
        ]
    );
}

#[test]
fn every_point_this_server_serves_is_a_service_the_contract_versions() {
    let mut served: Vec<_> = POINTS.iter().map(|point| point.service).collect();
    served.sort_unstable();
    let mut versioned: Vec<_> = SERVICES.iter().map(|&(service, _)| service).collect();
    versioned.sort_unstable();

    assert_eq!(served, versioned);
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
            "its Plugin contract requirement \"one\" isn't a semver requirement: \
             unexpected character 'o' while parsing major version number"
                .to_owned(),
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
            points: vec![point("nama.plugin.v1.TeleporterService", "^1.0.0")],
            ..notifier()
        },
        Manifest {
            points: vec![point(services::NOTIFIER, "^2.0.0")],
            ..notifier()
        },
        Manifest {
            points: vec![point(services::NOTIFIER, "one")],
            ..notifier()
        },
        Manifest {
            movie_id_namespaces: vec!["tmdb".to_owned(), "TMDB".to_owned()],
            ..notifier()
        },
        Manifest {
            show_id_namespaces: vec!["tv-db".to_owned()],
            ..notifier()
        },
        Manifest {
            points: vec![point(services::CONNECTOR, "^1.0.0")],
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
            "its allowed host sonarr.lan:8989 has a port, and allowed hosts are matched without one"
                .to_owned(),
            "its Manifest names nama.plugin.v1.TeleporterService, \
             an Extension Point this Server doesn't serve"
                .to_owned(),
            "it wants nama.plugin.v1.NotifierService ^2.0.0, and this Server serves 1.2.0"
                .to_owned(),
            "it wants nama.plugin.v1.NotifierService one, and this Server serves 1.2.0".to_owned(),
            r#"its movie_id_namespaces names "TMDB", which isn't a namespace"#.to_owned(),
            r#"its show_id_namespaces names "tv-db", which isn't a namespace"#.to_owned(),
            "its Manifest names nama.plugin.v1.ConnectorService, \
             and it has no start_connection export"
                .to_owned(),
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

// Adding a Plugin is adding its folder (`plugins/AGENTS.md`), so its exact settings are tested in
// its own crate, and here only what the Kernel needs of every one.
#[test]
fn every_plugins_folder_is_bundled_and_loads_with_defaults_of_their_kinds_and_unique_keys() {
    let data_dir = tempfile::tempdir().unwrap();
    let cache_config = write_cache_config(data_dir.path()).unwrap();
    let folders = bundling::bundled(Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../plugins"
    )))
    .unwrap();
    let folders: Vec<_> = folders.iter().map(|name| format!("{name}.wasm")).collect();

    assert_eq!(
        folders,
        BUNDLED.iter().map(|(file, _)| *file).collect::<Vec<_>>()
    );
    for (file, wasm) in BUNDLED {
        let fields = load(wasm.to_vec(), &cache_config)
            .unwrap()
            .manifest
            .settings
            .fields;
        let keys: HashSet<_> = fields.iter().map(|field| &field.key).collect();

        assert_eq!(keys.len(), fields.len(), "{file} repeats a key");
        for field in &fields {
            let default = field.default.as_ref();
            assert!(
                default.is_none_or(|default| default.is(field.kind)),
                "{file} {}",
                field.key
            );
        }
    }
}

fn refused_for_lacking(service: &str, export: &str) -> Option<String> {
    let package = Manifest {
        points: vec![point(service, "^1.0.0")],
        ..Default::default()
    };
    let points = points(&package).unwrap();
    let refused = check_exports(&points, |exported| exported != export);
    refused.err().map(|refused| refused.to_string())
}

#[test]
fn a_point_missing_one_of_its_exports_refuses_the_plugin_naming_the_export() {
    let refusals = [
        (services::NOTIFIER, "deliver"),
        (services::SHOW_FULFILLER, "unfollow"),
        (services::CONNECTOR, "poll_connection"),
    ]
    .map(|(named, export)| refused_for_lacking(named, export));

    assert_eq!(
        refusals,
        [
            "its Manifest names nama.plugin.v1.NotifierService, and it has no deliver export",
            "its Manifest names nama.plugin.v1.ShowFulfillerService, and it has no unfollow export",
            "its Manifest names nama.plugin.v1.ConnectorService, \
             and it has no poll_connection export",
        ]
        .map(|refusal| Some(refusal.to_owned()))
    );
}

// The Kernel asks only a Show Fulfiller to unfollow.
#[test]
fn a_movie_fulfiller_needs_no_unfollow_export() {
    let refused = refused_for_lacking(services::MOVIE_FULFILLER, "unfollow");

    assert_eq!(refused, None);
}

#[test]
fn a_fulfiller_needs_no_stop_monitoring_export() {
    let refused = [services::MOVIE_FULFILLER, services::SHOW_FULFILLER]
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
    let running_with = |exports: &[&'static str]| Running {
        package: Arc::clone(&radarr.package),
        compiled: Arc::new(compile(wasm, &cache_config, Vec::new(), Vec::new()).unwrap()),
        secrets: Arc::default(),
        crashes: Arc::default(),
        exports: Arc::new(exports.iter().copied().collect()),
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

    assert!(radarr.exports.contains("stop_monitoring"));
    assert!(!stop(running_with(&["stop_monitoring"])).await);
    assert!(stop(running_with(&[])).await);
}

#[test]
fn a_fulfiller_needs_the_id_namespaces_its_manifest_names() {
    let data_dir = tempfile::tempdir().unwrap();
    let cache_config = write_cache_config(data_dir.path()).unwrap();
    let (_, wasm) = BUNDLED
        .iter()
        .find(|(file, _)| *file == "radarr.wasm")
        .unwrap();
    let compiled = Arc::new(compile(wasm, &cache_config, Vec::new(), Vec::new()).unwrap());
    let running = |package| Running {
        package: Arc::new(package),
        compiled: Arc::clone(&compiled),
        secrets: Arc::default(),
        crashes: Arc::default(),
        exports: Arc::default(),
    };
    let naming = running(Manifest {
        movie_id_namespaces: vec!["tmdb".to_owned(), "imdb".to_owned()],
        ..Default::default()
    });

    assert_eq!(Fulfiller::<Movies>::needs(&naming), ["tmdb", "imdb"]);
    assert_eq!(Fulfiller::<Shows>::needs(&naming), [""; 0]);
    assert_eq!(
        Fulfiller::<Movies>::needs(&running(Manifest::default())),
        [""; 0]
    );
}

#[test]
fn radarr_needs_a_tmdb_id_and_sonarr_a_tvdb_id() {
    let data_dir = tempfile::tempdir().unwrap();
    let cache_config = write_cache_config(data_dir.path()).unwrap();
    let package = |name: &str| {
        let (_, wasm) = BUNDLED.iter().find(|(file, _)| *file == name).unwrap();
        load(wasm.to_vec(), &cache_config).unwrap().package
    };

    assert_eq!(package("radarr.wasm").movie_id_namespaces, ["tmdb"]);
    assert_eq!(package("sonarr.wasm").show_id_namespaces, ["tvdb"]);
}

#[test]
fn a_point_this_server_doesnt_know_refuses_the_plugin() {
    let unknown_after_a_known_one = Manifest {
        points: vec![
            point(services::NOTIFIER, "^1.0.0"),
            point("nama.plugin.v1.TeleporterService", "^1.0.0"),
        ],
        ..Default::default()
    };
    // A service is named by its full name alone.
    let short_name = Manifest {
        points: vec![point("NotifierService", "^1.0.0")],
        ..Default::default()
    };

    let refusals = [unknown_after_a_known_one, short_name]
        .map(|package| points(&package).err().map(|refused| refused.to_string()));

    assert_eq!(
        refusals,
        [
            "its Manifest names nama.plugin.v1.TeleporterService, \
             an Extension Point this Server doesn't serve",
            "its Manifest names NotifierService, an Extension Point this Server doesn't serve",
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

fn notification() -> Notification {
    Notification {
        request_url: Some("https://nama.lan/requests/1".to_owned()),
        ..Default::default()
    }
}

#[test]
fn a_user_recipient_carries_exactly_their_personal_settings() {
    let settings = [("topic", "alerts"), ("webhook", "https://discord.lan/hook")]
        .map(|(key, value)| (key.to_owned(), value.to_owned()));
    let sent = delivery(&notification(), &Recipient::User(settings.clone().into()));

    assert_eq!(sent.notification.as_option(), Some(&notification()));
    let Some(wire::delivery::Recipient::User(user)) = sent.recipient else {
        panic!("not a User recipient: {:?}", sent.recipient);
    };
    let mut got: Vec<_> = user.personal_settings.into_iter().collect();
    got.sort();
    assert_eq!(got, settings);
}

#[test]
fn a_connection_or_channel_recipient_keeps_its_kind() {
    let connection = delivery(&notification(), &Recipient::Connection("42".to_owned()));
    assert_eq!(
        connection.recipient,
        Some(wire::delivery::Recipient::Connection("42".to_owned()))
    );
    let channel = delivery(&notification(), &Recipient::Channel);
    assert!(matches!(
        channel.recipient,
        Some(wire::delivery::Recipient::Channel(_))
    ));
}
