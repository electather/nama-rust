//! An Installed Plugin used only in the Server's tests. Build it with
//! `cargo build -p nama-test-plugin --target wasm32-unknown-unknown`.

/// The Plugin's exports. `plugin_fn` drops their doc comments and attributes, so lints about the
/// code it generates are expected here.
#[expect(
    clippy::unnecessary_wraps,
    reason = "an export returns `FnResult` even when it can't fail"
)]
mod exports {
    use nama_pdk::{
        Buffa, FnResult, Manifest,
        extism_pdk::{self, HttpRequest, config, http},
        nama_proto::nama::plugin::v1::{
            ObservePoint, ObservedEvents, SettingField, SettingKind, SettingValue, setting_value,
        },
        plugin_fn,
    };

    /// The setting that picks what its Observer does: `log`, `loop` or `panic`.
    const OBSERVER: &str = "observer";

    // Says who the Plugin is, that it observes Events, and its settings: a URL whose host it may
    // reach, one it requests, and how its Observer behaves. With the `out-of-range` feature it
    // wants a Nama API no Server has, and with `panic` it panics instead.
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
                observers: vec![ObservePoint::OBSERVE_POINT_EVENTS.into()],
                settings: vec![
                    setting("url", SettingKind::SETTING_KIND_URL),
                    setting("fetch", SettingKind::SETTING_KIND_TEXT),
                    observer_setting(),
                ],
                ..nama_pdk::manifest("test-plugin", env!("CARGO_PKG_VERSION"))
            }
        };
        #[cfg(feature = "out-of-range")]
        let manifest = Manifest {
            nama_api: "<0.1".to_owned(),
            ..manifest
        };
        Ok(Buffa(manifest))
    }

    fn setting(key: &str, kind: SettingKind) -> SettingField {
        SettingField {
            key: key.to_owned(),
            label: key.to_owned(),
            kind: kind.into(),
            ..SettingField::default()
        }
    }

    fn observer_setting() -> SettingField {
        SettingField {
            label: "Observer".to_owned(),
            description: "What it does when told about Events: log, loop or panic.".to_owned(),
            default_value: SettingValue {
                value: Some(setting_value::Value::Text("log".to_owned())),
                ..Default::default()
            }
            .into(),
            ..setting(OBSERVER, SettingKind::SETTING_KIND_TEXT)
        }
    }

    // Logs each Event it is told about, so a test can see them arrive, then requests its `fetch`
    // setting's URL, if it has one, and logs the response's status. Its `observer` setting can
    // make it loop forever or panic instead, so a test can see the Server survive that.
    #[plugin_fn]
    pub fn observe_events(Buffa(observed): Buffa<ObservedEvents>) -> FnResult<()> {
        match config::get(OBSERVER)?.as_deref() {
            #[expect(clippy::infinite_loop, reason = "it loops forever on purpose")]
            Some("loop") => {
                extism_pdk::info!("test-plugin is looping");
                loop {
                    std::hint::spin_loop();
                }
            }
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
}
