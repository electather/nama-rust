//! What a Linked Server is told of the Hub's titles, as `LinkedServerService` answers it.

use nama_hub_proto::nama::hub::v1 as proto;

use crate::{
    titles::{ExternalId, RecommendedTitle, SimilarTitle, TitleKind},
    viewing::TrendingTitle,
};

pub(super) fn similar_to_proto(similar: SimilarTitle) -> proto::SimilarTitle {
    proto::SimilarTitle {
        id: id_to_proto(similar.id).into(),
        weight: similar.weight,
        ..Default::default()
    }
}

pub(super) fn recommended_to_proto(recommended: RecommendedTitle) -> proto::RecommendedTitle {
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
pub(super) fn trending_to_proto(trending: &TrendingTitle) -> Option<proto::TrendingTitle> {
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
