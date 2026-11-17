//! Loading the Server config from defaults, a file and the environment.
#![expect(
    clippy::result_large_err,
    reason = "`figment::Jail` closures must return `figment::Result`, whose error is large; fine in tests"
)]

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use figment::Jail;
use nama_plugin_api::settings::SettingValue;
use nama_server::config::{Config, PluginConfig};
use schemars::generate::SchemaSettings;
use serde_json::Value;

#[test]
fn defaults_apply_when_there_is_no_config_file_or_environment() {
    Jail::expect_with(|_jail| {
        let config = Config::load(None).unwrap().0;

        assert_eq!(config.name, "Nama");
        assert_eq!(config.listen, "0.0.0.0:7980".parse().unwrap());
        assert_eq!(config.data_dir, PathBuf::from("data"));
        assert_eq!(config.trusted_proxies, []);
        Ok(())
    });
}

#[test]
fn a_config_file_sets_listen_address_and_data_directory() {
    Jail::expect_with(|jail| {
        jail.create_file(
            "server.toml",
            r#"
                listen = "127.0.0.1:9000"
                data_dir = "/var/lib/nama"
            "#,
        )?;

        let config = Config::load(Some(&jail.directory().join("server.toml")))
            .unwrap()
            .0;

        assert_eq!(config.listen, "127.0.0.1:9000".parse().unwrap());
        assert_eq!(config.data_dir, PathBuf::from("/var/lib/nama"));
        Ok(())
    });
}

#[test]
fn nama_environment_variables_override_the_config_file() {
    Jail::expect_with(|jail| {
        jail.create_file(
            "server.toml",
            r#"
                listen = "127.0.0.1:9000"
                data_dir = "/var/lib/nama"
            "#,
        )?;
        jail.set_env("NAMA_LISTEN", "[::1]:9100");
        jail.set_env("NAMA_DATA_DIR", "/srv/nama");

        let config = Config::load(Some(&jail.directory().join("server.toml")))
            .unwrap()
            .0;

        assert_eq!(config.listen, "[::1]:9100".parse().unwrap());
        assert_eq!(config.data_dir, PathBuf::from("/srv/nama"));
        Ok(())
    });
}

#[test]
fn a_named_config_file_that_does_not_exist_is_an_error() {
    Jail::expect_with(|jail| {
        let missing = jail.directory().join("missing.toml");

        Config::load(Some(&missing)).unwrap_err();
        Ok(())
    });
}

