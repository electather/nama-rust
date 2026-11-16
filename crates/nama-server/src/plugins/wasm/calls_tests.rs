use std::{
    sync::{Arc, atomic::Ordering},
    time::{Duration, Instant},
};

use nama_plugin_api::{
    BoxFuture,
    automation::{
        DeleteAsked, DeleteStarted, Kernel, KernelRefused, Libraries, LibrariesAsked, StoreEntry,
        StoreKey, StoreValue, Stored, Tick, Titles, TitlesAsked, Users, UsersAsked, WatchStates,
        WatchStatesAsked,
    },
};
use nama_plugin_proto::nama::plugin::v1::Ticked;

use crate::plugins::{Registry, tick};

use super::{
    CRASHES_BEFORE_DISABLING, Running,
    calls::Calls,
    load::{CALL_TIMEOUT, TICK_TIMEOUT, compile_with_kernel, load, write_cache_config},
    services::{Method, POINTS, TICK},
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
/// other calls in its tick at once.
struct Slow;

fn refuse<'a, T: Send + 'a>() -> BoxFuture<'a, Result<T, KernelRefused>> {
    Box::pin(async { Err(KernelRefused("not faked".to_owned())) })
}

impl Kernel for Slow {
    fn store_get<'a>(
        &'a self,
        _: &'a StoreKey,
    ) -> BoxFuture<'a, Result<StoreValue, KernelRefused>> {
        Box::pin(async { Ok(StoreValue::default()) })
    }

    fn store_put<'a>(&'a self, _: &'a StoreEntry) -> BoxFuture<'a, Result<Stored, KernelRefused>> {
        Box::pin(async {
            tokio::time::sleep(SHORT * 3).await;
            Ok(Stored::default())
        })
    }

    fn store_delete<'a>(&'a self, _: &'a StoreKey) -> BoxFuture<'a, Result<Stored, KernelRefused>> {
        refuse()
    }

    fn list_libraries<'a>(
        &'a self,
        _: &'a LibrariesAsked,
    ) -> BoxFuture<'a, Result<Libraries, KernelRefused>> {
        Box::pin(async { Ok(Libraries::default()) })
    }

    fn list_titles<'a>(
        &'a self,
        _: &'a TitlesAsked,
    ) -> BoxFuture<'a, Result<Titles, KernelRefused>> {
        refuse()
    }

    fn list_watch_states<'a>(
        &'a self,
        _: &'a WatchStatesAsked,
    ) -> BoxFuture<'a, Result<WatchStates, KernelRefused>> {
        refuse()
    }

    fn list_users<'a>(&'a self, _: &'a UsersAsked) -> BoxFuture<'a, Result<Users, KernelRefused>> {
        refuse()
    }

    fn delete_media<'a>(
        &'a self,
        _: &'a DeleteAsked,
    ) -> BoxFuture<'a, Result<DeleteStarted, KernelRefused>> {
        refuse()
    }
}

/// The test Plugin built with `features`, enabled as the Server enables it, with `kernel`.
fn running(
    features: &str,
    data_dir: &tempfile::TempDir,
    kernel: Option<Arc<dyn Kernel>>,
) -> Running {
    let cache_config = write_cache_config(data_dir.path()).unwrap();
    let plugin = load(test_plugin::build(features), &cache_config).unwrap();
    let compiled = compile_with_kernel(
        &plugin.wasm,
        &cache_config,
        Vec::new(),
        Vec::new(),
        kernel,
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
    let running = running("automation", &data_dir, Some(Arc::new(Slow)));
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
async fn a_looping_tick_past_its_limit_times_out_and_counts_as_a_crash() {
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
    let running = running("automation", &data_dir, Some(Arc::new(Slow)));
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
        ticked.map(|ticked| ticked.map_err(|failure| failure.message)),
        Some(Err("the Plugin was disabled".to_owned()))
    );
}

#[test]
fn a_tick_may_run_for_tick_timeout_and_every_other_method_for_call_timeout() {
    for method in POINTS.iter().flat_map(|point| point.methods) {
        let limit = if method.export == TICK.export {
            TICK_TIMEOUT
        } else {
            CALL_TIMEOUT
        };
        assert_eq!(method.limit, limit, "{}", method.export);
    }
}
