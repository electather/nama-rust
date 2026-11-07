//! Loading the Server config from defaults, a file and the environment.
#![expect(
    clippy::result_large_err,
    reason = "`figment::Jail` closures must return `figment::Result`, whose error is large; fine in tests"
)]

use std::{collections::BTreeMap, path::PathBuf};

use figment::Jail;
use nama_plugin_api::settings::SettingValue;
use nama_server::config::{Config, PluginConfig};

#[test]
fn defaults_apply_when_there_is_no_config_file_or_environment() {
    Jail::expect_with(|_jail| {
        let config = Config::load(None).unwrap();

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

        let config = Config::load(Some(&jail.directory().join("server.toml"))).unwrap();

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

        let config = Config::load(Some(&jail.directory().join("server.toml"))).unwrap();

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

        let config = Config::load(None).unwrap();

        assert_eq!(config.data_dir, PathBuf::from("/opt/nama"));
        assert_eq!(config.listen, "0.0.0.0:7980".parse().unwrap());
        Ok(())
    });
}

#[test]
fn the_server_name_comes_from_the_config_file_or_environment() {
    Jail::expect_with(|jail| {
        jail.create_file("nama.toml", r#"name = "Living Room""#)?;
        assert_eq!(Config::load(None).unwrap().name, "Living Room");

        jail.set_env("NAMA_NAME", "Basement");
        assert_eq!(Config::load(None).unwrap().name, "Basement");
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

        let config = Config::load(None).unwrap();

        assert_eq!(
            config.plugins,
            BTreeMap::from([
                (
                    "log-events".to_owned(),
                    PluginConfig {
                        enabled: false,
                        ..PluginConfig::default()
                    }
                ),
                ("other".to_owned(), PluginConfig::default()),
            ])
        );
        assert!(Config::default().plugins.is_empty());
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

        let config = Config::load(None).unwrap();

        assert_eq!(
            config.plugins["tmdb"],
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
fn trusted_proxies_are_ip_addresses_or_cidr_ranges() {
    Jail::expect_with(|jail| {
        jail.create_file("nama.toml", r#"trusted_proxies = ["10.0.0.0/8", "::1"]"#)?;

        let config = Config::load(None).unwrap();

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
