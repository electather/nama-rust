//! What the Hub suggests to the Linked Server, which Plugins ask for through the Kernel's
//! `list_hub_*` functions (ADR 0105), answered in the Plugin contract's terms: none without a Hub,
//! while the Server isn't Linked or once the Hub forgot it, so unlinked it loses nothing (ADR 0002).

use std::future::Future;

use nama_hub_proto::nama::hub::v1::{
    self as hub_api, ListRecommendedTitlesRequest, ListSimilarTitlesRequest,
};
use nama_plugin_api::contract::{
    HubRecommendedAsked, HubTitle, HubTitles, SimilarAsked, SimilarTitle, SimilarTitles, TitleId,
    TitleKind,
};

use super::{Error, Linking, hub::Failure, hub::LinkedHub};

impl Linking {
    /// The titles the Hub finds like the one `asked` names, of its kind; none for a kind neither
    /// Movie nor Show.
    pub(crate) async fn similar_titles(
        &self,
        asked: &SimilarAsked,
    ) -> Result<SimilarTitles, Error> {
        let Some(kind) = asked.kind.as_known().and_then(kind_to_hub) else {
            return Ok(SimilarTitles::default());
        };
        let request = ListSimilarTitlesRequest {
            kind: kind.into(),
            ids: asked.ids.iter().map(id_to_hub).collect(),
            ..Default::default()
        };
        let similar = self.suggested(|hub| hub.similar(request)).await?;
        Ok(SimilarTitles {
            titles: (similar.into_iter())
                .filter_map(|title| {
                    Some(SimilarTitle {
                        id: id_from_hub(title.id.into_option()?).into(),
                        weight: title.weight,
                        ..Default::default()
                    })
                })
                .collect(),
            ..Default::default()
        })
    }

    /// The titles the Hub recommends to the User `asked` names, by ids in its namespaces.
    pub(crate) async fn recommended_titles(
        &self,
        asked: HubRecommendedAsked,
    ) -> Result<HubTitles, Error> {
        let request = ListRecommendedTitlesRequest {
            user_id: asked.user_id,
            namespaces: asked.namespaces,
            ..Default::default()
        };
        let recommended = self.suggested(|hub| hub.recommended(request)).await?;
        Ok(hub_titles(recommended.into_iter().map(|title| {
            (title.kind.as_known(), title.id.into_option(), title.weight)
        })))
    }

    /// The Movies trending across Nama's Accounts.
    pub(crate) async fn trending_titles(&self) -> Result<HubTitles, Error> {
        let trending = self.suggested(LinkedHub::trending).await?;
        Ok(hub_titles(trending.into_iter().map(|title| {
            let movie = Some(hub_api::TitleKind::TITLE_KIND_MOVIE);
            (movie, title.id.into_option(), title.weight)
        })))
    }

    /// The titles on most of Nama's Accounts' Watchlists.
    pub(crate) async fn most_watchlisted_titles(&self) -> Result<HubTitles, Error> {
        let watchlisted = self.suggested(LinkedHub::most_watchlisted).await?;
        Ok(hub_titles(watchlisted.into_iter().map(|title| {
            (title.kind.as_known(), title.id.into_option(), title.weight)
        })))
    }

    /// What `call` answers as the Linked Server: none without a Hub or a Link.
    async fn suggested<T, Called>(
        &self,
        call: impl FnOnce(LinkedHub) -> Called,
    ) -> Result<Vec<T>, Error>
    where
        Called: Future<Output = Result<Vec<T>, Failure>>,
    {
        match self.ask_hub(async |server| server.call(call).await).await {
            Ok(titles) => Ok(titles.unwrap_or_default()),
            Err(Error::NoHub) => Ok(Vec::new()),
            Err(Error::Failed) => Err(Error::Failed),
        }
    }
}

/// `titles`, each a kind, an id and a weight, leaving out one without a known kind or an id, which
/// no title could match.
fn hub_titles(
    titles: impl Iterator<Item = (Option<hub_api::TitleKind>, Option<hub_api::ExternalId>, f32)>,
) -> HubTitles {
    let titles = titles.filter_map(|(kind, id, weight)| {
        Some(HubTitle {
            kind: kind_from_hub(kind?)?.into(),
            id: id_from_hub(id?).into(),
            weight,
            ..Default::default()
        })
    });
    HubTitles {
        titles: titles.collect(),
        ..Default::default()
    }
}

const fn kind_to_hub(kind: TitleKind) -> Option<hub_api::TitleKind> {
    match kind {
        TitleKind::TITLE_KIND_MOVIE => Some(hub_api::TitleKind::TITLE_KIND_MOVIE),
        TitleKind::TITLE_KIND_SHOW => Some(hub_api::TitleKind::TITLE_KIND_SHOW),
        TitleKind::TITLE_KIND_UNSPECIFIED => None,
    }
}

const fn kind_from_hub(kind: hub_api::TitleKind) -> Option<TitleKind> {
    match kind {
        hub_api::TitleKind::TITLE_KIND_MOVIE => Some(TitleKind::TITLE_KIND_MOVIE),
        hub_api::TitleKind::TITLE_KIND_SHOW => Some(TitleKind::TITLE_KIND_SHOW),
        hub_api::TitleKind::TITLE_KIND_UNSPECIFIED => None,
    }
}

fn id_to_hub(id: &TitleId) -> hub_api::ExternalId {
    hub_api::ExternalId {
        namespace: id.namespace.clone(),
        value: id.value.clone(),
        ..Default::default()
    }
}

fn id_from_hub(id: hub_api::ExternalId) -> TitleId {
    TitleId {
        namespace: id.namespace,
        value: id.value,
        ..Default::default()
    }
}
