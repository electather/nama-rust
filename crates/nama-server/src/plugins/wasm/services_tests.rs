use std::{collections::BTreeMap, path::PathBuf, sync::Arc, time::Duration};

use nama_plugin_api::{
    BoxFuture, Manifest, Observer, Plugin, PluginError, PluginId, PointKey, Registrar,
    points::{Events, ObservedEvents},
    settings::{SettingValue, Settings},
};
use nama_plugin_proto::{SERVICES, nama::plugin::v1::ScanStarted, services};

use super::{
    super::{
        listing::Found,
        load::{BLOB_TIMEOUT, load, write_cache_config},
        tests::{installed, running},
    },
    Method, OBSERVE_EVENTS, POINTS,
};
use crate::{log_capture::CapturedLog, plugins::Registry};

#[expect(
    clippy::duplicate_mod,
    reason = "`automation_tests.rs` loads it too, and keeps its copy private"
)]
#[path = "../../../tests/support/test_plugin.rs"]
mod test_plugin;

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
fn a_tick_is_a_required_row() {
    let automation = POINTS
        .iter()
        .find(|point| point.service == services::AUTOMATION)
        .unwrap();

    assert!(matches!(
        automation.methods,
        [Method {
            export: "tick",
            optional: false,
            ..
        }]
    ));
}

#[test]
fn a_blob_store_is_five_required_rows_that_may_run_long() {
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
        assert_eq!(method.limit, BLOB_TIMEOUT, "{}", method.name);
    }
    assert!(blob_store.runs_long());
    assert_eq!(blob_store.crash_group, services::BLOB_STORE);
}

#[test]
fn a_home_row_source_runs_long_on_a_compile_of_its_own() {
    let home_row = POINTS
        .iter()
        .find(|point| point.service == services::HOME_ROW)
        .unwrap();

    assert!(home_row.runs_long());
    assert!(matches!(
        home_row.methods,
        [Method {
            export: "shared_rows",
            ..
        }]
    ));
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
async fn an_answer_that_wont_decode_fails_saying_so_and_isnt_logged_where_its_called() {
    let (_data_dir, radarr) = installed("radarr.wasm");
    let running = running(&radarr);
    // Radarr's `manifest` succeeds, but its field 1 is a string where `ScanStarted`'s is an integer.
    let manifest = Method {
        export: "manifest",
        ..OBSERVE_EVENTS
    };
    let log = CapturedLog::start();

    let failed = running
        .call_method::<_, ScanStarted>(&manifest, &ObservedEvents::default())
        .await
        .unwrap_err();

    let failed = failed.to_string();
    assert!(failed.starts_with("its answer didn't decode: "), "{failed}");
    assert!(failed.contains("wire type"), "{failed}");
    assert_eq!(log.finish(), "");
}

const TEST_PLUGIN: PluginId = PluginId::from_static("test-plugin");

/// A host no Plugin may reach, so the test Plugin's Observer fetching it is refused.
const UNLISTED: &str = "http://unlisted.invalid/";

/// The test Plugin's refusal when its Observer fetches [`UNLISTED`].
const REFUSED: &str = "HTTP request to http://unlisted.invalid/ is not allowed";

/// A Built-in Plugin under the test Plugin's id whose Observer fails as the test Plugin's does
/// when fetching [`UNLISTED`].
struct Refuses;

impl Observer<Events> for Refuses {
    fn observe<'a>(&'a self, _: &'a ObservedEvents) -> BoxFuture<'a, Result<(), PluginError>> {
        Box::pin(async { Err(REFUSED.into()) })
    }
}

impl Plugin for Refuses {
    fn manifest(&self) -> Manifest {
        Manifest {
            extension_points: vec![PointKey::observe::<Events>()],
            ..Manifest::new(TEST_PLUGIN)
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        registrar.observe::<Events>(Self);
        Ok(())
    }
}

#[tokio::test]
async fn an_observers_failure_is_logged_once_alike_for_a_builtin_and_an_installed_plugin() {
    let builtin = Registry::new();
    builtin.describe_builtin(|| Arc::new(Refuses));
    let data_dir = tempfile::tempdir().unwrap();
    let installed = Registry::new();
    let cache_config = write_cache_config(data_dir.path()).unwrap();
    let test_plugin = load(test_plugin::build(""), &cache_config).unwrap();
    installed.list_installed(Found(vec![(PathBuf::from("test.wasm"), Ok(test_plugin))]));

    let builtin_lines = observer_failure_logged(&builtin).await;
    let installed_lines = observer_failure_logged(&installed).await;

    assert_eq!(builtin_lines.len(), 1, "{builtin_lines:?}");
    assert!(
        builtin_lines[0].contains(r#"observer failed plugin=test-plugin point="events""#),
        "{builtin_lines:?}"
    );
    assert_eq!(installed_lines, builtin_lines);
}

/// The lines naming [`REFUSED`] that `registry` logs once its listed `test-plugin`, enabled to
/// fetch [`UNLISTED`], fails to observe an Event, each without its timestamp.
async fn observer_failure_logged(registry: &Registry) -> Vec<String> {
    let fetch = BTreeMap::from([("fetch".to_owned(), SettingValue::Text(UNLISTED.to_owned()))]);
    registry
        .enable_by_id(&TEST_PLUGIN, Settings::new(fetch))
        .unwrap();
    let points = registry.extension_points();

    // The test's runtime has one thread, so the Observer's task logs to it.
    let log = CapturedLog::start();
    points.notify::<Events>(ObservedEvents::default());
    // The Installed Plugin's call runs on a blocking thread; the failure is logged as it is recorded.
    for _ in 0..500 {
        if points
            .failing(&TEST_PLUGIN, PointKey::observe::<Events>())
            .is_some()
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let logged = log.finish();

    let failed = logged.lines().filter(|line| line.contains(REFUSED));
    let untimed = failed.map(|line| line.split_once(' ').map_or(line, |(_, rest)| rest));
    untimed.map(str::to_owned).collect()
}
