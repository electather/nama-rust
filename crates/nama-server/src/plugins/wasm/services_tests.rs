use std::sync::Arc;

use nama_plugin_api::{PluginError, points::ObservedEvents};
use nama_plugin_proto::{
    SERVICES,
    nama::plugin::v1::{EventsObserved, ScanStarted},
    services,
};

use super::{
    super::{
        Running,
        bundled::BUNDLED,
        compile,
        load::{BLOB_TIMEOUT, load, write_cache_config},
    },
    Failure, Method, OBSERVE_EVENTS, POINTS,
};
use crate::log_capture::CapturedLog;

#[test]
fn a_title_sources_artwork_list_is_an_optional_row_calling_title_artwork() {
    let title_source = POINTS
        .iter()
        .find(|point| point.service == services::TITLE_SOURCE)
        .unwrap();
    let artwork = title_source
        .methods
        .iter()
        .find(|method| method.name == "Artwork")
        .unwrap();

    assert_eq!(artwork.export, "title_artwork");
    assert!(artwork.optional);
}

#[test]
fn a_title_sources_season_artwork_list_is_an_optional_row_calling_title_season_artwork() {
    let title_source = POINTS
        .iter()
        .find(|point| point.service == services::TITLE_SOURCE)
        .unwrap();
    let season_artwork = title_source
        .methods
        .iter()
        .find(|method| method.name == "SeasonArtwork")
        .unwrap();

    assert_eq!(season_artwork.export, "title_season_artwork");
    assert!(season_artwork.optional);
}

#[test]
fn a_tick_is_a_required_row_whose_failure_is_logged_not_shown() {
    let automation = POINTS
        .iter()
        .find(|point| point.service == services::AUTOMATION)
        .unwrap();

    assert!(matches!(
        automation.methods,
        [Method {
            export: "tick",
            optional: false,
            failure: Failure::Observer,
            ..
        }]
    ));
}

#[test]
fn a_blob_store_is_five_required_rows_that_may_run_long_and_fail_to_the_caller() {
    let blob_store = POINTS
        .iter()
        .find(|point| point.service == services::BLOB_STORE)
        .unwrap();
    let exports: Vec<_> = blob_store.required().map(|method| method.export).collect();

    assert_eq!(
        exports,
        [
            "blob_put",
            "blob_get",
            "blob_delete",
            "blob_list",
            "blob_exists"
        ]
    );
    for method in blob_store.methods {
        assert!(
            matches!(method.failure, Failure::Provider),
            "{}",
            method.name
        );
        assert_eq!(method.limit, BLOB_TIMEOUT, "{}", method.name);
    }
    assert!(blob_store.runs_long());
    assert_eq!(blob_store.crash_group, services::BLOB_STORE);
}

#[test]
fn every_contract_service_has_adapter_rows_and_every_row_a_contract_service() {
    for (service, _) in SERVICES {
        let point = POINTS.iter().find(|point| point.service == *service);
        assert!(
            point.is_some_and(|point| !point.methods.is_empty()),
            "{service} has no rows"
        );
    }
    for point in POINTS {
        assert!(
            SERVICES
                .iter()
                .any(|&(service, _)| service == point.service),
            "{} isn't a contract service",
            point.service
        );
        for method in point.methods {
            assert_eq!(method.service, point.service, "{}", method.name);
        }
    }
}

#[tokio::test]
async fn an_observers_failure_doesnt_show_its_secret_settings() {
    let data_dir = tempfile::tempdir().unwrap();
    let cache_config = write_cache_config(data_dir.path()).unwrap();
    let (_, wasm) = BUNDLED
        .iter()
        .find(|(file, _)| *file == "radarr.wasm")
        .unwrap();
    let radarr = load(wasm.to_vec(), &cache_config).unwrap();
    // Radarr has no Observer, so the call fails naming the export, here standing in for a secret.
    let running = Running {
        package: Arc::clone(&radarr.package),
        compiled: Arc::new(compile(wasm, &cache_config, Vec::new(), Vec::new()).unwrap()),
        secrets: vec![OBSERVE_EVENTS.export.to_owned()].into(),
        crashes: Arc::default(),
        exports: Arc::clone(&radarr.exports),
    };

    let failed = running
        .call_method::<_, EventsObserved>(&OBSERVE_EVENTS, &ObservedEvents::default())
        .await
        .unwrap_err();

    assert_eq!(failed.to_string(), "Function not found: …");
}

#[tokio::test]
async fn an_observers_answer_that_wont_decode_is_logged_once_naming_its_plugin() {
    let log = CapturedLog::start();

    call_manifest_decoded_as_a_scan_started(Failure::Observer)
        .await
        .unwrap_err();

    let logged = log.finish();
    let failures = logged.lines().filter(|line| {
        line.contains("plugin=radarr")
            && line.contains("observer failed: its answer didn't decode")
            && line.contains("wire type")
    });
    assert_eq!(failures.count(), 1, "{logged}");
}

#[tokio::test]
async fn a_providers_answer_that_wont_decode_isnt_logged_where_its_called() {
    let log = CapturedLog::start();

    let failed = call_manifest_decoded_as_a_scan_started(Failure::Provider)
        .await
        .unwrap_err();

    assert!(failed.to_string().contains("wire type"), "{failed}");
    assert_eq!(log.finish(), "");
}

/// Calls Radarr's `manifest`, which succeeds, and decodes its answer as a [`ScanStarted`]: the
/// Manifest's field 1 is a string where `ScanStarted`'s is an integer, so it won't decode.
async fn call_manifest_decoded_as_a_scan_started(
    failure: Failure,
) -> Result<ScanStarted, PluginError> {
    let data_dir = tempfile::tempdir().unwrap();
    let cache_config = write_cache_config(data_dir.path()).unwrap();
    let (_, wasm) = BUNDLED
        .iter()
        .find(|(file, _)| *file == "radarr.wasm")
        .unwrap();
    let radarr = load(wasm.to_vec(), &cache_config).unwrap();
    let running = Running {
        package: Arc::clone(&radarr.package),
        compiled: Arc::new(compile(wasm, &cache_config, Vec::new(), Vec::new()).unwrap()),
        secrets: Vec::new().into(),
        crashes: Arc::default(),
        exports: Arc::clone(&radarr.exports),
    };
    let manifest = Method {
        failure,
        export: "manifest",
        ..OBSERVE_EVENTS
    };

    running
        .call_method(&manifest, &ObservedEvents::default())
        .await
}
