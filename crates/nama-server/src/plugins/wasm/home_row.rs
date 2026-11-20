//! An Installed Plugin as a Home row source: the Kernel's `HomeRowSource` calls passed through as
//! the contract's `HomeRowService` rpcs.

use buffa::Message;
use nama_plugin_api::{
    BoxFuture, PluginError,
    contract::{FeaturedSuggestions, HomeRows, SharedRowsAsked, UserFeaturedAsked, UserRowsAsked},
    home_row::{HomeRowMethod, HomeRowSource},
};

use super::{
    Running,
    services::{Method, SHARED_ROWS, USER_FEATURED, USER_ROWS},
};

impl Running {
    /// Calls `method` if the Plugin serves it. One it doesn't serve gives nothing, without a call.
    fn answer<'a, Req: Message, Answer: Message + Default + 'a>(
        &'a self,
        method: &'static Method,
        asked: &'a Req,
    ) -> BoxFuture<'a, Result<Answer, PluginError>> {
        Box::pin(async move {
            if !self.serves(method) {
                return Ok(Answer::default());
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
        self.answer(&SHARED_ROWS, asked)
    }

    fn user_rows<'a>(
        &'a self,
        asked: &'a UserRowsAsked,
    ) -> BoxFuture<'a, Result<HomeRows, PluginError>> {
        self.answer(&USER_ROWS, asked)
    }

    fn user_featured<'a>(
        &'a self,
        asked: &'a UserFeaturedAsked,
    ) -> BoxFuture<'a, Result<FeaturedSuggestions, PluginError>> {
        self.answer(&USER_FEATURED, asked)
    }

    fn gives(&self, method: HomeRowMethod) -> bool {
        self.serves(match method {
            HomeRowMethod::Shared => &SHARED_ROWS,
            HomeRowMethod::User => &USER_ROWS,
            HomeRowMethod::Featured => &USER_FEATURED,
        })
    }
}
