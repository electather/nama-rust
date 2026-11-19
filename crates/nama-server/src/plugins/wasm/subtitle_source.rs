//! An Installed Plugin as a subtitle source: the Kernel's `SubtitleSource` calls passed through as
//! the contract's `SubtitleSourceService` rpcs.

use nama_plugin_api::{
    BoxFuture, PluginError,
    contract::{SubtitleDownload, SubtitleFile, SubtitleResults, SubtitleSearch},
    subtitle_source::SubtitleSource,
};

use super::{
    Running,
    services::{SUBTITLE_DOWNLOAD, SUBTITLE_SEARCH},
};

impl SubtitleSource for Running {
    fn search<'a>(
        &'a self,
        search: &'a SubtitleSearch,
    ) -> BoxFuture<'a, Result<SubtitleResults, PluginError>> {
        Box::pin(self.call_method(&SUBTITLE_SEARCH, search))
    }

    fn download<'a>(
        &'a self,
        download: &'a SubtitleDownload,
    ) -> BoxFuture<'a, Result<SubtitleFile, PluginError>> {
        Box::pin(self.call_method(&SUBTITLE_DOWNLOAD, download))
    }
}
