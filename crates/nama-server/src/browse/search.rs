//! Searching the titles of the Libraries Browse sees.

use std::collections::HashSet;

use nama_plugin_api::metadata::{ExternalId, ExternalIds};

use super::{Browse, Error, finding_failed, library_kind};
use crate::library::{Found, TitleKind, Whereabouts};

impl Browse {
    /// The best `limit` Movies, Shows and Episodes of the Libraries it sees that a search for
    /// `query` finds, of `kind` only if it's set, as `Contents::search` finds them. A copy its
    /// Reach lets through is still left out when its title is [`Whereabouts::KeptFrom`] it.
    pub(crate) async fn search(
        &self,
        query: &str,
        kind: Option<TitleKind>,
        limit: usize,
    ) -> Result<Vec<Found>, Error> {
        let reaches = |found: &Found| self.reach.reaches(found.library(), found.content_rating());
        if self.reach.lets_everything() {
            return Ok(self.contents.search(query, kind, limit, reaches).await?);
        }
        // Every copy, so leaving out those of titles kept from them can't shorten the page.
        let mut found = self
            .contents
            .search(query, kind, usize::MAX, reaches)
            .await?;
        let libraries = self.libraries.list().await.map_err(finding_failed)?;
        let mut kept_from = HashSet::new();
        for of in [TitleKind::Movie, TitleKind::Show] {
            let ids: Vec<ExternalId> = (found.iter().map(title_of))
                .filter(|&(title_kind, _)| title_kind == of)
                .flat_map(|(_, ids)| ids.iter())
                .collect();
            let (reach, kind_in) = (&self.reach, library_kind(of));
            let placed = Whereabouts::of_titles(&self.contents, &libraries, reach, kind_in, &ids);
            kept_from.extend(
                (placed.await?.into_iter())
                    .filter(|(_, placed)| matches!(placed, Whereabouts::KeptFrom))
                    .map(|(id, _)| (of, id)),
            );
        }
        found.retain(|found| {
            let (of, ids) = title_of(found);
            !ids.iter().any(|id| kept_from.contains(&(of, id)))
        });
        found.truncate(limit);
        Ok(found)
    }
}

/// The kind and ids of the title `found` is of, an Episode's its Show's.
fn title_of(found: &Found) -> (TitleKind, &ExternalIds) {
    match found {
        Found::Movie(movie) => (TitleKind::Movie, &movie.ids),
        Found::Show(show) => (TitleKind::Show, &show.ids),
        Found::Episode(episode) => (TitleKind::Show, &episode.show_ids),
    }
}

#[cfg(test)]
#[path = "search_tests.rs"]
mod tests;
