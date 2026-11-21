//! An Installed Plugin as a Search source: the Kernel's `SearchSource` calls passed through as the
//! contract's `SearchSourceService` rpc.

use nama_plugin_api::{
    BoxFuture, PluginError,
    contract::{SearchSourceQuery, SearchSourceResults},
    search_source::SearchSource,
};

use super::{Running, services::SEARCH_TITLES};

impl SearchSource for Running {
    fn search<'a>(
        &'a self,
        query: &'a SearchSourceQuery,
    ) -> BoxFuture<'a, Result<SearchSourceResults, PluginError>> {
        Box::pin(self.call_method(&SEARCH_TITLES, query))
    }
}
