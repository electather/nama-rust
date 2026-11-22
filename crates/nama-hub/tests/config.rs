//! Loading the Hub config from `NAMA_HUB_*` environment variables.
#![expect(
    clippy::result_large_err,
    reason = "`figment::Jail` closures must return `figment::Result`, whose error is large; fine in tests"
)]

use figment::Jail;
use nama_hub::config::Config;

#[test]
fn the_environment_names_the_database_and_listen_defaults_to_port_7981() {
    Jail::expect_with(|jail| {
        jail.set_env("NAMA_HUB_DATABASE_URL", "postgres://hub@db/hub");

        let config = Config::load().unwrap();

        assert_eq!(config.database_url, "postgres://hub@db/hub");
        assert_eq!(config.listen, "0.0.0.0:7981".parse().unwrap());
        Ok(())
    });
}

#[test]
fn the_environment_sets_the_listen_address() {
    Jail::expect_with(|jail| {
        jail.set_env("NAMA_HUB_DATABASE_URL", "postgres://hub@db/hub");
        jail.set_env("NAMA_HUB_LISTEN", "127.0.0.1:9000");

        assert_eq!(
            Config::load().unwrap().listen,
            "127.0.0.1:9000".parse().unwrap()
        );
        Ok(())
    });
}

#[test]
fn a_missing_database_url_fails_naming_it() {
    Jail::expect_with(|_jail| {
        let err = Config::load().unwrap_err();

        assert!(err.to_string().contains("database_url"), "{err}");
        Ok(())
    });
}

#[test]
fn the_database_url_stays_out_of_debug_output() {
    Jail::expect_with(|jail| {
        jail.set_env("NAMA_HUB_DATABASE_URL", "postgres://hub:secret@db/hub");

        let config = Config::load().unwrap();

        assert!(!format!("{config:?}").contains("secret"));
        Ok(())
    });
}

#[test]
fn the_environment_lists_the_trusted_proxies_and_none_are_trusted_by_default() {
    Jail::expect_with(|jail| {
        jail.set_env("NAMA_HUB_DATABASE_URL", "postgres://hub@db/hub");
        assert_eq!(Config::load().unwrap().trusted_proxies, []);

        jail.set_env("NAMA_HUB_TRUSTED_PROXIES", r#"["10.0.0.0/8", "::1"]"#);

        assert_eq!(
            Config::load().unwrap().trusted_proxies,
            ["10.0.0.0/8".parse().unwrap(), "::1".parse().unwrap()]
        );
        Ok(())
    });
}
