//! A Built-in Plugin that logs the Kernel's events. It is the smallest Plugin there is, and shows
//! that the wiring from the Kernel to a Plugin works.

use nama_plugin_api::{
    BoxFuture, Observer, Plugin, PluginId, Registrar,
    points::{ServerStarted, ServerStartedEvent},
};
use tracing::info;

/// The Plugin.
#[derive(Debug, Default)]
pub struct LogEvents;

impl LogEvents {
    /// The Plugin's id.
    pub const ID: PluginId = PluginId::from_static("log-events");
}

impl Plugin for LogEvents {
    fn id(&self) -> PluginId {
        Self::ID
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn register(&self, registrar: &mut Registrar) {
        registrar.observe::<ServerStarted>(Log);
    }
}

struct Log;

impl Observer<ServerStarted> for Log {
    fn observe<'a>(&'a self, event: &'a ServerStartedEvent) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            info!(name = %event.name, version = %event.version, "observed server started");
        })
    }
}
