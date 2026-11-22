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

#[test]
fn the_environment_holds_the_signing_keys_as_pem_blocks() {
    let pems = "-----BEGIN PRIVATE KEY-----\n\
                MC4CAQAwBQYDK2VwBCIEIJzJDsQn3SpQXPoFGl74lUI4L+huggYWuNcSt9vaCwTS\n\
                -----END PRIVATE KEY-----\n";
    Jail::expect_with(|jail| {
        jail.set_env("NAMA_HUB_DATABASE_URL", "postgres://hub@db/hub");
        jail.set_env("NAMA_HUB_SIGNING_KEYS", pems);

        let config = Config::load().unwrap();

        nama_hub::signing::Keys::from_pems(&config.signing_keys).unwrap();
        assert!(!format!("{config:?}").contains("MC4C"));
        Ok(())
    });
}

#[test]
fn a_resend_key_without_the_public_url_or_sender_refuses_to_load_naming_it() {
    Jail::expect_with(|jail| {
        jail.set_env("NAMA_HUB_DATABASE_URL", "postgres://hub@db/hub");
        jail.set_env("NAMA_HUB_RESEND_API_KEY", "re_secret");
        jail.set_env("NAMA_HUB_MAIL_FROM", "Nama Hub <hub@example.com>");

        let err = Config::load().unwrap_err();

        assert!(err.to_string().contains("NAMA_HUB_PUBLIC_URL"), "{err}");
        jail.set_env("NAMA_HUB_PUBLIC_URL", "https://hub.example.com");
        let config = Config::load().unwrap();
        assert_eq!(
            config.public_url.as_deref(),
            Some("https://hub.example.com")
        );
        assert!(!format!("{config:?}").contains("re_secret"));
        Ok(())
    });
}

#[test]
fn without_a_resend_key_or_with_an_empty_one_the_hub_needs_no_mail_settings() {
    Jail::expect_with(|jail| {
        jail.set_env("NAMA_HUB_DATABASE_URL", "postgres://hub@db/hub");
        assert_eq!(Config::load().unwrap().resend_api_key, None);

        jail.set_env("NAMA_HUB_RESEND_API_KEY", "");

        let config = Config::load().unwrap();

        assert_eq!(config.resend_api_key, None);
        Ok(())
    });
}
