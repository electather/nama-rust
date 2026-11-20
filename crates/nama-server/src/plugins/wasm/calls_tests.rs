use std::{
    sync::{Arc, atomic::Ordering},
    time::{Duration, Instant},
};

use nama_plugin_api::{
    BoxFuture,
    automation::{Kernel, KernelRefused, Tick},
};
use nama_plugin_proto::{
    nama::plugin::v1::{EventsObserved, ObservedEvents, Ticked},
    services,
};
use tokio::sync::Notify;

use crate::plugins::{Failure, Registry, tick};

use super::{
    CRASHES_BEFORE_DISABLING, Running,
    calls::Calls,
    load::{
        BLOB_TIMEOUT, CALL_TIMEOUT, HOME_ROWS_TIMEOUT, TICK_TIMEOUT, compile_with_kernel, load,
        write_cache_config,
    },
    plugin_log::PluginLog,
    services::{Method, OBSERVE_EVENTS, POINTS, TICK},
};

#[expect(
    clippy::duplicate_mod,
    reason = "`automation_tests.rs` loads it too, and keeps its copy private"
)]
#[path = "../../../tests/support/test_plugin.rs"]
mod test_plugin;

/// A limit short enough that a test waits past it in a moment.
const SHORT: Duration = Duration::from_millis(500);

/// A Kernel that takes three times [`SHORT`] to store a value, and answers the test Plugin's
/// other calls in its tick at once, each with an empty message.
fn slow() -> Arc<dyn Kernel> {
    Arc::new(
        |function: &str, _request| -> BoxFuture<'static, Result<Vec<u8>, KernelRefused>> {
            let store_put = function == "store_put";
            Box::pin(async move {
                if store_put {
                    tokio::time::sleep(SHORT * 3).await;
                }
                Ok(Vec::new())
            })
        },
    )
}

/// A Kernel whose `store_put`, the first function the test Plugin's tick calls, says it was
/// called on `entered` and answers once `release` is notified. It refuses everything else.
fn gate(entered: &Arc<Notify>, release: &Arc<Notify>) -> Arc<dyn Kernel> {
    let (entered, release) = (Arc::clone(entered), Arc::clone(release));
    Arc::new(
        move |function: &str, _request| -> BoxFuture<'static, Result<Vec<u8>, KernelRefused>> {
            let store_put = function == "store_put";
            let (entered, release) = (Arc::clone(&entered), Arc::clone(&release));
            Box::pin(async move {
                if !store_put {
                    return Err(KernelRefused("not faked".to_owned()));
                }
                entered.notify_one();
                release.notified().await;
                Ok(Vec::new())
            })
        },
    )
}

/// Opens a [`gate`] when dropped, so a test that fails while a call waits in it ends instead of
/// hanging: the runtime's shutdown waits for that call's blocking thread.
struct Release(Arc<Notify>);

impl Drop for Release {
    fn drop(&mut self) {
        self.0.notify_one();
    }
}

/// The test Plugin built with `features`, enabled as the Server enables it, with `kernel`.
fn running(
    features: &str,
    data_dir: &tempfile::TempDir,
    kernel: Option<Arc<dyn Kernel>>,
) -> Running {
    running_with(features, Vec::new(), data_dir, kernel)
}

