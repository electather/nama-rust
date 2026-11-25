//! Hub Suggestions (ADR 0105): the Built-in Plugin that turns what the Hub suggests to a Linked
//! Server into Similar titles, the titles its vectors from Wikidata's facts find alike (ADR 0084,
//! 0095), a Recommended for you row and Featured suggestions for each Linked User from what their
//! Account finished (ADR 0072), and shared Home rows of the Movies trending and the titles most
//! watchlisted across Nama's Accounts (ADR 0058, 0093). It asks through the Kernel's `list_hub_*`
//! functions, which answer nothing while the Server isn't Linked, so it needs no Hub of its own.

mod charts;
mod recommended;
mod similar;

use std::sync::Arc;

use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    automation::Permission,
    contract::{FeaturedSuggestions, HomeRows, SharedRowsAsked, UserFeaturedAsked, UserRowsAsked},
    home_row::{HomeRowMethod, HomeRowSource},
    similar_source::SimilarSource,
};

use crate::{charts::Charts, recommended::Recommended, similar::Similar};

/// The Plugin.
#[derive(Debug, Default)]
pub struct HubSuggestions;

impl HubSuggestions {
    /// The Plugin's id: the Hub Plugin's, which it was, so an upgrade keeps it enabled with its
    /// grants, stored Similar titles and Home layouts naming its rows.
    pub const ID: PluginId = PluginId::from_static("hub");
}

impl Plugin for HubSuggestions {
    fn manifest(&self) -> Manifest {
        Manifest {
            name: "Hub Suggestions".to_owned(),
            description:
                "Adds the titles the Hub finds alike to Similar titles, gives each Linked \
                          User a Recommended for you row and Featured suggestions from what they \
                          finished, and shows Trending on Nama and Most watchlisted on Nama rows \
                          on home."
                    .to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            setup_markdown: "Link the Server under Settings, Server, and enable it; until the \
                             Server is Linked it suggests nothing. \
                             Read Libraries tells it which of the titles the Hub suggests the \
                             Server has, and Read Watch States lets it ask for each User's \
                             recommendations, which the Hub makes from what their Account \
                             finished. \
                             Home's Trending on Nama row shows the Movies on the Server most \
                             Accounts finished this week and last, once at least 20 did, so it \
                             stays hidden until enough have. Most watchlisted on Nama shows the \
                             Movies and Shows on most Accounts' Watchlists, once at least 20 \
                             Accounts have each, and offers those the Server lacks as Requests."
                .to_owned(),
            extension_points: vec![
                PointKey::provide::<dyn SimilarSource>(),
                PointKey::provide::<dyn HomeRowSource>(),
            ],
            logo_svg: Some(include_str!("../logo.svg").to_owned()),
            permissions: vec![
                Permission::PERMISSION_READ_LIBRARIES,
                Permission::PERMISSION_READ_WATCH_STATES,
            ],
            ..Manifest::new(Self::ID)
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        let kernel = registrar
            .kernel()
            .ok_or("the Kernel handed Hub Suggestions none of its functions")?;
        let similar: Arc<dyn SimilarSource> = Arc::new(Similar(Arc::clone(&kernel)));
        let rows: Arc<dyn HomeRowSource> = Arc::new(Rows {
            recommended: Recommended(Arc::clone(&kernel)),
            charts: Charts(kernel),
        });
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
#[path = "lib_tests.rs"]
mod tests;
