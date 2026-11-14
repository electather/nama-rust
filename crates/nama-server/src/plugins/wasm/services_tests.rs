use std::sync::Arc;

use nama_plugin_api::points::ObservedEvents;
use nama_plugin_proto::{SERVICES, nama::plugin::v1::EventsObserved, services};

use super::{
    super::{
        Running,
        bundled::BUNDLED,
        compile,
        load::{load, write_cache_config},
    },
    OBSERVE_EVENTS, POINTS,
};

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