/// [`running`] with the settings `config`, as Extism hands them to the Plugin.
fn running_with(
    features: &str,
    config: Vec<(String, String)>,
    data_dir: &tempfile::TempDir,
    kernel: Option<Arc<dyn Kernel>>,
) -> Running {
    let cache_config = write_cache_config(data_dir.path()).unwrap();
    let plugin = load(test_plugin::build(features), &cache_config).unwrap();
    let compiled = compile_with_kernel(
        &plugin.wasm,
        &cache_config,
        Vec::new(),
        config,
        kernel,
        Arc::new(PluginLog::new(plugin.manifest.id.clone(), Arc::default())),
        Calls::default(),
    )
    .unwrap();
    Running {
        package: plugin.package,
        compiled: Arc::new(compiled),
        secrets: Arc::default(),
        crashes: Arc::default(),
        exports: plugin.exports,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_call_waiting_in_a_kernel_function_past_its_limit_still_succeeds() {
    let data_dir = tempfile::tempdir().unwrap();
    let running = running("automation", &data_dir, Some(slow()));
    let tick = Method {
        limit: SHORT,
        ..TICK
    };

    let ticked = running
        .call_method::<_, Ticked>(&tick, &Tick::default())
        .await;

    assert_eq!(ticked.err().map(|err| err.to_string()), None);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_tick_logging_in_a_loop_times_out_at_its_limit_and_counts_as_a_crash() {
    let data_dir = tempfile::tempdir().unwrap();
    let running = running("looping-tick", &data_dir, None);
    let tick = Method {
        limit: SHORT,
        ..TICK
    };
    let started = Instant::now();

    let failed = running
        .call_method::<_, Ticked>(&tick, &Tick::default())
        .await
        .unwrap_err();

    assert_eq!(failed.to_string(), "timeout");
    assert_eq!(running.crashes.load(Ordering::Relaxed), 1);
    assert!(started.elapsed() < CALL_TIMEOUT, "{:?}", started.elapsed());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_call_another_calls_timeout_interrupts_fails_plainly_and_not_as_a_crash() {
    let data_dir = tempfile::tempdir().unwrap();
    let (entered, release) = (Arc::new(Notify::new()), Arc::new(Notify::new()));
    let gate = gate(&entered, &release);
    let release = Release(release);
    // One compile whose tick waits in the gate and whose Observer loops.
    let running = running_with(
        "automation",
        vec![("observer".to_owned(), "loop".to_owned())],
        &data_dir,
        Some(gate),
    );
    let ticking = tokio::spawn({
        let running = running.clone();
        async move {
            running
                .call_method::<_, Ticked>(&TICK, &Tick::default())
                .await
        }
    });
    entered.notified().await;
    let observe = Method {
        limit: SHORT,
        ..OBSERVE_EVENTS
    };

    let timed_out = running
        .call_method::<_, EventsObserved>(&observe, &ObservedEvents::default())
        .await
        .unwrap_err();
    drop(release);
    let interrupted = tokio::time::timeout(CALL_TIMEOUT, ticking)
        .await
        .expect("the tick ran on past the other call's timeout")
        .unwrap()
        .unwrap_err();

    assert_eq!(timed_out.to_string(), "timeout");
    assert_eq!(
        interrupted.to_string(),
        "another call of the Plugin ran past its time limit, and ending it ended this one too"
    );
    assert_eq!(running.crashes.load(Ordering::Relaxed), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn stopping_a_plugins_calls_fails_a_running_one_as_disabled_and_not_as_a_crash() {
    let data_dir = tempfile::tempdir().unwrap();
    let running = running("looping-tick", &data_dir, None);
    // One crash short of disabling, so counting the stop as one would unwind here.
    running
        .crashes
        .store(CRASHES_BEFORE_DISABLING - 1, Ordering::Relaxed);
    let calling = running.clone();
    let call = tokio::spawn(async move {
        calling
            .call_method::<_, Ticked>(&TICK, &Tick::default())
            .await
    });
    tokio::time::sleep(SHORT).await;
    let stopped = Instant::now();

    running.compiled.calls.stop();

    let failed = call.await.unwrap().unwrap_err();
    assert_eq!(failed.to_string(), "the Plugin was disabled");
    assert_eq!(
        running.crashes.load(Ordering::Relaxed),
        CRASHES_BEFORE_DISABLING - 1
    );
    assert!(stopped.elapsed() < CALL_TIMEOUT, "{:?}", stopped.elapsed());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_call_after_its_plugins_calls_are_stopped_fails_as_disabled_without_running() {
    let data_dir = tempfile::tempdir().unwrap();
    let running = running("automation", &data_dir, Some(slow()));
    running.compiled.calls.stop();

    let failed = running
        .call_method::<_, Ticked>(&TICK, &Tick::default())
        .await
        .unwrap_err();

    assert_eq!(failed.to_string(), "the Plugin was disabled");
}

#[tokio::test(flavor = "multi_thread")]
async fn disabling_a_plugin_while_its_tick_runs_ends_the_wasm_call_promptly() {
    let data_dir = tempfile::tempdir().unwrap();
    let cache_config = write_cache_config(data_dir.path()).unwrap();
    let plugin = load(test_plugin::build("looping-tick"), &cache_config).unwrap();
    let id = plugin.manifest.id.clone();
    let registry = Registry::new();
    registry.enable(Arc::new(plugin)).unwrap();
    // Outside the Plugin's scope, so only the stop, not the scope's cancel, can end it.
    let ticking = tokio::spawn({
        let (points, id) = (registry.extension_points(), id.clone());
        async move { tick::once(&points, &id).await }
    });
    tokio::time::sleep(SHORT).await;

    let disabled = tokio::time::timeout(CALL_TIMEOUT, registry.disable(&id)).await;

    assert_eq!(disabled.ok(), Some(true));
    let ticked = tokio::time::timeout(CALL_TIMEOUT, ticking)
        .await
        .expect("the tick ran on past the disable")
        .unwrap();
    assert_eq!(
        ticked,
        tick::Ticked::Ran(Err(Failure {
            kind: (),
            message: "the Plugin was disabled".to_owned()
        }))
    );
}

#[test]
fn a_tick_a_blob_call_and_a_home_row_refresh_may_run_long_and_every_other_method_for_call_timeout()
{
    for method in POINTS.iter().flat_map(|point| point.methods) {
        let limit = if method.export == TICK.export {
            TICK_TIMEOUT
        } else if method.service == services::BLOB_STORE {
            BLOB_TIMEOUT
        } else if method.service == services::HOME_ROW {
            HOME_ROWS_TIMEOUT
        } else {
            CALL_TIMEOUT
        };
        assert_eq!(method.limit, limit, "{}", method.export);
    }
}
