//! Provide: a title source, which tells the Kernel about Movies and Shows so it can Match the
//! files in a Library to titles.
//!
//! A Plugin serves it by registering an `Arc<dyn TitleSource>`:
//!
//! ```ignore
//! registrar.provide::<dyn TitleSource>(Priority::DEFAULT, Arc::new(my_source));
//! ```

use std::fmt;

pub use crate::ids::{ExternalId, ExternalIds, IMDB, InvalidId, TMDB, TVDB};
use crate::{
    BoxFuture, PluginError,
    contract::{
        ArtworkAnswer, ArtworkAsked, EpisodeAnswer, EpisodeAsked, ImageAnswer, ImageAsked,
        MovieAnswer, SearchQuery, SearchResults, SeasonAnswer, SeasonAsked, ShowAnswer, TitleAsked,
    },
};

/// A title source: where the Kernel finds and fetches Movies and Shows. It speaks the contract's
/// `TitleSourceService` messages, as an Installed Plugin's exports do. Titles carry ids as open
/// namespace and value pairs, of which `tmdb`, `imdb` and `tvdb` are well known. Asked by an id in
/// a namespace it can't look titles up by, a source answers with `unsupported_id` set, not an
/// error, and the Kernel asks the next source. An answer with its title left unset means the
/// source has no such title.
///
/// Every method but [`namespace`](Self::namespace) and
/// [`lookup_namespaces`](Self::lookup_namespaces) is optional: one left out answers
/// [`MetadataError::Unsupported`], so one point serves sources that answer only some of them.
pub trait TitleSource: Send + Sync + 'static {
    /// The namespace this source keeps titles under, such as `tmdb`: one or more of a-z and 0-9.
    fn namespace(&self) -> &str;

    /// The namespaces besides [`namespace`](Self::namespace) it looks titles up by, such as
    /// `imdb`.
    fn lookup_namespaces(&self) -> Vec<String> {
        Vec::new()
    }

    /// The titles of `query.kind` that best fit `query`, best first.
    fn search<'a>(
        &'a self,
        _query: &'a SearchQuery,
    ) -> BoxFuture<'a, Result<SearchResults, MetadataError>> {
        unsupported()
    }

    /// The Movie with the id asked for.
    fn movie<'a>(
        &'a self,
        _asked: &'a TitleAsked,
    ) -> BoxFuture<'a, Result<MovieAnswer, MetadataError>> {
        unsupported()
    }

    /// The Show with the id asked for.
    fn show<'a>(
        &'a self,
        _asked: &'a TitleAsked,
    ) -> BoxFuture<'a, Result<ShowAnswer, MetadataError>> {
        unsupported()
    }

    /// One Season of the Show with the id asked for, with its Episodes. Season 0 holds specials.
    fn season<'a>(
        &'a self,
        _asked: &'a SeasonAsked,
    ) -> BoxFuture<'a, Result<SeasonAnswer, MetadataError>> {
        unsupported()
    }

    /// One Episode of the Show with the id asked for.
    fn episode<'a>(
        &'a self,
        _asked: &'a EpisodeAsked,
    ) -> BoxFuture<'a, Result<EpisodeAnswer, MetadataError>> {
        unsupported()
    }

    /// The image at the path asked for, as this source gave it in a poster, backdrop, still or
    /// artwork.
    ///
    /// A path names bytes that never change, as TMDB's do: the Server keeps an image under its
    /// path and browsers cache it for good, so a changed image needs a new path.
    fn image<'a>(
        &'a self,
        _asked: &'a ImageAsked,
    ) -> BoxFuture<'a, Result<ImageAnswer, MetadataError>> {
        unsupported()
    }

    /// The artwork this source has for the title asked for, of every kind it has.
    fn artwork<'a>(
        &'a self,
        _asked: &'a ArtworkAsked,
    ) -> BoxFuture<'a, Result<ArtworkAnswer, MetadataError>> {
        unsupported()
    }

    /// The artwork this source has for the Season asked for, of every kind it has.
    fn season_artwork<'a>(
        &'a self,
        _asked: &'a SeasonAsked,
    ) -> BoxFuture<'a, Result<ArtworkAnswer, MetadataError>> {
        unsupported()
    }

    /// Whether this source serves `method`. The Kernel asks a source only for the methods it
    /// answers; a Built-in that leaves a method out may keep the default, which costs one
    /// [`MetadataError::Unsupported`] answer.
    fn answers(&self, _method: TitleMethod) -> bool {
        true
    }
}

/// What a Plugin's [`Manifest`](crate::Manifest) says of the title source it provides, so the
/// Kernel can judge a Library's title source order while the Plugin isn't enabled. It must agree
/// with the [`TitleSource`] the Plugin registers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceManifest {
    /// The namespace it keeps titles under, as [`TitleSource::namespace`] gives it.
    pub namespace: String,
    /// The namespaces it looks titles up by, as [`TitleSource::lookup_namespaces`] gives them.
    pub looks_up: Vec<String>,
    /// Whether it answers [`TitleMethod::Search`].
    pub searches: bool,
}

/// A [`TitleSource`] method the Kernel may ask, for [`TitleSource::answers`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitleMethod {
    /// [`TitleSource::search`].
    Search,
    /// [`TitleSource::movie`].
    Movie,
    /// [`TitleSource::show`].
    Show,
    /// [`TitleSource::season`].
    Season,
    /// [`TitleSource::episode`].
    Episode,
    /// [`TitleSource::image`].
    Image,
    /// [`TitleSource::artwork`].
    Artwork,
    /// [`TitleSource::season_artwork`].
    SeasonArtwork,
}

/// The answer of a [`TitleSource`] method its source left out.
fn unsupported<'a, T: Send + 'a>() -> BoxFuture<'a, Result<T, MetadataError>> {
    Box::pin(std::future::ready(Err(MetadataError::Unsupported)))
}

/// Why a [`TitleSource`] couldn't answer.
#[derive(Debug)]
pub enum MetadataError {
    /// This title source doesn't serve that method.
    Unsupported,
    /// Anything else, such as the metadata source being unreachable. Worth trying again later.
    Failed(PluginError),
}

impl fmt::Display for MetadataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported => f.write_str("this title source doesn't serve that"),
            Self::Failed(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for MetadataError {}

#[cfg(test)]
#[path = "metadata_tests.rs"]
mod tests;
