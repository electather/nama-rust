//! An Installed Plugin as Watch sync: the Kernel's `WatchSync` calls passed through as the
//! contract's `WatchSyncService` rpcs.

use nama_plugin_api::{
    BoxFuture, PluginError,
    contract::{ImportAsked, Imported, ScrobbleAsked, Scrobbled},
    watch_sync::WatchSync,
};

use super::{
    Running,
    services::{IMPORT, SCROBBLE},
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
}
