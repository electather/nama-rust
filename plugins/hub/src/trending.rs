//! The Hub as a Home row source (ADR 0058): "Trending on Nama", the Movies most Accounts finished
//! lately by the Hub's pooled counts (ADR 0093), of those the Server's Libraries have.

use std::{collections::HashSet, sync::Arc};

use nama_hub_proto::nama::hub::v1::TrendingTitle;
use nama_plugin_api::{
    BoxFuture, PluginError,
    automation::{
        Kernel,
        functions::{ListLibraries, ListTitles},
    },
    contract::{
        HomeRow, HomeRowTitle, HomeRows, LibrariesAsked, LibraryKind, SharedRowsAsked, TitleId,
        TitleKind, TitlesAsked,
    },
    home_row::{HomeRowMethod, HomeRowSource},
};

use crate::{hub::Hub, linker};

/// How long the row stays fresh: the Hub's counts are weekly, so a day costs it little.
const TTL_SECONDS: u32 = 24 * 60 * 60;

/// Asks the Hub, as the Linked Server, which Movies are trending.
pub(crate) struct Trending {
    hub: Hub,
    kernel: Arc<dyn Kernel>,
}

impl Trending {
    pub(crate) fn new(hub: Hub, kernel: Arc<dyn Kernel>) -> Self {
        Self { hub, kernel }
    }

    /// The row: none while the Server isn't Linked, and an empty one, which home hides, until the
    /// Hub has enough counts or the Libraries have none of what it answers.
    async fn rows(&self) -> Result<HomeRows, PluginError> {
        let Some(credential) = linker::credential(self.kernel.as_ref()).await? else {
            return Ok(HomeRows::default());
        };
        let trending = self.hub.trending(&credential).await?.titles;
        let movies = self.movies().await?;
        // Missing titles would show as Requests, but this row is only what Users can play.
        let titles = (trending.into_iter())
            .filter_map(movie_id)
            .filter(|id| movies.contains(&(id.namespace.clone(), id.value.clone())))
            .map(|id| HomeRowTitle {
                kind: TitleKind::TITLE_KIND_MOVIE.into(),
                id: id.into(),
                ..Default::default()
            })
            .collect();
        Ok(HomeRows {
            rows: vec![HomeRow {
                key: "trending".to_owned(),
                heading: "Trending on Nama".to_owned(),
                ttl_seconds: TTL_SECONDS,
                titles,
                ..Default::default()
            }],
            ..Default::default()
        })
    }

    /// The namespace and value of the id each Movie in the Server's Libraries keeps its Watch
    /// State by, as Viewing Events, and so the Hub's counts, name it.
    async fn movies(&self) -> Result<HashSet<(String, String)>, PluginError> {
        let libraries = self.kernel.ask::<ListLibraries>(LibrariesAsked::default());
        let mut movies = HashSet::new();
        for library in libraries.await?.libraries {
            if library.kind.as_known() != Some(LibraryKind::LIBRARY_KIND_MOVIES) {
                continue;
            }
            // ponytail: every Movie in one answer, once a day; page it if Libraries get huge.
            let asked = TitlesAsked {
                library_id: library.id,
                ..Default::default()
            };
            let titles = self.kernel.ask::<ListTitles>(asked).await?.titles;
            movies.extend(
                (titles.into_iter())
                    .filter_map(|title| title.title_ref.into_option()?.id.into_option())
                    .map(|id| (id.namespace, id.value)),
            );
        }
        Ok(movies)
    }
}

impl HomeRowSource for Trending {
    fn shared_rows<'a>(
        &'a self,
        _asked: &'a SharedRowsAsked,
    ) -> BoxFuture<'a, Result<HomeRows, PluginError>> {
        Box::pin(self.rows())
    }

    fn gives(&self, method: HomeRowMethod) -> bool {
        method == HomeRowMethod::Shared
    }
}

/// `trending`'s id as the Kernel takes it, or `None` without one.
fn movie_id(trending: TrendingTitle) -> Option<TitleId> {
    let id = trending.id.into_option()?;
    Some(TitleId {
        namespace: id.namespace,
        value: id.value,
        ..Default::default()
    })
}
