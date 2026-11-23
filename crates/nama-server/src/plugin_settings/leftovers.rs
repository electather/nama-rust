//! What a Plugin leaves behind in the database, and when each piece goes: "One owner for what a
//! Plugin leaves behind" in the crate's AGENTS.md. A Plugin reinstalled before its settings go
//! works again as it was left (ADR 0028).

use std::{sync::Arc, time::Duration};

use nama_plugin_api::PluginId;
use sqlx::SqliteConnection;
use tracing::error;

use super::PluginSettings;
use crate::{
    automation::delete_plugins_store_entries,
    connections,
    library::Contents,
    plugins::forget_undeclared_jobs,
    requests::delete_plugins_qualities,
    server_settings::Settings,
    users::{self, UserHomeLayout},
};

/// When a Plugin's rows in a table go, or why they stay.
#[cfg(test)]
pub(super) enum Goes {
    /// As an Admin disables the Plugin or it crashes: [`PluginSettings::forget_disabled`].
    Disabled,
    /// As startup or a listing finds the Plugin uninstalled: [`PluginSettings::forget_uninstalled`].
    Uninstalled,
    /// On the next listing of Home or Featured, whose sweep also drops what an enabled Plugin's
    /// update stopped giving: `HomeRows::list` and `HomeRows::featured`.
    NextHomeVisit,
    /// Once the Server hasn't had the Plugin for Upkeep's set number of days:
    /// [`PluginSettings::forget_gone`].
    GoneForDays,
    /// Never, for the reason given.
    Kept(
        #[expect(
            dead_code,
            reason = "the reason is for the reader; no check can judge it"
        )]
        &'static str,
    ),
}

/// Every table keyed by a Plugin, with when its rows go; a table whose rows go at two moments is
/// listed twice.
#[cfg(test)]
pub(super) const LEFTOVERS: &[(&str, Goes)] = &[
    ("similar_titles", Goes::Disabled),
    ("job_overrides", Goes::Uninstalled),
    ("job_runs", Goes::Uninstalled),
    // The default Home layout's rows.
    ("server_settings", Goes::Uninstalled),
    ("user_home_layouts", Goes::Uninstalled),
    ("home_rows", Goes::NextHomeVisit),
    ("home_row_refreshes", Goes::NextHomeVisit),
    ("featured_suggestions", Goes::NextHomeVisit),
    ("featured_refreshes", Goes::NextHomeVisit),
    ("plugins_had", Goes::GoneForDays),
    ("plugin_settings", Goes::GoneForDays),
    ("plugin_store", Goes::GoneForDays),
    ("personal_settings", Goes::GoneForDays),
    ("plugin_connections", Goes::GoneForDays),
    ("title_details", Goes::GoneForDays),
    ("box_sets", Goes::GoneForDays),
    // A Plugin removed while enabled was never disabled.
    ("similar_titles", Goes::GoneForDays),
    ("title_roles", Goes::GoneForDays),
    ("episode_air_dates", Goes::GoneForDays),
    ("qualities", Goes::GoneForDays),
    ("default_qualities", Goes::GoneForDays),
    (
        "requests",
        Goes::Kept("a Requester's history names its Fulfiller (ADR 0079)"),
    ),
    (
        "item_matches",
        Goes::Kept("a Match names its title source and its artwork's Plugin"),
    ),
    (
        "segments",
        Goes::Kept("they fit the Version and are replaced when it is asked again"),
    ),
    (
        "sign_in_methods",
        Goes::Kept("a User must keep a way to sign in"),
    ),
    (
        "libraries",
        Goes::Kept(
            "its title source, Recogniser and Segment source orders are Admin configuration",
        ),
    ),
    (
        "server_settings",
        Goes::Kept("the image store is Admin configuration (ADR 0052)"),
    ),
];

/// The columns naming a Plugin that the migrations' naming, `plugin` in the name, misses.
#[cfg(test)]
pub(super) const OLDER_COLUMNS: &[(&str, &str)] = &[
    ("libraries", "title_sources"),
    ("libraries", "recognisers"),
    ("libraries", "recognisers_read"),
    ("libraries", "segment_sources"),
    ("segments", "source"),
    ("server_settings", "default_home_layout"),
    ("sign_in_methods", "provider_id"),
    ("user_home_layouts", "layout"),
];

impl PluginSettings {
    /// Deletes what goes as the Plugin `id` is disabled or crashes: its Similar titles (ADR 0084),
    /// so enabling it again shows them only as its titles refresh. Pages already leave out a
    /// disabled Plugin's, so a failure is only logged.
    pub(super) async fn forget_disabled(&self, id: &PluginId) {
        let contents = Contents::new(Arc::clone(&self.db));
        if let Err(err) = contents.forget_similar_from(id).await {
            error!(plugin = %id, %err, "couldn't delete a disabled Plugin's Similar titles");
        }
    }

    /// Deletes what goes as the Server finds a Plugin uninstalled: the overrides and Job Runs of
    /// Jobs no Plugin declares (ADR 0079), and the rows of Plugins it no longer has from the
    /// default Home layout held in `settings` and from each User's (ADR 0077). Startup and the
    /// Admin's listing call it. Each failure is logged, which leaves those rows until the next.
    pub(crate) async fn forget_uninstalled(&self, settings: &Settings) {
        forget_undeclared_jobs(&self.db, &self.registry.plugins()).await;
        let installed = |plugin: &PluginId| self.has(plugin);
        if let Err(err) = settings.forget_uninstalled(installed).await {
            error!("could not drop uninstalled Plugins' rows from the Home layout: {err}");
        }
        if let Err(err) = UserHomeLayout::forget_uninstalled(&self.db, installed).await {
            error!("could not drop uninstalled Plugins' rows from Users' Home layouts: {err}");
        }
    }

    /// Deletes, in `conn`'s write, everything `LEFTOVERS` says goes once the Server hasn't had a
    /// Plugin for `gone_for`, and returns those Plugins' ids, sorted. Upkeep's Kernel function
    /// calls it.
    pub(crate) async fn forget_gone(
        &self,
        conn: &mut SqliteConnection,
        gone_for: Duration,
    ) -> Result<Vec<String>, sqlx::Error> {
        let gone = self.take_gone(conn, gone_for).await?;
        for plugin in &gone {
            delete_plugins_store_entries(conn, plugin).await?;
            users::delete_plugins_personal_settings(conn, plugin).await?;
            connections::delete_plugin(conn, plugin).await?;
            Contents::forget_plugin(conn, plugin).await?;
            delete_plugins_qualities(conn, plugin).await?;
        }
        Ok(gone)
    }
}

#[cfg(test)]
#[path = "leftovers_tests.rs"]
mod tests;
