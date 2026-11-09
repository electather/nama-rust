//! A Built-in Plugin that logs the Kernel's events. It is the smallest Plugin there is, and shows
//! that the wiring from the Kernel to a Plugin works.

use nama_plugin_api::{
    BoxFuture, Manifest, Observer, Plugin, PluginId, Registrar,
    points::{ServerStarted, ServerStartedEvent},
    settings::SettingsSchema,
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
    fn manifest(&self) -> Manifest {
        Manifest {
            id: Self::ID,
            name: "Event log".to_owned(),
            description: "Writes everything that happens on the Server to its log.".to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            settings: SettingsSchema::default(),
            logo_svg: Some(include_str!("../logo.svg").to_owned()),
        }
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
