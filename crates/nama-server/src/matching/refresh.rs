//! Fetching Matched titles again by the ids their Matches have, and storing what comes back
//! (ADR 0040).

use std::collections::{BTreeMap, BTreeSet};

use nama_plugin_api::{
    PluginId,
    metadata::{ExternalId, ExternalIds},
};
use tokio_util::sync::CancellationToken;

use super::{Error, Matcher, Outcomes, PendingEpisode, sources};
use crate::library::{Decided, Decisions, TitleSources, Unanswered};

/// The Matches stored at an older [`METADATA_VERSION`], by the Plugin that Matched them and the id
/// of their title: a Movie, or the Show an Episode is in.
///
/// [`METADATA_VERSION`]: crate::library::METADATA_VERSION
#[derive(Debug, Default)]
struct Outdated {
    movies: BTreeMap<(PluginId, ExternalId), Vec<i64>>,
    shows: BTreeMap<(PluginId, ExternalId), Vec<PendingEpisode>>,
}

impl Matcher {
    /// Fetches the titles of the Library `library`'s Matches stored at an older
    /// [`METADATA_VERSION`] again and stores what it gets, until `cancel` is cancelled. Returns
    /// whether every title source asked could answer. Each title's artwork is picked again from
    /// `ordered`, the Library's sources in its order, while the title comes only from its Plugin.
    ///
    /// [`METADATA_VERSION`]: crate::library::METADATA_VERSION
    pub(super) async fn refresh(
        &self,
        library: i64,
        ordered: &TitleSources,
        cancel: &CancellationToken,
    ) -> Result<bool, Error> {
        let Outdated { movies, shows } = self.outdated(library).await?;
        // Once a Plugin fails (or is disabled or gone), asking it about its other titles would
        // only fail too, but another Plugin's Matches can still be fetched.
        let mut unavailable = BTreeSet::new();
        for ((plugin, id), items) in movies {
            if cancel.is_cancelled() {
                return Err(Error::Cancelled);
            }
            if unavailable.contains(&plugin) {
                continue;
            }
            let movie = match self.only(plugin.clone()).metadata.movie(&id).await {
                Ok(movie) => self.picked_for(ordered, movie).await,
                Err(Unanswered::NotFound) => continue,
                Err(Unanswered::Unavailable) => {
                    unavailable.insert(plugin);
                    continue;
                }
            };
            let found = items.into_iter().map(|item| (item, Ok(movie.clone())));
            self.store_refreshed(found.collect()).await?;
        }
        for ((plugin, id), episodes) in shows {
            if cancel.is_cancelled() {
                return Err(Error::Cancelled);
            }
            if unavailable.contains(&plugin) {
                continue;
            }
            let only = self.only(plugin.clone());
            let show = match sources::fetch_show(&only.metadata, &id, ExternalIds::default()).await
            {
                Ok(show) => show,
                Err(Unanswered::NotFound) => continue,
                Err(Unanswered::Unavailable) => {
                    unavailable.insert(plugin);
                    continue;
                }
            };
            let picked = self.pick_show(ordered, &show).await;
            let found = only.match_in_show(&show, &picked, &episodes).await;
            if found.any_undecided() {
                unavailable.insert(plugin);
            }
            self.store_refreshed(found).await?;
        }
        Ok(unavailable.is_empty())
    }

    async fn outdated(&self, library: i64) -> Result<Outdated, sqlx::Error> {
        let mut outdated = Outdated::default();
        for (id, item, plugin, title) in self.contents.outdated(library).await? {
            let key = (plugin, title);
            match PendingEpisode::of(id, &item) {
                Some(episode) => outdated.shows.entry(key).or_default().push(episode),
                None => outdated.movies.entry(key).or_default().push(id),
            }
        }
        Ok(outdated)
    }

    /// Stores the Matches found again in `found`. An item missed keeps the Match it has, since a
    /// refresh never changes which title an item Matches.
    async fn store_refreshed(&self, found: Outcomes) -> Result<(), sqlx::Error> {
        let items = (found.items.into_iter())
            .filter_map(|(item, outcome)| Some((item, Decided::Matched(Box::new(outcome.ok()?)))))
            .collect();
        let decided = Decisions {
            items,
            seasons: found.seasons,
        };
        self.events.publish(self.contents.record(decided).await?);
        Ok(())
    }
}
