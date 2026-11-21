//! Asking the Search sources about a search, and how a viewer sees a title they find that no
//! Library has (ADR 0083).

use std::{sync::Arc, time::Duration};

use futures_util::future::join_all;
use nama_plugin_api::{
    PluginError, PluginId,
    contract::{self, SearchSourceQuery, SearchSourceResults, SearchSourceTitle},
    metadata::{ExternalId, ExternalIds},
    search_source::SearchSource,
};
use nama_proto::nama::v1::{Permission, Request};

use super::Requests;
use super::lookup::sees_followed_show;
use crate::{library::TitleKind, plugins::Answer, users::User};

/// How long a search waits for each Search source, so a slow one can't hold up its results
/// (ADR 0083). Fixed: there is no Admin setting for it.
const SEARCH_SOURCE_TIMEOUT: Duration = Duration::from_secs(5);

/// A title a Search source found.
#[derive(Debug)]
pub(crate) struct Sourced {
    pub(crate) kind: TitleKind,
    /// Every id the source gave it; any one finds it.
    pub(crate) ids: ExternalIds,
    pub(crate) title: String,
    pub(crate) year: Option<i32>,
    /// The poster's path, for the source Plugin's own `title_image` export.
    pub(crate) poster: Option<String>,
}

/// What one Search source answered a search.
#[derive(Debug)]
pub(crate) struct SourceAnswer {
    pub(crate) plugin: PluginId,
    /// Its titles, best first, or `None` if it failed or took too long.
    pub(crate) titles: Option<Vec<Sourced>>,
}

/// How a viewer sees a title a Search source found that no Library has.
#[derive(Debug)]
pub(crate) enum Unavailable {
    /// Requested, through its open Request the viewer may see, or a Show's Followed Show alone,
    /// which leaves `request` unset. `id` is the one a title source keeps, if any.
    Requested {
        id: ExternalId,
        request: Option<Box<Request>>,
    },
    /// Not on this Server, and the viewer may request it under this id.
    Requestable(ExternalId),
}

impl Requests {
    /// What every enabled Search source answers `query` with, in the Admin's precedence of
    /// Plugins, asked for `kind`, or both kinds when `None`, and `limit`. Each source is asked at
    /// once, and never told who is searching. Its titles of another kind, or with an id that isn't
    /// well formed, are dropped.
    pub(crate) async fn ask_search_sources(
        &self,
        query: &str,
        kind: Option<TitleKind>,
        limit: usize,
    ) -> Vec<SourceAnswer> {
        let asked = &SearchSourceQuery {
            query: query.to_owned(),
            kind: kind
                .map_or(contract::TitleKind::TITLE_KIND_UNSPECIFIED, wire_kind)
                .into(),
            limit: u32::try_from(limit).unwrap_or(u32::MAX),
            ..Default::default()
        };
        let sources = self.plugins.ranked::<dyn SearchSource>();
        let answers = (sources.into_iter()).map(|plugin| async move {
            let answer = (self.plugins)
                .ask(&plugin, |source: Arc<dyn SearchSource>| async move {
                    within_timeout(source.search(asked)).await
                })
                .await;
            let titles = match answer {
                Some(Answer {
                    result: Ok(results),
                    ..
                }) => Some(sourced(results, kind)),
                Some(Answer { result: Err(_), .. }) | None => None,
            };
            SourceAnswer { plugin, titles }
        });
        join_all(answers).await
    }

    /// The namespaces `viewer` may request a title of `kind` under: those the enabled title sources
    /// keep, or none if they may not request that kind.
    pub(crate) async fn requestable_namespaces(
        &self,
        viewer: &User,
        kind: TitleKind,
    ) -> Vec<String> {
        let permission = match kind {
            TitleKind::Movie => Permission::RequestMovie,
            TitleKind::Show => Permission::RequestTv,
        };
        if !viewer.permissions.allow(permission) {
            return Vec::new();
        }
        (self.metadata.sources().await.into_iter())
            .map(|source| source.namespace)
            .collect()
    }

    /// How `viewer` sees `title`, which no Library has: Requested by any of its ids, else
    /// requestable under its first id in one of `requestable`, the namespaces
    /// [`Self::requestable_namespaces`] gives; `None` if neither, so it isn't shown.
    pub(crate) async fn unavailable(
        &self,
        viewer: &User,
        title: &Sourced,
        requestable: &[String],
    ) -> Result<Option<Unavailable>, sqlx::Error> {
        let ids = &title.ids;
        let requestable_id = ids.iter().find(|id| {
            let namespace = id.namespace();
            requestable.iter().any(|kept| kept == namespace)
        });
        let Some(id) = requestable_id.clone().or_else(|| ids.iter().next()) else {
            return Ok(None);
        };
        let mut conn = self.db.reader().acquire().await?;
        let request = match title.kind {
            TitleKind::Movie => (self.visible_open_movie_request(&mut conn, viewer, ids)).await?,
            TitleKind::Show => (self.visible_open_season_request(&mut conn, viewer, ids)).await?,
        };
        if request.is_some()
            || (title.kind == TitleKind::Show && sees_followed_show(&mut conn, viewer, ids).await?)
        {
            let request = request.map(Box::new);
            return Ok(Some(Unavailable::Requested { id, request }));
        }
        Ok(requestable_id.map(Unavailable::Requestable))
    }
}

/// `searched`, failing as a Search source's error once [`SEARCH_SOURCE_TIMEOUT`] passes.
async fn within_timeout(
    searched: impl Future<Output = Result<SearchSourceResults, PluginError>>,
) -> Result<SearchSourceResults, PluginError> {
    (tokio::time::timeout(SEARCH_SOURCE_TIMEOUT, searched).await)
        .unwrap_or_else(|_| Err(format!("took longer than {SEARCH_SOURCE_TIMEOUT:?}").into()))
}

/// The titles of `results` of `kind`, or of either kind when `None`, as the Kernel reads them.
fn sourced(results: SearchSourceResults, kind: Option<TitleKind>) -> Vec<Sourced> {
    (results.titles.into_iter())
        .filter_map(read_title)
        .filter(|title| kind.is_none_or(|kind| kind == title.kind))
        .collect()
}

/// `title`, unless it is of neither kind or has an id that isn't well formed.
fn read_title(title: SearchSourceTitle) -> Option<Sourced> {
    let kind = match title.kind.as_known()? {
        contract::TitleKind::TITLE_KIND_MOVIE => TitleKind::Movie,
        contract::TitleKind::TITLE_KIND_SHOW => TitleKind::Show,
        contract::TitleKind::TITLE_KIND_UNSPECIFIED => return None,
    };
    let ids = (title.ids.iter())
        .map(|id| ExternalId::new(&id.namespace, id.value.clone()).ok())
        .collect::<Option<ExternalIds>>()?;
    Some(Sourced {
        kind,
        ids,
        title: title.title,
        year: title.year,
        poster: title.poster,
    })
}

const fn wire_kind(kind: TitleKind) -> contract::TitleKind {
    match kind {
        TitleKind::Movie => contract::TitleKind::TITLE_KIND_MOVIE,
        TitleKind::Show => contract::TitleKind::TITLE_KIND_SHOW,
    }
}
