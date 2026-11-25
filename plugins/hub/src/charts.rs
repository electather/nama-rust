//! Hub Suggestions as a Home row source (ADR 0058) of Nama's own charts, by the Hub's pooled counts (ADR
//! 0093): "Trending on Nama", the Movies most Accounts finished lately, of those the Server's
//! Libraries have, and "Most watchlisted on Nama", the titles on most Accounts' Watchlists, which
//! home offers as Requests where the Server lacks them.

use std::{collections::HashSet, sync::Arc};

use nama_plugin_api::{
    BoxFuture, PluginError,
    automation::{
        Kernel,
        functions::{ListHubMostWatchlisted, ListHubTrending, ListLibraries, ListTitles},
    },
    contract::{
        HomeRow, HomeRowTitle, HomeRows, HubChartAsked, HubTitle, LibrariesAsked, LibraryKind,
        SharedRowsAsked, TitlesAsked,
    },
    home_row::{HomeRowMethod, HomeRowSource},
};

/// How long a row stays fresh: the charts move over weeks, so a day costs them little.
const TTL_SECONDS: u32 = 24 * 60 * 60;

/// Asks the Kernel for Nama's charts from the Hub.
pub(crate) struct Charts(pub(crate) Arc<dyn Kernel>);

impl Charts {
    /// The rows: none while the Server isn't Linked or the Hub has no counts yet, and otherwise
    /// each empty, which home hides, until the Hub has enough of its own.
    async fn rows(&self) -> Result<HomeRows, PluginError> {
        let chart = HubChartAsked::default;
        let trending = self.0.ask::<ListHubTrending>(chart()).await?.titles;
        let watchlisted = self.0.ask::<ListHubMostWatchlisted>(chart()).await?.titles;
        // No rows rather than empty ones a day fresh, so the Kernel asks again within 15 minutes,
        // as the Server may Link meanwhile.
        if trending.is_empty() && watchlisted.is_empty() {
            return Ok(HomeRows::default());
        }
        Ok(HomeRows {
            rows: vec![
                self.trending(trending).await?,
                most_watchlisted(watchlisted),
            ],
            ..Default::default()
        })
    }

    /// "Trending on Nama" of `trending`, empty too while the Libraries have none of them.
    async fn trending(&self, trending: Vec<HubTitle>) -> Result<HomeRow, PluginError> {
        let movies = self.movies().await?;
        // Missing titles would show as Requests, but this row is only what Users can play.
        let titles = (trending.into_iter())
            .filter(|title| movies.contains(&(title.id.namespace.clone(), title.id.value.clone())))
            .map(home_row_title)
            .collect();
        Ok(row("trending", "Trending on Nama", titles))
    }

    /// The namespace and value of the id each Movie in the Server's Libraries keeps its Watch
    /// State by, as Viewing Events, and so the Hub's counts, name it.
    async fn movies(&self) -> Result<HashSet<(String, String)>, PluginError> {
        let libraries = self.0.ask::<ListLibraries>(LibrariesAsked::default());
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
            let titles = self.0.ask::<ListTitles>(asked).await?.titles;
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

/// "Most watchlisted on Nama" of `watchlisted`, in the Hub's order, whether or not the Server has
/// each.
fn most_watchlisted(watchlisted: Vec<HubTitle>) -> HomeRow {
    let titles = watchlisted.into_iter().map(home_row_title);
    row(
        "most-watchlisted",
        "Most watchlisted on Nama",
        titles.collect(),
    )
}

/// `title` as home takes it.
pub(crate) fn home_row_title(title: HubTitle) -> HomeRowTitle {
    HomeRowTitle {
        kind: title.kind,
        id: title.id,
        ..Default::default()
    }
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
