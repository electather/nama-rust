//! Searching the titles of the Libraries Browse sees.

use std::collections::HashMap;

use nama_plugin_api::metadata::{ExternalId, ExternalIds};

use super::{Browse, Error, finding_failed, library_kind};
use crate::library::{Found, TitleKind, Whereabouts};

impl Browse {
    /// The best `limit` Movies, Shows and Episodes of the Libraries it sees that a search for
    /// `query` finds, of `kind` only if it's set, as `Contents::search` finds them. A title is
    /// found only in the Library its [`Whereabouts`] places it in, so once at most, and not at all
    /// when it is kept from the User.
    pub(crate) async fn search(
        &self,
        query: &str,
        kind: Option<TitleKind>,
        limit: usize,
    ) -> Result<Vec<Found>, Error> {
        let reaches = |found: &Found| self.reach.reaches(found.library(), found.content_rating());
        // Every copy, so leaving out those found elsewhere can't shorten the page.
        let mut found = (self.contents)
            .search(query, kind, usize::MAX, reaches)
            .await?;
        let libraries = self.libraries.list().await.map_err(finding_failed)?;
        let mut placed_in = HashMap::new();
        for of in [TitleKind::Movie, TitleKind::Show] {
            let ids: Vec<ExternalId> = (found.iter().map(title_of))
                .filter(|&(title_kind, _)| title_kind == of)
                .flat_map(|(_, ids)| ids.iter())
                .collect();
            let (reach, kind_in) = (&self.reach, library_kind(of));
            let placed = Whereabouts::of_titles(&self.contents, &libraries, reach, kind_in, &ids);
            placed_in.extend(
                (placed.await?.into_iter())
                    .map(|(id, placed)| ((of, id), placed.reached().map(|title| title.library()))),
            );
        }
        found.retain(|found| {
            let (of, ids) = title_of(found);
            (ids.iter().filter_map(|id| placed_in.get(&(of, id))))
                .all(|&library| library == Some(found.library()))
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
