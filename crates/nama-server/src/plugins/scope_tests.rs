use std::{
    future,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use nama_plugin_api::{
    Manifest, Plugin, PluginError, PluginId, Registrar,
    automation::{Kernel, KernelRefused, StoreKey},
};

use super::Scope;
use crate::plugins::Registry;

const WAIT: Duration = Duration::from_secs(5);

/// Set once the task holding it has ended, run to its end or cancelled.
#[derive(Clone, Default)]
struct Ended(Arc<AtomicBool>);

impl Ended {
    fn get(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }

    /// A task that runs until it is cancelled, then marks this ended.
    fn forever(&self) -> impl Future<Output = ()> + Send + 'static {
        let guard = SetOnDrop(self.clone());
        async move {
            let _guard = guard;
            future::pending::<()>().await;
        }
    }
}

struct SetOnDrop(Ended);

impl Drop for SetOnDrop {
    fn drop(&mut self) {
        self.0.0.store(true, Ordering::SeqCst);
    }
}

/// A Plugin that, when dropped, records whether `task` had ended by then.
struct Recording {
    id: &'static str,
    task: Ended,
    ended_at_drop: Arc<Mutex<Option<bool>>>,
}

impl Plugin for Recording {
    fn manifest(&self) -> Manifest {
        Manifest {
            version: "0.0.0".to_owned(),
            ..Manifest::new(PluginId::from_static(self.id))
        }
    }

    fn register(&self, _: &mut Registrar) -> Result<(), PluginError> {
        Ok(())
    }
}

impl Drop for Recording {
    fn drop(&mut self) {
        *self.ended_at_drop.lock().unwrap() = Some(self.task.get());
    }
}

/// Enables a [`Recording`] Plugin `id` and spawns a task in its scope that runs until cancelled.
/// Returns the task's flag and what the Plugin records when dropped.
fn enabled_with_task(registry: &Registry, id: &'static str) -> (Ended, Arc<Mutex<Option<bool>>>) {
    let task = Ended::default();
    let ended_at_drop = Arc::default();
    registry
        .enable(Arc::new(Recording {
            id,
            task: task.clone(),
            ended_at_drop: Arc::clone(&ended_at_drop),
        }))
        .unwrap();
    assert!(registry.spawn_in(&PluginId::from_static(id), task.forever()));
    (task, ended_at_drop)
}

async fn until(done: impl Fn() -> bool) {
    tokio::time::timeout(WAIT, async {
        while !done() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("timed out");
}

#[tokio::test]
async fn closing_a_scope_cancels_and_joins_its_tasks() {
    let scope = Scope::default();
    let (first, second) = (Ended::default(), Ended::default());
    scope.spawn(first.forever());
    scope.spawn(second.forever());

    scope.close().await;

    assert!(first.get() && second.get());
}

#[tokio::test]
async fn a_task_in_a_plugins_scope_has_ended_before_disable_returns_and_the_plugin_drops() {
    let registry = Registry::new();
    let (task, ended_at_drop) = enabled_with_task(&registry, "a");

    assert!(registry.disable(&PluginId::from_static("a")).await);

    assert!(task.get());
    assert_eq!(*ended_at_drop.lock().unwrap(), Some(true));
}

#[tokio::test]
async fn spawning_in_a_plugin_that_is_not_enabled_runs_nothing() {
    let registry = Registry::new();
    let ran = Arc::new(AtomicBool::new(false));
    let running = Arc::clone(&ran);

    let spawned = registry.spawn_in(&PluginId::from_static("none"), async move {
        running.store(true, Ordering::SeqCst);
    });

    assert!(!spawned);
    tokio::task::yield_now().await;
    assert!(!ran.load(Ordering::SeqCst));
}

#[tokio::test]
async fn disable_now_cancels_at_once_and_drops_the_plugin_after_it_returns() {
    let registry = Registry::new();
    let (task, ended_at_drop) = enabled_with_task(&registry, "a");

    assert!(registry.disable_now(&PluginId::from_static("a")));

    assert_eq!(*ended_at_drop.lock().unwrap(), None);
    assert!(!registry.spawn_in(&PluginId::from_static("a"), async {}));
    until(|| ended_at_drop.lock().unwrap().is_some()).await;
    assert!(task.get());
    assert_eq!(*ended_at_drop.lock().unwrap(), Some(true));
}

#[tokio::test]
async fn enabling_again_gives_a_scope_the_old_ones_cancel_does_not_touch() {
    let registry = Registry::new();
    let (old_task, old_dropped) = enabled_with_task(&registry, "a");
    registry.disable_now(&PluginId::from_static("a"));

    let (new_task, _) = enabled_with_task(&registry, "a");
    until(|| old_dropped.lock().unwrap().is_some()).await;

    assert!(old_task.get());
    assert!(!new_task.get());
}

#[tokio::test]
async fn shutdown_joins_every_plugins_scope_and_leaves_them_enabled() {
    let registry = Registry::new();
    let (a, _) = enabled_with_task(&registry, "a");
    let (b, _) = enabled_with_task(&registry, "b");

    registry.shutdown().await;

    assert!(a.get() && b.get());
    assert_eq!(
        registry.enabled(),
        [PluginId::from_static("a"), PluginId::from_static("b")]
    );
}

/// A Plugin that keeps the Kernel's functions each enablement of it is handed.
#[derive(Default)]
struct Keeping {
    kernels: Arc<Mutex<Vec<Arc<dyn Kernel>>>>,
}

impl Plugin for Keeping {
    fn manifest(&self) -> Manifest {
        Manifest {
            version: "0.0.0".to_owned(),
            ..Manifest::new(PluginId::from_static("keeping"))
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        self.kernels.lock().unwrap().extend(registrar.kernel());
        Ok(())
    }
}

/// What `kernel` answers asking for a Store entry: the refusal's words, or `None` if it answered.
async fn refusal(kernel: &Arc<dyn Kernel>) -> Option<String> {
    let asked = kernel.store_get(&StoreKey::default()).await;
    asked.err().map(|KernelRefused(words)| words)
}

#[tokio::test]
async fn once_disabled_an_enablements_kernel_functions_refuse_and_the_next_ones_do_not() {
    let registry = Registry::new();
    let id = PluginId::from_static("keeping");
    let plugin = Keeping::default();
    let kernels = Arc::clone(&plugin.kernels);
    registry.enable(Arc::new(plugin)).unwrap();
    let old = Arc::clone(&kernels.lock().unwrap()[0]);

    assert!(registry.disable(&id).await);
    registry
        .enable(Arc::new(Keeping {
            kernels: Arc::clone(&kernels),
        }))
        .unwrap();
    let new = Arc::clone(&kernels.lock().unwrap()[1]);

    let disabled = Some("the Plugin is disabled".to_owned());
    assert_eq!(refusal(&old).await, disabled);
    // The Kernel isn't served in this test, so the new one refuses only for that.
    assert_ne!(refusal(&new).await, disabled);
}
