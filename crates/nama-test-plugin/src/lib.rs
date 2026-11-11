//! An Installed Plugin used only in the Server's tests. Build it with
//! `cargo build -p nama-test-plugin --target wasm32-unknown-unknown`.

/// The Plugin's exports. `plugin_fn` drops their doc comments and attributes, so lints about the
/// code it generates are expected here.
#[expect(
    clippy::unnecessary_wraps,
    reason = "an export returns `FnResult` even when it can't fail"
)]
mod exports {
    use std::collections::HashMap;

    use nama_pdk::{
        Buffa, CheckedSettings, Download, FnResult, Fulfilled, Fulfiller, Manifest, Notification,
        Notifier, Recipient, Request,
        extism_pdk::{self, Error, HttpRequest, config, http},
        nama_proto::nama::plugin::v1::{
            ObservePoint, ObservedEvents, ProvidePoint, SettingField, SettingKind, SettingValue,
            setting_value,
        },
        plugin_fn,
    };

    /// The setting that picks what its Observer does: `log`, `loop` or `panic`.
    const OBSERVER: &str = "observer";

    /// The setting that picks what its Fulfiller does: `take`, `error`, `loop` or `panic`.
    const FULFILLER: &str = "fulfiller";

    /// The setting that picks what its Notifier does: `post`, `error`, `loop` or `panic`.
    const NOTIFIER: &str = "notifier";

    /// The setting that makes its settings check fail with its text, or loop forever or panic for
    /// `loop` or `panic`.
    const FAIL_WITH: &str = "fail_with";

