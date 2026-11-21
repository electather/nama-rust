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

/// What fetching Matched titles again did.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Refreshed {
    /// Titles, a Movie or a Show, fetched again and stored.
    pub(crate) titles: u64,
    /// The Plugins whose titles were left as they were, as they couldn't answer.
    pub(crate) unavailable: BTreeSet<PluginId>,
}

/// The Matches stored at an older metadata version than asked, by the Plugin that Matched them
/// and the id of their title: a Movie, or the Show an Episode is in.
#[derive(Debug, Default)]
struct Outdated {
    movies: BTreeMap<(PluginId, ExternalId), Vec<i64>>,
    shows: BTreeMap<(PluginId, ExternalId), Vec<PendingEpisode>>,
}

impl Matcher {
    /// Fetches every Matched title of every Library again by the ids its Match has, whatever its
    /// metadata version, and stores what it gets, until `cancel` is cancelled. Each Library is
    /// refreshed under its scan lock, so it waits for a running scan. A title that can't be
    /// fetched keeps what it had, for the next refresh to try again.
    pub(crate) async fn refresh_all(&self, cancel: &CancellationToken) -> Result<Refreshed, Error> {
        let mut all = Refreshed::default();
        for library in self.libraries.list().await? {
            // A Library with no Matches never reaches `refresh`'s own check.
            if cancel.is_cancelled() {
                return Err(Error::Cancelled);
            }
            let _scan = self.locks.lock(library.id).await;
            let ordered = self.metadata.in_order(library.title_sources);
            let refreshed = self.refresh(library.id, &ordered, i64::MAX, cancel).await?;
            all.titles += refreshed.titles;
            all.unavailable.extend(refreshed.unavailable);
            self.artwork.drop_unused(&self.contents).await;
            (self.artwork).download_missing(self.contents.clone(), library.id);
        }
        Ok(all)
    }

    /// Fetches the titles of the Library `library`'s Matches stored at a metadata version below
    /// `below` again and stores what it gets, until `cancel` is cancelled. Each title's artwork is
    /// picked again from `ordered`, the Library's sources in its order, while the title comes
    /// only from its Plugin.
    pub(super) async fn refresh(
        &self,
        library: i64,
        ordered: &TitleSources,
        below: i64,
        cancel: &CancellationToken,
    ) -> Result<Refreshed, Error> {
        let Outdated { movies, shows } = self.outdated(library, below).await?;
        // Once a Plugin fails (or is disabled or gone), asking it about its other titles would
        // only fail too, but another Plugin's Matches can still be fetched.
        let Refreshed {
            mut titles,
            mut unavailable,
        } = Refreshed::default();
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
            titles += 1;
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
            } else {
                titles += 1;
            }
            self.store_refreshed(found).await?;
        }
        Ok(Refreshed {
            titles,
            unavailable,
        })
    }

    async fn outdated(&self, library: i64, below: i64) -> Result<Outdated, sqlx::Error> {
        let mut outdated = Outdated::default();
        let hints = self.contents.nfo_hints(library).await?;
        for (id, item, plugin, title) in self.contents.matched_below(library, below).await? {
            let key = (plugin, title);
            match PendingEpisode::of(id, &item, &hints) {
                Some(episode) => outdated.shows.entry(key).or_default().push(episode),
                None => outdated.movies.entry(key).or_default().push(id),
            }
        }
        Ok(outdated)
    }

    /// Stores the Matches found again in `found`. An item missed keeps the Match it has, since a
    /// refresh never changes which title an item Matches.
    async fn store_refreshed(&self, found: Outcomes) -> Result<(), sqlx::Error> {
        let found = self.with_similar(found).await;
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

#[cfg(test)]
#[path = "refresh_tests.rs"]
mod tests;
