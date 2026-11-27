//! What a Linked Server asks of the Hub's titles: `LinkedServerService.ListSimilarTitles` and
//! `ListRecommendedTitles` and `ListTrendingTitles`, and the conversions they use.

use connectrpc::{RequestContext, Response, ServiceResult};
use nama_hub_proto::nama::hub::v1::{
    self as proto, ListRecommendedTitlesRequest, ListRecommendedTitlesResponse,
    ListSimilarTitlesRequest, ListSimilarTitlesResponse, ListTrendingTitlesResponse,
};

use super::{
    hub_failed,
    links::{LinkedServerServiceHandler, linked_server},
};
use crate::{
    titles::{self, ExternalId, RecommendedTitle, SimilarTitle, TitleKind},
    viewing::TrendingTitle,
};

pub(super) async fn list_similar_titles(
    handler: &LinkedServerServiceHandler,
    request: ListSimilarTitlesRequest,
) -> ServiceResult<ListSimilarTitlesResponse> {
    let kind = match request.kind.as_known() {
        Some(proto::TitleKind::TITLE_KIND_MOVIE) => TitleKind::Movie,
        Some(proto::TitleKind::TITLE_KIND_SHOW) => TitleKind::Show,
        Some(proto::TitleKind::TITLE_KIND_UNSPECIFIED) | None => {
            return Response::ok(ListSimilarTitlesResponse::default());
        }
    };
    let ids: Vec<ExternalId> = (request.ids.into_iter())
        .map(|id| ExternalId {
            namespace: id.namespace,
            value: id.value,
        })
        .collect();
    let similar = titles::similar(&handler.db, kind, &ids)
        .await
        .map_err(|err| hub_failed(&err))?;
    Response::ok(ListSimilarTitlesResponse {
        titles: similar.into_iter().map(similar_to_proto).collect(),
        ..Default::default()
    })
}

pub(super) async fn list_recommended_titles(
    handler: &LinkedServerServiceHandler,
    ctx: &RequestContext,
    request: ListRecommendedTitlesRequest,
) -> ServiceResult<ListRecommendedTitlesResponse> {
    let recommended = (handler.recommendations)
        .for_user(
            &handler.links,
            linked_server(ctx)?,
            request.user_id,
            &request.namespaces,
        )
        .await
        .map_err(|err| hub_failed(&err))?;
    Response::ok(ListRecommendedTitlesResponse {
        titles: recommended.into_iter().map(recommended_to_proto).collect(),
        ..Default::default()
    })
}

pub(super) async fn list_trending_titles(
    handler: &LinkedServerServiceHandler,
) -> ServiceResult<ListTrendingTitlesResponse> {
    let trending = (handler.viewing.trending())
        .await
        .map_err(|err| hub_failed(&err))?;
    Response::ok(ListTrendingTitlesResponse {
        titles: trending.iter().filter_map(trending_to_proto).collect(),
        ..Default::default()
    })
}

fn similar_to_proto(similar: SimilarTitle) -> proto::SimilarTitle {
    proto::SimilarTitle {
        id: id_to_proto(similar.id).into(),
        weight: similar.weight,
        ..Default::default()
    }
}

fn recommended_to_proto(recommended: RecommendedTitle) -> proto::RecommendedTitle {
    let kind = match recommended.kind {
        TitleKind::Movie => proto::TitleKind::TITLE_KIND_MOVIE,
        TitleKind::Show => proto::TitleKind::TITLE_KIND_SHOW,
    };
    proto::RecommendedTitle {
        kind: kind.into(),
        id: id_to_proto(recommended.id).into(),
        weight: recommended.weight,
        ..Default::default()
    }
}

fn id_to_proto(id: ExternalId) -> proto::ExternalId {
    proto::ExternalId {
        namespace: id.namespace,
        value: id.value,
        ..Default::default()
    }
}

/// `trending` as a Server reads it, or `None` for an External ID not written `namespace-value`,
/// which no Server sends.
fn trending_to_proto(trending: &TrendingTitle) -> Option<proto::TrendingTitle> {
    let (namespace, value) = trending.external_id.split_once('-')?;
    Some(proto::TrendingTitle {
        id: proto::ExternalId {
            namespace: namespace.to_owned(),
            value: value.to_owned(),
            ..Default::default()
        }
        .into(),
        weight: trending.weight,
        ..Default::default()
    })
}

#[cfg(test)]
#[path = "titles_tests.rs"]
mod tests;
