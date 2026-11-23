//! The Hub as a Home row source (ADR 0058, 0072): the titles it recommends to each Linked User from
//! their Account's taste vector (ADR 0095), as a "Recommended for you" row and as Featured
//! suggestions, by External ID with weights. The Kernel stores both with their time to live.

use std::{collections::HashSet, sync::Arc};

use nama_hub_proto::nama::hub::v1::{self as hub, ListRecommendedTitlesRequest};
use nama_plugin_api::{
    BoxFuture, PluginError,
    automation::{
        Kernel,
        functions::{ListLibraries, ListTitles},
    },
    contract::{
        FeaturedSuggestion, FeaturedSuggestions, HomeRow, HomeRowTitle, HomeRows, LibrariesAsked,
        Title, TitleId, TitleKind, TitlesAsked, UserFeaturedAsked, UserRowsAsked,
    },
    home_row::{HomeRowMethod, HomeRowSource},
};

use crate::linker::Linker;

/// How long an answer stays fresh: the Hub learns a title was finished only once its play ends,
/// after the Kernel asked again at its start, so an hour keeps a finished title from lingering.
const FRESH_FOR_SECONDS: u32 = 60 * 60;

/// The namespaces the Hub answers in, most preferred first.
///
/// ponytail: the ids the Server's Built-in title sources Match by, whatever the Libraries use;
/// ask the Kernel for the namespaces its titles have if a title source adds another.
const NAMESPACES: [&str; 3] = ["tmdb", "tvdb", "imdb"];

/// Asks the Hub, as the Linked Server, what to recommend to each of its Users.
pub(crate) struct Recommended {
    linker: Arc<Linker>,
    kernel: Arc<dyn Kernel>,
}

impl Recommended {
    pub(crate) fn new(linker: Arc<Linker>, kernel: Arc<dyn Kernel>) -> Self {
        Self { linker, kernel }
    }

    /// The Hub's recommendations for the User `user_id`, each with its kind: none while the Server
    /// isn't Linked, which isn't a failure.
    async fn titles(&self, user_id: i64) -> Result<Vec<(TitleKind, TitleId, f32)>, PluginError> {
        let request = ListRecommendedTitlesRequest {
            user_id,
            namespaces: NAMESPACES.map(str::to_owned).to_vec(),
            ..Default::default()
        };
        let recommended = self.linker.call(|hub| hub.recommended(request)).await?;
        Ok((recommended.unwrap_or_default().into_iter())
            .filter_map(recommended_title)
            .collect())
    }

    async fn rows(&self, user_id: i64) -> Result<HomeRows, PluginError> {
        let titles: Vec<HomeRowTitle> = (self.titles(user_id).await?.into_iter())
            .map(|(kind, id, _weight)| HomeRowTitle {
                kind: kind.into(),
                id: id.into(),
                ..Default::default()
            })
            .collect();
        let row = HomeRow {
            key: "recommended".to_owned(),
            heading: "Recommended for you".to_owned(),
            ttl_seconds: FRESH_FOR_SECONDS,
            titles,
            ..Default::default()
        };
        Ok(HomeRows {
            rows: (!row.titles.is_empty())
                .then_some(row)
                .into_iter()
                .collect(),
            ..Default::default()
        })
    }

    /// The User `user_id`'s recommendations some Library has: the Kernel keeps only the first 20
    /// suggestions before dropping those, since Featured's main button is Play (ADR 0072).
    async fn featured(&self, user_id: i64) -> Result<FeaturedSuggestions, PluginError> {
        let titles = self.titles(user_id).await?;
        let on_server = if titles.is_empty() {
            HashSet::new()
        } else {
            self.on_server().await?
        };
        let suggestions = (titles.into_iter())
            .filter(|(kind, id, _weight)| {
                on_server.contains(&(*kind, id.namespace.clone(), id.value.clone()))
            })
            .map(|(kind, id, weight)| FeaturedSuggestion {
                kind: kind.into(),
                id: id.into(),
                weight,
                ..Default::default()
            })
            .collect();
        Ok(FeaturedSuggestions {
            suggestions,
            ttl_seconds: FRESH_FOR_SECONDS,
            ..Default::default()
        })
    }
}

impl Recommended {
    /// Every id of every Movie and Show in the Server's Libraries, with its kind.
    ///
    /// ponytail: lists every title of every Library each time the Kernel asks for a User's
    /// suggestions; ask the Kernel for the titles by id once that is measured slow.
    async fn on_server(&self) -> Result<HashSet<(TitleKind, String, String)>, PluginError> {
        let libraries = self
            .kernel
            .ask::<ListLibraries>(LibrariesAsked::default())
            .await?;
        let mut on_server = HashSet::new();
        for library in libraries.libraries {
            let asked = TitlesAsked {
                library_id: library.id,
                ..Default::default()
            };
            let titles = self.kernel.ask::<ListTitles>(asked).await?.titles;
            on_server.extend(titles.into_iter().flat_map(kinded_ids));
        }
        Ok(on_server)
    }
}

impl HomeRowSource for Recommended {
    fn user_rows<'a>(
        &'a self,
        asked: &'a UserRowsAsked,
    ) -> BoxFuture<'a, Result<HomeRows, PluginError>> {
        Box::pin(self.rows(asked.user_id))
    }

    fn user_featured<'a>(
        &'a self,
        asked: &'a UserFeaturedAsked,
    ) -> BoxFuture<'a, Result<FeaturedSuggestions, PluginError>> {
        Box::pin(self.featured(asked.user_id))
    }

    fn gives(&self, method: HomeRowMethod) -> bool {
        method != HomeRowMethod::Shared
    }
}

/// Every id of `title`, with its kind. An Episode stands for its Show, which is what the Hub
/// recommends.
fn kinded_ids(title: Title) -> impl Iterator<Item = (TitleKind, String, String)> {
    let (kind, ids) = if title.show_ids.is_empty() {
        (TitleKind::TITLE_KIND_MOVIE, title.ids)
    } else {
        (TitleKind::TITLE_KIND_SHOW, title.show_ids)
    };
    ids.into_iter()
        .map(move |id| (kind, id.namespace, id.value))
}

/// `recommended` as the Kernel takes it, or `None` without an id or a kind, which no title could
/// match.
fn recommended_title(recommended: hub::RecommendedTitle) -> Option<(TitleKind, TitleId, f32)> {
    let kind = match recommended.kind.as_known()? {
        hub::TitleKind::TITLE_KIND_MOVIE => TitleKind::TITLE_KIND_MOVIE,
        hub::TitleKind::TITLE_KIND_SHOW => TitleKind::TITLE_KIND_SHOW,
        hub::TitleKind::TITLE_KIND_UNSPECIFIED => return None,
    };
    let id = recommended.id.into_option()?;
    let id = TitleId {
        namespace: id.namespace,
        value: id.value,
        ..Default::default()
    };
    Some((kind, id, recommended.weight))
}
