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
        Buffa, FnResult, Manifest, extism_pdk,
        nama_proto::nama::plugin::v1::{
            ObservePoint, ObservedEvents, SettingField, SettingKind, SettingValue, setting_value,
        },
        plugin_fn,
    };

    /// The setting that picks what its Observer does: `log`, `loop` or `panic`.
    const OBSERVER: &str = "observer";

    // Says who the Plugin is, that it observes Events, and that its `observer` setting picks how.
    // With the `out-of-range` feature it wants a Nama API no Server has, and with `panic` it
    // panics instead.
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
                settings: vec![observer_setting()],
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

    fn observer_setting() -> SettingField {
        SettingField {
            key: OBSERVER.to_owned(),
            label: "Observer".to_owned(),
            description: "What it does when told about Events: log, loop or panic.".to_owned(),
            kind: SettingKind::SETTING_KIND_TEXT.into(),
            default_value: SettingValue {
                value: Some(setting_value::Value::Text("log".to_owned())),
                ..Default::default()
            }
            .into(),
            ..Default::default()
        }
    }

    // Logs each Event it is told about, so a test can see them arrive. Its `observer` setting can
    // make it loop forever or panic instead, so a test can see the Server survive that.
    #[plugin_fn]
    pub fn observe_events(Buffa(observed): Buffa<ObservedEvents>) -> FnResult<()> {
        match extism_pdk::config::get(OBSERVER)?.as_deref() {
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
                Ok(())
            }
        }
    }
}
