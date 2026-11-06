//! Loading the Server config from defaults, a file and the environment.
#![expect(
    clippy::result_large_err,
    reason = "`figment::Jail` closures must return `figment::Result`, whose error is large; fine in tests"
)]

use std::path::PathBuf;

use figment::Jail;
use nama_server::config::Config;

#[test]
fn defaults_apply_when_there_is_no_config_file_or_environment() {
    Jail::expect_with(|_jail| {
        let config = Config::load(None).unwrap();

        assert_eq!(config.name, "Nama");
        assert_eq!(config.listen, "0.0.0.0:7980".parse().unwrap());
        assert_eq!(config.data_dir, PathBuf::from("data"));
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
