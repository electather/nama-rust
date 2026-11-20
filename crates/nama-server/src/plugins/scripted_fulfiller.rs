//! A Fulfiller a unit test scripts, so it states only the ids it needs, how often it says to try
//! again, and whether it fails or panics, and reads back what it was told.

use std::{
    collections::HashMap,
    mem,
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    contract::Request,
    fulfiller::{Download, Fulfilled, Fulfiller, Movies, Shows, StopMonitoring, TryAgainIn},
};
use tokio::sync::Notify;

use super::Registry;

/// What a [`ScriptedFulfiller`] was told, in the order told.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Told {
    /// Handed the Request with this id.
    Fulfill(i64),
    /// Told to stop following the Show of the Request with this id.
    Unfollow(i64),
    /// Told to stop monitoring a deleted title.
    StopMonitoring(StopMonitoring),
}

/// A Fulfiller of Movies and Shows that acts on whatever it's told, unless scripted otherwise.
/// Its Plugin, [`Self::enabled`], provides it for both kinds.
#[derive(Debug, Default)]
pub(crate) struct ScriptedFulfiller {
    needs: Mutex<Vec<String>>,
    /// How many of the first asks of each method say to try again, and after how long.
    tries_again: (usize, Duration),
    fails: Option<String>,
    panics: bool,
    /// Holds each ask after the first until [`Self::let_one_through`].
    hold: Option<Notify>,
    started: AtomicUsize,
    told: Mutex<Vec<Told>>,
}

impl ScriptedFulfiller {
    /// Needs an id in one of `namespaces` to be asked about a title.
    pub(crate) fn needing(self, namespaces: &[&str]) -> Self {
        self.need(namespaces);
        self
    }

    /// Says to try again in `wait` the first `times` it is asked each method.
    pub(crate) fn trying_again(self, times: usize, wait: Duration) -> Self {
        Self {
            tries_again: (times, wait),
            ..self
        }
    }

    /// Fails with `message` once it no longer says to try again.
    pub(crate) fn failing(self, message: &str) -> Self {
        Self {
            fails: Some(message.to_owned()),
            ..self
        }
    }

    /// Panics whenever it is asked.
    pub(crate) fn panicking(self) -> Self {
        Self {
            panics: true,
            ..self
        }
    }

    /// Holds each ask after the first until [`Self::let_one_through`], so a test can read what the
    /// first ask did before a retry lands.
    pub(crate) fn holding(self) -> Self {
        Self {
            hold: Some(Notify::new()),
            ..self
        }
    }

    /// Needs an id in one of `namespaces` from now on, or none if empty.
    pub(crate) fn need(&self, namespaces: &[&str]) {
        *lock(&self.needs) = namespaces.iter().map(|&n| n.to_owned()).collect();
    }

    /// Lets one held ask go on.
    pub(crate) fn let_one_through(&self) {
        if let Some(hold) = &self.hold {
            hold.notify_one();
        }
    }

    /// Everything it was told, held asks excepted.
    pub(crate) fn told(&self) -> Vec<Told> {
        lock(&self.told).clone()
    }

    /// How many times it was asked anything, held asks excepted.
    pub(crate) fn asked(&self) -> usize {
        lock(&self.told).len()
    }

    /// The ids of the Requests it was handed.
    pub(crate) fn handed(&self) -> Vec<i64> {
        let told = self.told().into_iter();
        told.filter_map(|told| match told {
            Told::Fulfill(id) => Some(id),
            Told::Unfollow(_) | Told::StopMonitoring(_) => None,
        })
        .collect()
    }

    /// The ids of the Requests whose Show it was told to stop following.
    pub(crate) fn unfollowed(&self) -> Vec<i64> {
        let told = self.told().into_iter();
        told.filter_map(|told| match told {
            Told::Unfollow(id) => Some(id),
            Told::Fulfill(_) | Told::StopMonitoring(_) => None,
        })
        .collect()
    }

    /// A new Registry whose one Plugin, with the id `id`, provides this as the Fulfiller of
    /// Movies and of Shows.
    pub(crate) fn enabled(self, id: &'static str) -> (Registry, Arc<Self>) {
        let fulfiller = Arc::new(self);
        let plugins = Registry::new();
        let plugin = ScriptedPlugin {
            id,
            fulfiller: Arc::clone(&fulfiller),
        };
        plugins.enable(Arc::new(plugin)).unwrap();
        (plugins, fulfiller)
    }

    /// Answers `told` with `acted` as scripted, recording it once any hold lets it go.
    fn answer<'a, R: Send + 'a>(
        &'a self,
        told: Told,
        acted: R,
    ) -> BoxFuture<'a, Result<R, PluginError>> {
        assert!(!self.panics, "the scripted Fulfiller panicked");
        let held = self.started.fetch_add(1, Ordering::SeqCst) > 0;
        Box::pin(async move {
            if let (true, Some(hold)) = (held, &self.hold) {
                hold.notified().await;
            }
            let mut log = lock(&self.told);
            let method = mem::discriminant(&told);
            let earlier = log
                .iter()
                .filter(|t| mem::discriminant(*t) == method)
                .count();
            log.push(told);
            drop(log);
            let (times, wait) = self.tries_again;
            if earlier < times {
                return Err(TryAgainIn(wait).into());
            }
            match &self.fails {
                Some(message) => Err(message.clone().into()),
                None => Ok(acted),
            }
        })
    }
}

impl<Kind: 'static> Fulfiller<Kind> for ScriptedFulfiller {
    fn needs(&self) -> Vec<String> {
        lock(&self.needs).clone()
    }

    fn fulfill<'a>(
        &'a self,
        request: &'a Request,
    ) -> BoxFuture<'a, Result<Fulfilled, PluginError>> {
        let taken = Fulfilled {
            reference: format!("taken-{}", request.id),
            ..Fulfilled::default()
        };
        self.answer(Told::Fulfill(request.id), taken)
    }

    fn unfollow<'a>(&'a self, request: &'a Request) -> BoxFuture<'a, Result<(), PluginError>> {
        self.answer(Told::Unfollow(request.id), ())
    }

    fn stop_monitoring<'a>(
        &'a self,
        title: &'a StopMonitoring,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        self.answer(Told::StopMonitoring(title.clone()), ())
    }

    fn progress<'a>(
        &'a self,
        _references: &'a [String],
    ) -> BoxFuture<'a, Result<HashMap<String, Vec<Download>>, PluginError>> {
        Box::pin(async { Ok(HashMap::new()) })
    }
}

/// The Plugin [`ScriptedFulfiller::enabled`] enables.
struct ScriptedPlugin {
    id: &'static str,
    fulfiller: Arc<ScriptedFulfiller>,
}

impl Plugin for ScriptedPlugin {
    fn manifest(&self) -> Manifest {
        Manifest {
            version: "0.0.0".to_owned(),
            extension_points: vec![
                PointKey::provide::<dyn Fulfiller<Movies>>(),
                PointKey::provide::<dyn Fulfiller<Shows>>(),
            ],
            ..Manifest::new(PluginId::from_static(self.id))
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        let fulfiller = &self.fulfiller;
        registrar.provide::<dyn Fulfiller<Movies>>(
            Priority::DEFAULT,
            Arc::<ScriptedFulfiller>::clone(fulfiller),
        );
        registrar.provide::<dyn Fulfiller<Shows>>(
            Priority::DEFAULT,
            Arc::<ScriptedFulfiller>::clone(fulfiller),
        );
        Ok(())
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