#[test]
fn without_a_named_file_nama_toml_in_the_working_directory_is_read() {
    Jail::expect_with(|jail| {
        jail.create_file("nama.toml", r#"data_dir = "/opt/nama""#)?;

        let config = Config::load(None).unwrap().0;

        assert_eq!(config.data_dir, PathBuf::from("/opt/nama"));
        assert_eq!(config.listen, "0.0.0.0:7980".parse().unwrap());
        Ok(())
    });
}

#[test]
fn the_server_name_comes_from_the_config_file_or_environment() {
    Jail::expect_with(|jail| {
        jail.create_file("nama.toml", r#"name = "Living Room""#)?;
        assert_eq!(Config::load(None).unwrap().0.name, "Living Room");

        jail.set_env("NAMA_NAME", "Basement");
        assert_eq!(Config::load(None).unwrap().0.name, "Basement");
        Ok(())
    });
}

#[test]
fn ffmpeg_and_ffprobe_paths_come_from_the_config_file_or_environment() {
    Jail::expect_with(|jail| {
        let defaults = Config::load(None).unwrap().0;
        assert_eq!((defaults.ffmpeg, defaults.ffprobe), (None, None));

        jail.create_file(
            "nama.toml",
            r#"
                ffmpeg = "/opt/ffmpeg/ffmpeg"
                ffprobe = "/opt/ffmpeg/ffprobe"
            "#,
        )?;
        jail.set_env("NAMA_FFPROBE", "/usr/lib/jellyfin-ffmpeg/ffprobe");
        let config = Config::load(None).unwrap().0;

        assert_eq!(config.ffmpeg, Some(PathBuf::from("/opt/ffmpeg/ffmpeg")));
        assert_eq!(
            config.ffprobe,
            Some(PathBuf::from("/usr/lib/jellyfin-ffmpeg/ffprobe"))
        );
        Ok(())
    });
}

#[test]
fn the_config_file_can_turn_a_plugin_off() {
    Jail::expect_with(|jail| {
        jail.create_file(
            "nama.toml",
            "
                [plugins.log-events]
                enabled = false

                [plugins.other]
            ",
        )?;

        let config = Config::load(None).unwrap().0;

        assert_eq!(
            config.plugins.parse().unwrap(),
            BTreeMap::from([
                (
                    "log-events".to_owned(),
                    PluginConfig {
                        enabled: false,
                        settings: BTreeMap::new(),
                    }
                ),
                // A table with no `enabled` line imports as enabled.
                (
                    "other".to_owned(),
                    PluginConfig {
                        enabled: true,
                        settings: BTreeMap::new(),
                    }
                ),
            ])
        );
        assert_eq!(Config::default().plugins.ids().count(), 0);
        Ok(())
    });
}

#[test]
fn a_plugins_other_keys_are_its_settings() {
    Jail::expect_with(|jail| {
        jail.create_file(
            "nama.toml",
            r#"
                [plugins.tmdb]
                api_key = "secret"
                adult = false
                retries = 3
            "#,
        )?;

        let config = Config::load(None).unwrap().0;

        assert_eq!(
            config.plugins.parse().unwrap()["tmdb"],
            PluginConfig {
                enabled: true,
                settings: BTreeMap::from([
                    ("adult".to_owned(), SettingValue::Bool(false)),
                    (
                        "api_key".to_owned(),
                        SettingValue::Text("secret".to_owned())
                    ),
                    ("retries".to_owned(), SettingValue::Integer(3)),
                ]),
            }
        );
        Ok(())
    });
}

#[test]
fn a_plugin_table_with_a_wrong_value_loads_and_fails_only_when_parsed() {
    Jail::expect_with(|jail| {
        jail.create_file(
            "nama.toml",
            r#"
                [plugins.tmdb]
                api_key = "secret"
                retries = 1.5
            "#,
        )?;

        let config = Config::load(None).unwrap().0;

        assert_eq!(config.plugins.ids().collect::<Vec<_>>(), ["tmdb"]);
        let err = config.plugins.parse().unwrap_err().to_string();
        assert!(err.contains("[plugins.tmdb]"), "{err}");
        assert!(!format!("{config:?}").contains("secret"), "{config:?}");
        Ok(())
    });
}

#[test]
fn trusted_proxies_are_ip_addresses_or_cidr_ranges() {
    Jail::expect_with(|jail| {
        jail.create_file(
            "nama.toml",
            r#"trusted_proxies = ["10.0.0.0/8", "::1", "::ffff:192.168.0.0/112"]"#,
        )?;

        let config = Config::load(None).unwrap().0;

        let trusted = |ip: &str| {
            let ip = ip.parse().unwrap();
            config
                .trusted_proxies
                .iter()
                .any(|proxies| proxies.contains(ip))
        };
        assert!(trusted("10.20.30.40"));
        assert!(
            trusted("::ffff:10.0.0.1"),
            "an IPv4 peer of a dual-stack socket"
        );
        assert!(trusted("::1"));
        assert!(
            trusted("192.168.4.5"),
            "a range written as IPv4 mapped into IPv6"
        );
        assert!(!trusted("192.169.0.1"));
        assert!(!trusted("11.0.0.1"));
        assert!(!trusted("::2"));
        Ok(())
    });
}

#[test]
fn a_trusted_proxy_that_isnt_an_ip_address_or_cidr_range_is_an_error() {
    Jail::expect_with(|jail| {
        jail.create_file("nama.toml", r#"trusted_proxies = ["10.0.0.0/33"]"#)?;

        Config::load(None).unwrap_err();
        Ok(())
    });
}

#[test]
fn an_unknown_key_in_the_config_file_is_reported_by_name_and_ignored() {
    Jail::expect_with(|jail| {
        jail.create_file(
            "nama.toml",
            r#"
                watch_folder = false
                api_key = "secret"

                [plugins.tmdb]
                anything = "goes"
            "#,
        )?;

        let (config, unknown_keys) = Config::load(None).unwrap();

        assert!(config.watch_folders);
        assert_eq!(unknown_keys, ["api_key", "watch_folder"]);
        Ok(())
    });
}

#[test]
fn an_unrelated_nama_environment_variable_is_not_reported() {
    Jail::expect_with(|jail| {
        jail.set_env("NAMA_SERVER_URL", "http://localhost:7980");

        let (_, unknown_keys) = Config::load(None).unwrap();

        assert!(unknown_keys.is_empty(), "{unknown_keys:?}");
        Ok(())
    });
}

/// The schema of `nama.toml`, as draft-07 for Taplo's validator.
fn schema() -> Value {
    SchemaSettings::draft07()
        .into_generator()
        .into_root_schema_for::<Config>()
        .to_value()
}

#[test]
fn the_schema_requires_no_key_and_rejects_unknown_ones() {
    let schema = schema();

    assert_eq!(schema.get("required"), None);
    assert_eq!(schema["additionalProperties"], false);
}

#[test]
fn the_schema_types_trusted_proxies_as_strings_and_plugin_tables_as_free_form() {
    let schema = schema();

    assert_eq!(
        schema["properties"]["trusted_proxies"]["items"]["type"],
        "string"
    );
    let plugin_table = &schema["properties"]["plugins"]["additionalProperties"];
    assert_eq!(plugin_table["type"], "object");
    assert_eq!(plugin_table["properties"]["enabled"]["type"], "boolean");
    assert_eq!(plugin_table.get("required"), None);
}

#[test]
fn the_committed_schema_matches_config() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/nama.schema.json");
    let generated = schema();
    // Compared parsed, since a Windows checkout may turn the file's line endings into CRLF.
    let committed: Option<Value> = fs::read_to_string(&path)
        .ok()
        .and_then(|file| serde_json::from_str(&file).ok());

    if committed.as_ref() != Some(&generated) {
        // Rewritten so a local rerun passes ("Config schema", `crates/nama-server/AGENTS.md`).
        fs::write(
            &path,
            serde_json::to_string_pretty(&generated).unwrap() + "\n",
        )
        .unwrap();
        panic!(
            "{} was out of date and is now regenerated; commit it",
            path.display()
        );
    }
}
