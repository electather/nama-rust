//! The Hub Plugin (ADR 0094): the Built-in Plugin through which a Server does everything
//! Hub-facing. It Links the Server to an Account, through a code an Admin enters on the Hub (ADR
//! 0011), keeps the credential the Server presents to the Hub from then on in its Store, and sends
//! the Hub the Diagnostics and Usage Analytics the Admin shares (ADR 0054). Its Users then Link to
//! their own Accounts the same way, their Watch State syncs to the Hub, each play of theirs goes
//! to the Hub as a Viewing Event (ADR 0093), and the Hub lists the Server, with its Server
//! Addresses, to every Account it has a User on.
//! Once Linked, it is also a Similar source answering the titles the Hub finds alike (ADR 0084,
//! 0095).

mod hub;
mod linker;
mod similar;
mod token;

use std::{env, sync::Arc};

use nama_plugin_api::{
    Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    automation::Permission,
    diagnostics_sharer::DiagnosticsSharer,
    job::JobRunner,
    points::{Events, WatchStatesChanged},
    server_link::ServerLinker,
    similar_source::SimilarSource,
};

use crate::{
    hub::Hub as HubApi,
    linker::{Jobs, Linker, ViewingEvents, WatchStateSync, send_job, sync_job},
    similar::Similar,
};

/// The variable naming the Hub's address, such as `https://hub.example`.
const HUB_URL_VARIABLE: &str = "NAMA_HUB_URL";

/// The Plugin.
#[derive(Debug)]
pub struct Hub {
    /// The Hub's address, if one is set.
    url: Option<String>,
}

impl Hub {
    /// The Plugin's id.
    pub const ID: PluginId = PluginId::from_static("hub");

    /// The Plugin, calling the Hub that `NAMA_HUB_URL` names.
    // ponytail: no default address until #1834 picks the Hub's domain; then default to it and
    // keep the variable as the development override ADR 0094 allows.
    #[must_use]
    pub fn from_env() -> Self {
        Self {
            url: env::var(HUB_URL_VARIABLE)
                .ok()
                .filter(|url| !url.is_empty()),
        }
    }

    /// The Plugin, calling the Hub at `url`, as a test's stand-in.
    #[must_use]
    pub fn at(url: impl Into<String>) -> Self {
        Self {
            url: Some(url.into()),
        }
    }
}

impl Plugin for Hub {
    fn manifest(&self) -> Manifest {
        Manifest {
            name: "Hub".to_owned(),
            description: "Links the Server, and its Users, to Accounts on Nama's Hub, syncs \
                          Linked Users' Watch State there and sends it what they play, sends it \
                          the Diagnostics and Usage Analytics you share in Settings, and adds the \
                          titles the Hub finds alike to Similar titles."
                .to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            setup_markdown: "Enable it, then Link the Server under Settings, Server. The Server \
                             shows a code to enter on the Hub while signed in to your Account. \
                             Once it is Linked, each User can Link to their own Account the same \
                             way, from their own Settings. The Hub then lists the Server's \
                             Server URL and LAN address to the Accounts its Users Linked to, so \
                             their apps can find it. Each play of a Matched title by a Linked User \
                             goes to the Hub as a Viewing Event, which their Account can see, \
                             delete and export there. Read Libraries, Read Watch State and Read \
                             Users tell it what was played, how far, and on what kind of Device."
                .to_owned(),
            extension_points: vec![
                PointKey::provide::<dyn ServerLinker>(),
                PointKey::provide::<dyn DiagnosticsSharer>(),
                PointKey::observe::<WatchStatesChanged>(),
                PointKey::observe::<Events>(),
                PointKey::provide::<dyn JobRunner>(),
                PointKey::provide::<dyn SimilarSource>(),
            ],
            logo_svg: Some(include_str!("../logo.svg").to_owned()),
            permissions: vec![
                Permission::PERMISSION_STORE,
                Permission::PERMISSION_READ_LIBRARIES,
                Permission::PERMISSION_READ_WATCH_STATES,
                Permission::PERMISSION_READ_USERS,
            ],
            jobs: vec![sync_job(), send_job()],
            ..Manifest::new(Self::ID)
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        let url = self.url.as_deref().ok_or_else(|| {
            format!("{HUB_URL_VARIABLE} isn't set, so the Plugin doesn't know where the Hub is")
        })?;
        let kernel = registrar
            .kernel()
            .ok_or("the Kernel handed the Hub Plugin none of its functions")?;
        let hub = HubApi::new(url)?;
        let similar: Arc<dyn SimilarSource> =
            Arc::new(Similar::new(hub.clone(), Arc::clone(&kernel)));
        let linker = Arc::new(Linker::new(hub, kernel));
        let viewing = ViewingEvents::new(Arc::clone(&linker));
        let sharer: Arc<dyn DiagnosticsSharer> = Arc::<Linker>::clone(&linker);
        registrar.provide::<dyn ServerLinker>(Priority::DEFAULT, Arc::<Linker>::clone(&linker));
        registrar.provide::<dyn DiagnosticsSharer>(Priority::DEFAULT, sharer);
        registrar.observe::<WatchStatesChanged>(WatchStateSync(Arc::clone(&linker)));
        registrar.observe::<Events>(viewing.clone());
        let jobs: Arc<dyn JobRunner> = Arc::new(Jobs { linker, viewing });
        registrar.provide::<dyn JobRunner>(Priority::DEFAULT, jobs);
        registrar.provide::<dyn SimilarSource>(Priority::DEFAULT, similar);
        Ok(())
    }
}

#[cfg(test)]
mod fake_hub;

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
