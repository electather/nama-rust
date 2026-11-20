use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
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
use crate::plugins::Registry;

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
/// once every wait is over, and each reason it handed `gave_up`.
async fn asked_again(busy_for: u32) -> (u32, Vec<String>) {
    let busy = Arc::new(Busy {
        busy_for,
        asked: AtomicU32::new(0),
    });
    let plugins = Registry::new();
    plugins
        .enable(Arc::new(BusyPlugin(Arc::clone(&busy))))
        .unwrap();
    let reasons = Arc::new(Mutex::new(Vec::new()));

    let gave_up = {
        let reasons = Arc::clone(&reasons);
        move |_: &PluginId, reason| reasons.lock().unwrap().push(reason)
    };
    let unfollow = <dyn Fulfiller<Shows>>::unfollow;
    let points = plugins.extension_points();
    ask_again(&points, ID, WAIT, Request::default(), unfollow, gave_up);
    // The paused clock jumps through every wait while all else is idle.
    tokio::time::sleep(WAIT * (RETRIES + 1)).await;

    let reasons = reasons.lock().unwrap().clone();
    (busy.asked.load(Ordering::SeqCst), reasons)
}

#[tokio::test(start_paused = true)]
async fn a_fulfiller_is_asked_again_until_it_acts_without_giving_up() {
    let (asked, gave_up) = asked_again(2).await;

    assert_eq!(asked, 3);
    assert_eq!(gave_up, [""; 0]);
}

#[tokio::test(start_paused = true)]
async fn a_fulfiller_that_keeps_saying_to_try_again_is_asked_retries_times_then_given_up_on_once() {
    let (asked, gave_up) = asked_again(u32::MAX).await;

    assert_eq!(asked, RETRIES);
    let [reason] = &gave_up[..] else {
        panic!("gave up {} times", gave_up.len());
    };
    assert!(
        reason.ends_with(&format!("asked again {RETRIES} times")),
        "{reason}"
    );
}
