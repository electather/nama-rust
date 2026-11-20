//! An Installed Plugin as a Home row source: the Kernel's `HomeRowSource` calls passed through as
//! the contract's `HomeRowService` rpcs.

use buffa::Message;
use nama_plugin_api::{
    BoxFuture, PluginError,
    contract::{HomeRows, SharedRowsAsked, UserRowsAsked},
    home_row::{HomeRowMethod, HomeRowSource},
};

use super::{
    Running,
    services::{Method, SHARED_ROWS, USER_ROWS},
};

impl Running {
    /// Calls `method` if the Plugin serves it. One it doesn't serve gives no rows, without a call.
    fn rows<'a, Req: Message>(
        &'a self,
        method: &'static Method,
        asked: &'a Req,
    ) -> BoxFuture<'a, Result<HomeRows, PluginError>> {
        Box::pin(async move {
            if !self.serves(method) {
                return Ok(HomeRows::default());
            }
            self.call_method(method, asked).await
        })
    }
}

impl HomeRowSource for Running {
    fn shared_rows<'a>(
        &'a self,
        asked: &'a SharedRowsAsked,
    ) -> BoxFuture<'a, Result<HomeRows, PluginError>> {
        self.rows(&SHARED_ROWS, asked)
    }

    fn user_rows<'a>(
        &'a self,
        asked: &'a UserRowsAsked,
    ) -> BoxFuture<'a, Result<HomeRows, PluginError>> {
        self.rows(&USER_ROWS, asked)
    }

    fn gives(&self, method: HomeRowMethod) -> bool {
        self.serves(match method {
            HomeRowMethod::Shared => &SHARED_ROWS,
            HomeRowMethod::User => &USER_ROWS,
        })
    }
}
