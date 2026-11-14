//! An Installed Plugin as a title source: the Kernel's `TitleSource` calls passed through as the
//! contract's `TitleSourceService` rpcs. The Library's title source checks the answers.

use buffa::Message;
use nama_plugin_api::{
    BoxFuture,
    contract::{
        EpisodeAnswer, EpisodeAsked, ImageAnswer, ImageAsked, MovieAnswer, SearchQuery,
        SearchResults, SeasonAnswer, SeasonAsked, ShowAnswer, TitleAsked,
    },
    metadata::{MetadataError, TitleSource},
};

use super::{
    Running,
    services::{
        Method, TITLE_EPISODE, TITLE_IMAGE, TITLE_MOVIE, TITLE_SEARCH, TITLE_SEASON, TITLE_SHOW,
    },
};

impl Running {
    /// Calls `method` if the Plugin serves it, its failure as [`MetadataError::Failed`]. One it
    /// doesn't serve answers [`MetadataError::Unsupported`] without a call.
    fn ask<'a, Req: Message, Resp: Message + 'a>(
        &'a self,
        method: &'static Method,
        request: &'a Req,
    ) -> BoxFuture<'a, Result<Resp, MetadataError>> {
        Box::pin(async move {
            if !self.serves(method) {
                return Err(MetadataError::Unsupported);
            }
            self.call_method(method, request)
                .await
                .map_err(MetadataError::Failed)
        })
    }
}

impl TitleSource for Running {
    fn namespace(&self) -> &str {
        &self.package.title_namespace
    }

    fn search<'a>(
        &'a self,
        query: &'a SearchQuery,
    ) -> BoxFuture<'a, Result<SearchResults, MetadataError>> {
        self.ask(&TITLE_SEARCH, query)
    }

    fn movie<'a>(
        &'a self,
        asked: &'a TitleAsked,
    ) -> BoxFuture<'a, Result<MovieAnswer, MetadataError>> {
        self.ask(&TITLE_MOVIE, asked)
    }

    fn show<'a>(
        &'a self,
        asked: &'a TitleAsked,
    ) -> BoxFuture<'a, Result<ShowAnswer, MetadataError>> {
        self.ask(&TITLE_SHOW, asked)
    }

    fn season<'a>(
        &'a self,
        asked: &'a SeasonAsked,
    ) -> BoxFuture<'a, Result<SeasonAnswer, MetadataError>> {
        self.ask(&TITLE_SEASON, asked)
    }

    fn episode<'a>(
        &'a self,
        asked: &'a EpisodeAsked,
    ) -> BoxFuture<'a, Result<EpisodeAnswer, MetadataError>> {
        self.ask(&TITLE_EPISODE, asked)
    }

    fn image<'a>(
        &'a self,
        asked: &'a ImageAsked,
    ) -> BoxFuture<'a, Result<ImageAnswer, MetadataError>> {
        self.ask(&TITLE_IMAGE, asked)
    }
}

#[cfg(test)]
#[path = "title_source_tests.rs"]
mod tests;
