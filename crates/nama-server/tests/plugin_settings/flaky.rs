//! The Built-in Plugin `flaky`, whose making panics as many times as its test says.

use std::sync::{
    Arc, Mutex, PoisonError,
    atomic::{AtomicUsize, Ordering},
};

use nama_plugin_api::{Manifest, Plugin, PluginError, Registrar};
use nama_server::plugin_settings::PluginSettings;
use tempfile::TempDir;

use crate::{Fake, config, open, registry};

/// What one test shares with its [`Flaky`] Plugin: how many more times making it panics, and the
/// greeting its last enable handed it.
pub(super) struct Flakiness {
    panics: AtomicUsize,
    greeting: Mutex<Option<String>>,
}

impl Flakiness {
    pub(super) const fn new() -> Self {
        Self {
            panics: AtomicUsize::new(0),
            greeting: Mutex::new(None),
        }
    }

    pub(super) fn panic_next(&self, makes: usize) {
        self.panics.store(makes, Ordering::SeqCst);
    }

    pub(super) fn greeting(&self) -> Option<String> {
        self.greeting
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

/// The Built-in Plugin `flaky`, with [`Fake`]'s settings, keeping the greeting it's enabled with.
struct Flaky(&'static Flakiness);

impl Plugin for Flaky {
    fn manifest(&self) -> Manifest {
        let fake = Fake {
            id: "flaky",
            panics: false,
        };
        fake.manifest()
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        let greeting = registrar.settings().text("greeting").map(str::to_owned);
        *self
            .0
            .greeting
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = greeting;
        Ok(())
    }
}

/// Starts the Plugins of [`registry`] with `flaky` beside them, whose making panics as
/// `flakiness` says, and the config's `[plugins.*]` tables in `toml`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
pub(super) async fn with_flaky(
    flakiness: &'static Flakiness,
    toml: &str,
) -> (TempDir, PluginSettings) {
    let (data_dir, db) = open().await;
    let registry = registry();
    registry.describe_builtin(move || {
        let panicking = flakiness
            .panics
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |left| {
                left.checked_sub(1)
            });
        assert!(panicking.is_err(), "make bug");
        Arc::new(Flaky(flakiness))
    });
    let config = config(data_dir.path(), toml);
    let plugins = PluginSettings::start(db, registry, &config).await.unwrap();
    (data_dir, plugins)
}
