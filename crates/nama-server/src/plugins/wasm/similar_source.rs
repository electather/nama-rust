//! An Installed Plugin as a Similar source: the Kernel's `SimilarSource` calls passed through as
//! the contract's `SimilarSourceService` rpc.

use nama_plugin_api::{
    BoxFuture, PluginError,
    contract::{SimilarAsked, SimilarTitles},
    similar_source::SimilarSource,
};

use super::{Running, services::SIMILAR_TITLES};

impl SimilarSource for Running {
    fn similar<'a>(
        &'a self,
        asked: &'a SimilarAsked,
    ) -> BoxFuture<'a, Result<SimilarTitles, PluginError>> {
        Box::pin(self.call_method(&SIMILAR_TITLES, asked))
    }
}
