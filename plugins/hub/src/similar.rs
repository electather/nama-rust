//! Hub Suggestions as a Similar source (ADR 0084): the titles the Hub's vectors from Wikidata's
//! facts find alike (ADR 0095), by the External IDs the Server knows a title by.

use std::sync::Arc;

use nama_plugin_api::{
    BoxFuture, PluginError,
    automation::{Kernel, functions::ListHubSimilar},
    contract::{SimilarAsked, SimilarTitles},
    similar_source::SimilarSource,
};

/// Asks the Kernel which titles the Hub finds like one: none while the Server isn't Linked, which
/// isn't a failure, so the Server's other Similar sources answer alone.
pub(crate) struct Similar(pub(crate) Arc<dyn Kernel>);

impl SimilarSource for Similar {
    fn similar<'a>(
        &'a self,
        asked: &'a SimilarAsked,
    ) -> BoxFuture<'a, Result<SimilarTitles, PluginError>> {
        Box::pin(async { Ok(self.0.ask::<ListHubSimilar>(asked.clone()).await?) })
    }
}
