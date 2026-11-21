//! An Installed Plugin as a Segment source: the Kernel's `SegmentSource` calls passed through as
//! the contract's `SegmentSourceService` rpc.

use nama_plugin_api::{
    BoxFuture, PluginError,
    contract::{SegmentResults, SegmentSearch},
    segment_source::SegmentSource,
};

use super::{Running, services::SEGMENT_SEARCH, try_again};

impl SegmentSource for Running {
    fn search<'a>(
        &'a self,
        search: &'a SegmentSearch,
    ) -> BoxFuture<'a, Result<SegmentResults, PluginError>> {
        Box::pin(async move {
            let results: SegmentResults = self.call_method(&SEGMENT_SEARCH, search).await?;
            try_again(results.retry_after_seconds).map_or(Ok(results), |wait| Err(wait.into()))
        })
    }
}
