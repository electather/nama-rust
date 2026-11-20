use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
    time::Duration,
};

use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    contract::Request,
    fulfiller::{Download, Fulfilled, Fulfiller, Shows, TryAgainIn},
};

use super::{RETRIES, ask_again};
use crate::{log_capture::CapturedLog, plugins::Registry};

const ID: PluginId = PluginId::from_static("busy");
const WAIT: Duration = Duration::from_secs(5);

/// A Fulfiller that says to try again the first `busy_for` times it is told to unfollow a Show,
/// counting each time.
struct Busy {
    busy_for: u32,
    asked: AtomicU32,
}

impl Fulfiller<Shows> for Busy {
    fn fulfill<'a>(&'a self, _: &'a Request) -> BoxFuture<'a, Result<Fulfilled, PluginError>> {
        Box::pin(async { Err("not asked here".into()) })
    }

    fn unfollow<'a>(&'a self, _: &'a Request) -> BoxFuture<'a, Result<(), PluginError>> {
        let asked = self.asked.fetch_add(1, Ordering::SeqCst) + 1;
        let busy = asked <= self.busy_for;
        Box::pin(async move {
            if busy {
                return Err(TryAgainIn(WAIT).into());
            }
            Ok(())
        })
    }

    fn progress<'a>(
        &'a self,
        _: &'a [String],
    ) -> BoxFuture<'a, Result<HashMap<String, Vec<Download>>, PluginError>> {
        Box::pin(async { Ok(HashMap::new()) })
    }
}

/// The Plugin providing `.0`.
struct BusyPlugin(Arc<Busy>);

impl Plugin for BusyPlugin {
    fn manifest(&self) -> Manifest {
        Manifest {
            version: "0.0.0".to_owned(),
            extension_points: vec![PointKey::provide::<dyn Fulfiller<Shows>>()],
            ..Manifest::new(ID)
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        registrar.provide::<dyn Fulfiller<Shows>>(Priority::DEFAULT, Arc::<Busy>::clone(&self.0));
        Ok(())
    }
}

/// How many times a [`Busy`] that says to try again `busy_for` times is asked by [`ask_again`],
/// once every wait is over, and what was logged meanwhile.
async fn asked_again(busy_for: u32) -> (u32, String) {
    let busy = Arc::new(Busy {
        busy_for,
        asked: AtomicU32::new(0),
    });
    let plugins = Registry::new();
    plugins
        .enable(Arc::new(BusyPlugin(Arc::clone(&busy))))
        .unwrap();
    let log = CapturedLog::start();

    let points = plugins.extension_points();
    ask_again(
        &points,
        ID,
        WAIT,
        Request::default(),
        <dyn Fulfiller<Shows>>::unfollow,
        "unfollow it".to_owned(),
    );
    // The paused clock jumps through every wait while all else is idle.
    tokio::time::sleep(WAIT * (RETRIES + 1)).await;

    (busy.asked.load(Ordering::SeqCst), log.finish())
}

#[tokio::test(start_paused = true)]
async fn a_fulfiller_is_asked_again_until_it_acts_and_nothing_is_logged() {
    let (asked, log) = asked_again(2).await;

    assert_eq!(asked, 3);
    assert_eq!(log, "");
}

#[tokio::test(start_paused = true)]
async fn a_fulfiller_that_keeps_saying_to_try_again_is_asked_retries_times_then_logged_once() {
    let (asked, log) = asked_again(u32::MAX).await;

    assert_eq!(asked, RETRIES);
    assert_eq!(log.matches("didn't unfollow it").count(), 1, "{log}");
    assert!(log.contains("retried=10"), "{log}");
}
