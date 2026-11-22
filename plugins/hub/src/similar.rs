//! The Hub as a Similar source (ADR 0084): the titles its vectors from Wikidata's facts find alike
//! (ADR 0095), by the External IDs the Server knows a title by.

use std::sync::Arc;

use nama_hub_proto::nama::hub::v1::{self as hub, ListSimilarTitlesRequest};
use nama_plugin_api::{
    BoxFuture, PluginError,
    automation::Kernel,
    contract::{SimilarAsked, SimilarTitle, SimilarTitles, TitleId, TitleKind},
    similar_source::SimilarSource,
};

use crate::{hub::Hub, linker};

/// Asks the Hub, as the Linked Server, which titles are like one.
pub(crate) struct Similar {
    hub: Hub,
    kernel: Arc<dyn Kernel>,
}

impl Similar {
    pub(crate) fn new(hub: Hub, kernel: Arc<dyn Kernel>) -> Self {
        Self { hub, kernel }
    }

    /// The Hub's answer for `asked`: none while the Server isn't Linked, which isn't a failure, so
    /// the Server's other Similar sources answer alone.
    async fn ask(&self, asked: &SimilarAsked) -> Result<SimilarTitles, PluginError> {
        let kind = match asked.kind.as_known() {
            Some(TitleKind::TITLE_KIND_MOVIE) => hub::TitleKind::TITLE_KIND_MOVIE,
            Some(TitleKind::TITLE_KIND_SHOW) => hub::TitleKind::TITLE_KIND_SHOW,
            Some(TitleKind::TITLE_KIND_UNSPECIFIED) | None => return Ok(SimilarTitles::default()),
        };
        let Some(credential) = linker::credential(self.kernel.as_ref()).await? else {
            return Ok(SimilarTitles::default());
        };
        let request = ListSimilarTitlesRequest {
            kind: kind.into(),
            ids: asked
                .ids
                .iter()
                .map(|id| hub::ExternalId {
                    namespace: id.namespace.clone(),
                    value: id.value.clone(),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        };
        let similar = self.hub.similar(&credential, request).await?;
        Ok(SimilarTitles {
            titles: similar
                .titles
                .into_iter()
                .filter_map(similar_title)
                .collect(),
            ..Default::default()
        })
    }
}

impl SimilarSource for Similar {
    fn similar<'a>(
        &'a self,
        asked: &'a SimilarAsked,
    ) -> BoxFuture<'a, Result<SimilarTitles, PluginError>> {
        Box::pin(self.ask(asked))
    }
}

/// `similar` as the Kernel takes it, or `None` without an id, which no title could match.
fn similar_title(similar: hub::SimilarTitle) -> Option<SimilarTitle> {
    let id = similar.id.into_option()?;
    Some(SimilarTitle {
        id: TitleId {
            namespace: id.namespace,
            value: id.value,
            ..Default::default()
        }
        .into(),
        weight: similar.weight,
        ..Default::default()
    })
}
