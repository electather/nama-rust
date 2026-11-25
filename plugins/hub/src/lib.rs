//! The Hub Plugin (ADR 0094): the Built-in Plugin through which a Linked Server does what is
//! Hub-facing but the Link itself, which the Kernel keeps (ADR 0105, [`KernelLink`]). It sends the
//! Hub the Diagnostics and Usage Analytics the Admin shares (ADR 0054), and each play of the Users
//! the Kernel Linked to Accounts goes to the Hub as a Viewing Event (ADR 0093).
//! Once Linked, it is also a Similar source answering the titles the Hub finds alike (ADR 0084,
//! 0095), a Home row source giving each Linked User the titles the Hub recommends from what
//! their Account finished, as a row and as Featured suggestions (ADR 0072), and sharing Home rows
//! of the Movies trending and the titles most watchlisted across Nama's Accounts (ADR 0058, 0093).

mod charts;
mod hub;
mod linker;
mod recommended;
mod similar;

use std::{fmt, sync::Arc};

use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    automation::Permission,
    contract::{FeaturedSuggestions, HomeRows, SharedRowsAsked, UserFeaturedAsked, UserRowsAsked},
    diagnostics_sharer::DiagnosticsSharer,
    home_row::{HomeRowMethod, HomeRowSource},
    job::JobRunner,
    points::Events,
    server_link::ServerLinker,
    similar_source::SimilarSource,
};

use crate::{
    charts::Charts,
    hub::Hub as HubApi,
    linker::{Jobs, Linker, ViewingEvents, send_job},
    recommended::Recommended,
    similar::Similar,
};

/// The Server's Link with the Hub, which the Kernel keeps (ADR 0105): the Plugin calls the Hub as
/// the Linked Server with its credential.
// ponytail: a Rust handle the Server hands its Built-in Plugin, not the Plugin contract, as it goes
// with the rest of this Plugin's Linking once the Kernel does it all (#2203).
pub trait KernelLink: fmt::Debug + Send + Sync + 'static {
    /// The Hub's address, such as `https://hub.example`.
    fn hub_url(&self) -> &str;

    /// The credential the Linked Server presents to the Hub, `None` while it isn't Linked.
    ///
    /// # Errors
    ///
    /// When the Kernel couldn't read it.
    fn credential(&self) -> BoxFuture<'_, Result<Option<String>, PluginError>>;

    /// Forgets the Link holding `credential`, which the Hub answered it doesn't know any more,
    /// so the Server can Link again. A Link made since stays.
    ///
    /// # Errors
    ///
    /// When the Kernel couldn't forget it.
    fn forget<'a>(&'a self, credential: &'a str) -> BoxFuture<'a, Result<(), PluginError>>;

    /// Whether the User `user_id` is Linked to an Account, as Settings shows it: the one answer
    /// that decides whether their Watch State, Subtitle and Audio Preferences, Watchlist and plays
    /// go to the Hub. It asks the Hub as Settings would, so a User who never opened Settings, such
    /// as one who only signs in with their Account, counts once Linked there.
    ///
    /// # Errors
    ///
    /// When the Kernel can't answer even from what it last heard.
    fn user_linked(&self, user_id: i64) -> BoxFuture<'_, Result<bool, PluginError>>;
}

/// The Plugin.
#[derive(Debug, Default)]
pub struct Hub {
    /// The Kernel's Link with the Hub, `None` when the Server's config names no Hub.
    link: Option<Arc<dyn KernelLink>>,
}

impl Hub {
    /// The Plugin's id.
    pub const ID: PluginId = PluginId::from_static("hub");

    /// The Plugin, calling the Hub through `link`, the Kernel's.
    #[must_use]
    pub fn through(link: Arc<dyn KernelLink>) -> Self {
        Self { link: Some(link) }
    }
}

