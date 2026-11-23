//! The Hub as a Home row source (ADR 0058) of Nama's own charts, by the Hub's pooled counts (ADR
//! 0093): "Trending on Nama", the Movies most Accounts finished lately, of those the Server's
//! Libraries have, and "Most watchlisted on Nama", the titles on most Accounts' Watchlists, which
//! home offers as Requests where the Server lacks them.

use std::{collections::HashSet, sync::Arc};

use nama_hub_proto::nama::hub::v1::{self as hub_api, TrendingTitle, WatchlistedTitle};
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

/// How long a row stays fresh: the charts move over weeks, so a day costs them little.
const TTL_SECONDS: u32 = 24 * 60 * 60;

/// Asks the Hub, as the Linked Server, for Nama's charts.
pub(crate) struct Charts {
    hub: Hub,
    kernel: Arc<dyn Kernel>,
}

impl Charts {
    /// Nama's charts from `hub`, as the Server `kernel` serves.
    pub(crate) fn new(hub: Hub, kernel: Arc<dyn Kernel>) -> Self {
        Self { hub, kernel }
    }

    /// The rows: none while the Server isn't Linked, and each empty, which home hides, until the
    /// Hub has enough counts.
    async fn rows(&self) -> Result<HomeRows, PluginError> {
        let Some(credential) = linker::credential(self.kernel.as_ref()).await? else {
            return Ok(HomeRows::default());
        };
        Ok(HomeRows {
            rows: vec![
                self.trending(&credential).await?,
                self.most_watchlisted(&credential).await?,
            ],
            ..Default::default()
        })
    }

    /// "Trending on Nama", empty too while the Libraries have none of what the Hub answers.
    async fn trending(&self, credential: &str) -> Result<HomeRow, PluginError> {
        let trending = self.hub.trending(credential).await?.titles;
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
        Ok(row("trending", "Trending on Nama", titles))
    }

    /// "Most watchlisted on Nama", in the Hub's order, whether or not the Server has each.
    async fn most_watchlisted(&self, credential: &str) -> Result<HomeRow, PluginError> {
        let watchlisted = self.hub.most_watchlisted(credential).await?.titles;
        let titles = watchlisted.into_iter().filter_map(watchlisted_title);
        Ok(row(
            "most-watchlisted",
            "Most watchlisted on Nama",
            titles.collect(),
        ))
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

impl HomeRowSource for Charts {
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

/// `watchlisted` as home takes it, or `None` without a kind or an id.
fn watchlisted_title(watchlisted: WatchlistedTitle) -> Option<HomeRowTitle> {
    let kind = match watchlisted.kind.as_known()? {
        hub_api::TitleKind::TITLE_KIND_MOVIE => TitleKind::TITLE_KIND_MOVIE,
        hub_api::TitleKind::TITLE_KIND_SHOW => TitleKind::TITLE_KIND_SHOW,
        hub_api::TitleKind::TITLE_KIND_UNSPECIFIED => return None,
    };
    let id = watchlisted.id.into_option()?;
    Some(HomeRowTitle {
        kind: kind.into(),
        id: TitleId {
            namespace: id.namespace,
            value: id.value,
            ..Default::default()
        }
        .into(),
        ..Default::default()
    })
}

/// The row keyed `key`, under `heading`, of `titles`.
fn row(key: &str, heading: &str, titles: Vec<HomeRowTitle>) -> HomeRow {
    HomeRow {
        key: key.to_owned(),
        heading: heading.to_owned(),
        ttl_seconds: TTL_SECONDS,
        titles,
        ..Default::default()
    }
}
