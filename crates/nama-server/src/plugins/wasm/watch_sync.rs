//! An Installed Plugin as Watch sync: the Kernel's `WatchSync` calls passed through as the
//! contract's `WatchSyncService` rpcs.

use nama_plugin_api::{
    BoxFuture, PluginError,
    contract::{
        ImportAsked, Imported, LinkPolled, LinkStarted, PollLinkAsked, ScrobbleAsked, Scrobbled,
        StartLinkAsked,
    },
    watch_sync::WatchSync,
};

use super::{
    Running,
    services::{IMPORT, POLL_LINK, SCROBBLE, START_LINK},
};

impl WatchSync for Running {
    fn scrobble<'a>(
        &'a self,
        asked: &'a ScrobbleAsked,
    ) -> BoxFuture<'a, Result<Scrobbled, PluginError>> {
        Box::pin(self.call_method(&SCROBBLE, asked))
    }

    fn import<'a>(
        &'a self,
        asked: &'a ImportAsked,
    ) -> BoxFuture<'a, Result<Imported, PluginError>> {
        Box::pin(self.call_method(&IMPORT, asked))
    }

    fn start_link<'a>(
        &'a self,
        asked: &'a StartLinkAsked,
    ) -> BoxFuture<'a, Result<LinkStarted, PluginError>> {
        Box::pin(self.call_method(&START_LINK, asked))
    }

    fn poll_link<'a>(
        &'a self,
        asked: &'a PollLinkAsked,
    ) -> BoxFuture<'a, Result<LinkPolled, PluginError>> {
        Box::pin(self.call_method(&POLL_LINK, asked))
    }
}
