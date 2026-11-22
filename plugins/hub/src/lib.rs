//! The Hub Plugin (ADR 0094): the Built-in Plugin through which a Server does everything
//! Hub-facing. It Links the Server to an Account, through a code an Admin enters on the Hub (ADR
//! 0011), keeps the credential the Server presents to the Hub from then on in its Store, and sends
//! the Hub the Diagnostics and Usage Analytics the Admin shares (ADR 0054). Its Users then Link to
//! their own Accounts the same way.

mod hub;
mod linker;
mod token;

use std::{env, sync::Arc};

use nama_plugin_api::{
    Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar, automation::Permission,
    diagnostics_sharer::DiagnosticsSharer, server_link::ServerLinker,
};

use crate::{hub::Hub as HubApi, linker::Linker};

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
            description: "Links the Server, and its Users, to Accounts on Nama's Hub, and sends \
                          it the Diagnostics and Usage Analytics you share in Settings."
                .to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            setup_markdown: "Enable it, then Link the Server under Settings, Server. The Server \
                             shows a code to enter on the Hub while signed in to your Account. \
                             Once it is Linked, each User can Link to their own Account the same \
                             way, from their own Settings."
                .to_owned(),
            extension_points: vec![
                PointKey::provide::<dyn ServerLinker>(),
                PointKey::provide::<dyn DiagnosticsSharer>(),
            ],
            logo_svg: Some(include_str!("../logo.svg").to_owned()),
            permissions: vec![Permission::PERMISSION_STORE],
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
        let linker = Arc::new(Linker::new(HubApi::new(url)?, kernel));
        let sharer: Arc<dyn DiagnosticsSharer> = Arc::<Linker>::clone(&linker);
        registrar.provide::<dyn ServerLinker>(Priority::DEFAULT, linker);
        registrar.provide::<dyn DiagnosticsSharer>(Priority::DEFAULT, sharer);
        Ok(())
    }
}

#[cfg(test)]
mod fake_hub;

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