    // Says who the Plugin is, that it observes Events, fulfills Movie Requests and notifies, and
    // its settings: a URL whose host it may reach, one it requests, how its Observer, Fulfiller
    // and Notifier behave, a profile its check offers, and how its check fails, and its logo. With
    // the `out-of-range` feature it wants a Nama API no Server has, with `unnamed` it gives no name
    // or description, with `unexported-connector` it names a Connector it doesn't export, and
    // with `panic` it panics instead.
    #[plugin_fn]
    pub fn manifest() -> FnResult<Buffa<Manifest>> {
        #[expect(
            clippy::panic,
            reason = "with the `panic` feature it panics on purpose"
        )]
        let manifest = if cfg!(feature = "panic") {
            panic!("the test Plugin was built to panic")
        } else {
            Manifest {
                name: "Test Plugin".to_owned(),
                description: "Stands in for an Installed Plugin in the Server's tests.".to_owned(),
                observers: vec![ObservePoint::OBSERVE_POINT_EVENTS.into()],
                providers: vec![
                    ProvidePoint::PROVIDE_POINT_MOVIE_FULFILLER.into(),
                    ProvidePoint::PROVIDE_POINT_NOTIFIER.into(),
                ],
                settings: vec![
                    setting("url", SettingKind::SETTING_KIND_URL),
                    setting("fetch", SettingKind::SETTING_KIND_TEXT),
                    behaviour_setting(
                        OBSERVER,
                        "log",
                        "when told about Events: log, loop or panic",
                    ),
                    behaviour_setting(
                        FULFILLER,
                        "take",
                        "when handed a Request: take, error, loop or panic",
                    ),
                    behaviour_setting(
                        NOTIFIER,
                        "post",
                        "when handed a notification: post, error, loop or panic",
                    ),
                    setting("profile", SettingKind::SETTING_KIND_CHOICE),
                    setting(FAIL_WITH, SettingKind::SETTING_KIND_TEXT),
                ],
                logo_svg: logo(),
                ..nama_pdk::manifest("test-plugin", env!("CARGO_PKG_VERSION"))
            }
        };
        #[cfg(feature = "out-of-range")]
        let manifest = Manifest {
            nama_api: "<0.1".to_owned(),
            ..manifest
        };
        #[cfg(feature = "unexported-connector")]
        let manifest = Manifest {
            providers: vec![ProvidePoint::PROVIDE_POINT_CONNECTOR.into()],
            ..manifest
        };
        #[cfg(feature = "unnamed")]
        let manifest = Manifest {
            name: String::new(),
            description: String::new(),
            ..manifest
        };
        Ok(Buffa(manifest))
    }

    /// Its logo, or with the `big-logo`, `html-logo` or `doctype-logo` feature one the Server
    /// drops, or with `logoless` none.
    fn logo() -> String {
        if cfg!(feature = "big-logo") {
            // Well-formed SVG, so only its size gets it dropped.
            let padding = " ".repeat(16 * 1024);
            format!(r#"<svg xmlns="http://www.w3.org/2000/svg">{padding}</svg>"#)
        } else if cfg!(feature = "html-logo") {
            r#"<html xmlns="http://www.w3.org/1999/xhtml"/>"#.to_owned()
        } else if cfg!(feature = "doctype-logo") {
            format!(
                r#"<!DOCTYPE svg PUBLIC "-//W3C//DTD SVG 1.1//EN" "http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd">{}"#,
                include_str!("../logo.svg")
            )
        } else if cfg!(feature = "logoless") {
            String::new()
        } else {
            include_str!("../logo.svg").to_owned()
        }
    }

    fn setting(key: &str, kind: SettingKind) -> SettingField {
        SettingField {
            key: key.to_owned(),
            label: key.to_owned(),
            kind: kind.into(),
            ..SettingField::default()
        }
    }

    /// The text setting `key`, `default` unless set, that picks what the Plugin does `when`.
    /// Required, so a test sees a default fill a required setting.
    fn behaviour_setting(key: &str, default: &str, when: &str) -> SettingField {
        SettingField {
            description: format!("What it does {when}."),
            required: true,
            default_value: SettingValue {
                value: Some(setting_value::Value::Text(default.to_owned())),
                ..Default::default()
            }
            .into(),
            ..setting(key, SettingKind::SETTING_KIND_TEXT)
        }
    }

    // Logs each Event it is told about, so a test can see them arrive, then requests its `fetch`
    // setting's URL, if it has one, and logs the response's status. Its `observer` setting can
    // make it loop forever or panic instead, so a test can see the Server survive that.
    #[plugin_fn]
    pub fn observe_events(Buffa(observed): Buffa<ObservedEvents>) -> FnResult<()> {
        match config::get(OBSERVER)?.as_deref() {
            Some("loop") => loop_forever(),
            #[expect(clippy::panic, reason = "it panics on purpose")]
            Some("panic") => panic!("the test Plugin's Observer was set to panic"),
            _ => {
                for event in observed.events {
                    extism_pdk::info!("test-plugin observed {event:?}");
                }
                if let Some(url) = config::get("fetch")? {
                    let response = http::request::<()>(&HttpRequest::new(&url), None)?;
                    extism_pdk::info!("test-plugin fetched {url}: {}", response.status_code());
                }
                Ok(())
            }
        }
    }

    fn loop_forever() -> ! {
        extism_pdk::info!("test-plugin is looping");
        loop {
            std::hint::spin_loop();
        }
    }

    /// A Movie Fulfiller that, as its `fulfiller` setting says, takes each Request, giving
    /// `test-<TMDB id>` as its reference, refuses it, loops forever or panics. It says each
    /// reference it is asked about is half downloaded.
    struct TestFulfiller;

    impl Fulfiller for TestFulfiller {
        fn fulfill(request: Request) -> Result<Fulfilled, Error> {
            match config::get(FULFILLER)?.as_deref() {
                Some("error") => Err(Error::msg("the test Plugin's Fulfiller was set to error")),
                Some("loop") => loop_forever(),
                #[expect(clippy::panic, reason = "it panics on purpose")]
                Some("panic") => panic!("the test Plugin's Fulfiller was set to panic"),
                _ => Ok(Fulfilled {
                    reference: format!("test-{}", request.movie.tmdb_id),
                    ..Fulfilled::default()
                }),
            }
        }

        fn progress(references: Vec<String>) -> Result<HashMap<String, Vec<Download>>, Error> {
            let half_done = |reference: String| {
                let download = Download {
                    title: reference.clone(),
                    percent: 50.0,
                    ..Download::default()
                };
                (reference, vec![download])
            };
            Ok(references.into_iter().map(half_done).collect())
        }
    }

    nama_pdk::fulfiller!(TestFulfiller);

    /// A Notifier that, as its `notifier` setting says, posts each notification for its channel,
    /// encoded as protobuf, to its `url` setting's URL if it has one, fails, loops forever or
    /// panics.
    struct TestNotifier;

    impl Notifier for TestNotifier {
        fn deliver(notification: Notification, recipient: Recipient) -> Result<(), Error> {
            let Recipient::Channel(_) = recipient else {
                return Ok(());
            };
            match config::get(NOTIFIER)?.as_deref() {
                Some("error") => Err(Error::msg("the test Plugin's Notifier was set to error")),
                Some("loop") => loop_forever(),
                #[expect(clippy::panic, reason = "it panics on purpose")]
                Some("panic") => panic!("the test Plugin's Notifier was set to panic"),
                _ => post(notification),
            }
        }
    }

    nama_pdk::notifier!(TestNotifier);

    /// Posts `notification`, encoded as protobuf, to its `url` setting's URL, if it has one.
    fn post(notification: Notification) -> Result<(), Error> {
        if let Some(url) = config::get("url")? {
            let request = HttpRequest::new(&url).with_method("POST");
            http::request(&request, Some(Buffa(notification)))?;
        }
        Ok(())
    }

    // Requests its `url` setting's URL, if it has one, as Sonarr's check would, then offers two
    // profiles. Its `fail_with` setting makes it fail, loop forever or panic instead.
    #[plugin_fn]
    pub fn check_settings() -> FnResult<Buffa<CheckedSettings>> {
        match config::get(FAIL_WITH)?.as_deref() {
            Some("loop") => loop_forever(),
            #[expect(clippy::panic, reason = "it panics on purpose")]
            Some("panic") => panic!("the test Plugin's check was set to panic"),
            Some(error) => Err(Error::msg(error.to_owned()).into()),
            None => {
                if let Some(url) = config::get("url")? {
                    http::request::<()>(&HttpRequest::new(&url), None)?;
                }
                let profiles = [("1", "HD"), ("2", "Ultra HD")];
                Ok(Buffa(nama_pdk::checked_settings([("profile", profiles)])))
            }
        }
    }
}