impl Plugin for Hub {
    fn manifest(&self) -> Manifest {
        Manifest {
            name: "Hub".to_owned(),
            description: "Sends the Hub what Linked Users play and the Diagnostics and Usage \
                          Analytics you share in Settings, adds the titles the Hub finds alike \
                          to Similar titles, gives each Linked User a Recommended for you row \
                          and Featured suggestions from what they finished, and shows Trending \
                          on Nama and Most watchlisted on Nama rows on home."
                .to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            setup_markdown: "Link the Server under Settings, Server, and enable it. Each play \
                             of a Matched title by a User Linked to their own Account goes to the \
                             Hub as a Viewing Event, which their Account can see, delete and \
                             export there. \
                             Read Libraries, Read Watch State and Read Users tell it what was \
                             played, how far, and on what kind of Device. \
                             Home's Trending on Nama row shows the Movies on the Server most \
                             Accounts finished this week and last, once at least 20 did, so it \
                             stays hidden until enough have. Most watchlisted on Nama shows the \
                             Movies and Shows on most Accounts' Watchlists, once at least 20 \
                             Accounts have each, and offers those the Server lacks as Requests."
                .to_owned(),
            extension_points: vec![
                PointKey::provide::<dyn ServerLinker>(),
                PointKey::provide::<dyn DiagnosticsSharer>(),
                PointKey::observe::<Events>(),
                PointKey::provide::<dyn JobRunner>(),
                PointKey::provide::<dyn SimilarSource>(),
                PointKey::provide::<dyn HomeRowSource>(),
            ],
            logo_svg: Some(include_str!("../logo.svg").to_owned()),
            permissions: vec![
                Permission::PERMISSION_STORE,
                Permission::PERMISSION_READ_LIBRARIES,
                Permission::PERMISSION_READ_WATCH_STATES,
                Permission::PERMISSION_READ_USERS,
            ],
            jobs: vec![send_job()],
            ..Manifest::new(Self::ID)
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        let link = (self.link.clone())
            .ok_or("the Server's config names no Hub, so the Plugin has none to call")?;
        let kernel = registrar
            .kernel()
            .ok_or("the Kernel handed the Hub Plugin none of its functions")?;
        let hub = HubApi::new(link.hub_url())?;
        let linker = Arc::new(Linker::new(hub, link, Arc::clone(&kernel)));
        let similar: Arc<dyn SimilarSource> = Arc::new(Similar(Arc::clone(&linker)));
        let rows: Arc<dyn HomeRowSource> = Arc::new(Rows {
            recommended: Recommended::new(Arc::clone(&linker), Arc::clone(&kernel)),
            charts: Charts::new(Arc::clone(&linker), kernel),
        });
        let viewing = ViewingEvents::new(Arc::clone(&linker));
        let sharer: Arc<dyn DiagnosticsSharer> = Arc::<Linker>::clone(&linker);
        registrar.provide::<dyn ServerLinker>(Priority::DEFAULT, Arc::<Linker>::clone(&linker));
        registrar.provide::<dyn DiagnosticsSharer>(Priority::DEFAULT, sharer);
        registrar.observe::<Events>(viewing.clone());
        let jobs: Arc<dyn JobRunner> = Arc::new(Jobs { viewing });
        registrar.provide::<dyn JobRunner>(Priority::DEFAULT, jobs);
        registrar.provide::<dyn SimilarSource>(Priority::DEFAULT, similar);
        registrar.provide::<dyn HomeRowSource>(Priority::DEFAULT, rows);
        Ok(())
    }
}

/// The Hub's Home rows: the shared ones are Nama's charts, each User's own and Featured are what
/// is recommended.
struct Rows {
    recommended: Recommended,
    charts: Charts,
}

impl HomeRowSource for Rows {
    fn shared_rows<'a>(
        &'a self,
        asked: &'a SharedRowsAsked,
    ) -> BoxFuture<'a, Result<HomeRows, PluginError>> {
        self.charts.shared_rows(asked)
    }

    fn user_rows<'a>(
        &'a self,
        asked: &'a UserRowsAsked,
    ) -> BoxFuture<'a, Result<HomeRows, PluginError>> {
        self.recommended.user_rows(asked)
    }

    fn user_featured<'a>(
        &'a self,
        asked: &'a UserFeaturedAsked,
    ) -> BoxFuture<'a, Result<FeaturedSuggestions, PluginError>> {
        self.recommended.user_featured(asked)
    }

    fn gives(&self, _method: HomeRowMethod) -> bool {
        true
    }
}

#[cfg(test)]
mod fake_hub;

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
