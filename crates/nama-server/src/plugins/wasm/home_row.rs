//! An Installed Plugin as a Home row source: the Kernel's `HomeRowSource` calls passed through as
//! the contract's `HomeRowService` rpcs.

use nama_plugin_api::{
    BoxFuture, PluginError,
    contract::{HomeRows, SharedRowsAsked},
    home_row::HomeRowSource,
};

use super::{Running, services::SHARED_ROWS};

impl HomeRowSource for Running {
    fn shared_rows<'a>(
        &'a self,
        asked: &'a SharedRowsAsked,
    ) -> BoxFuture<'a, Result<HomeRows, PluginError>> {
        Box::pin(self.call_method(&SHARED_ROWS, asked))
    }
}
