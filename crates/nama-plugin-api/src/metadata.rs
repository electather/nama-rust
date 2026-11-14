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
use crate::{BoxFuture, PluginError};

/// A title source: where the Kernel finds and fetches Movies and Shows. Titles carry ids as open
/// namespace and value pairs, of which `tmdb`, `imdb` and `tvdb` are well known; a source refuses
/// an id in a namespace it doesn't know with [`MetadataError::UnsupportedId`].
///
/// Every method but [`namespace`](Self::namespace) is optional: one left out answers
/// [`MetadataError::Unsupported`], so one point serves sources that answer only some of them.
pub trait TitleSource: Send + Sync + 'static {
    /// The namespace this source keeps titles under, such as `tmdb`: one or more of a-z and 0-9.
    fn namespace(&self) -> &str;

    /// The titles of `query.kind` that best fit `query`, best first.
    fn search<'a>(
        &'a self,
        _query: &'a SearchQuery,
    ) -> BoxFuture<'a, Result<Vec<Candidate>, MetadataError>> {
        unsupported()
    }

    /// The Movie with this id.
    fn movie<'a>(&'a self, _id: &'a ExternalId) -> BoxFuture<'a, Result<Movie, MetadataError>> {
        unsupported()
    }

    /// The Show with this id.
    fn show<'a>(&'a self, _id: &'a ExternalId) -> BoxFuture<'a, Result<Show, MetadataError>> {
        unsupported()
    }

    /// One Season of the Show with this id, with its Episodes. Season 0 holds specials.
    fn season<'a>(
        &'a self,
        _show: &'a ExternalId,
        _season: u32,
    ) -> BoxFuture<'a, Result<Season, MetadataError>> {
        unsupported()
    }

    /// One Episode of the Show with this id.
    fn episode<'a>(
        &'a self,
        _show: &'a ExternalId,
        _season: u32,
        _episode: u32,
    ) -> BoxFuture<'a, Result<Episode, MetadataError>> {
        unsupported()
    }

    /// The image at `path`, as this source gave it in a poster, backdrop or still.
    ///
    /// A path names bytes that never change, as TMDB's do: the Server keeps an image under its
    /// path and browsers cache it for good, so a changed image needs a new path.
    fn image<'a>(&'a self, _path: &'a str) -> BoxFuture<'a, Result<Image, MetadataError>> {
        unsupported()
    }
}

/// The answer of a [`TitleSource`] method its source left out.
fn unsupported<'a, T: Send + 'a>() -> BoxFuture<'a, Result<T, MetadataError>> {
    Box::pin(std::future::ready(Err(MetadataError::Unsupported)))
}

/// Whether a title is a Movie or a Show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TitleKind {
    /// A Movie.
    Movie,
    /// A Show.
    Show,
}

/// What to search for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchQuery {
    /// Movies or Shows.
    pub kind: TitleKind,
    /// The title, as a file or folder names it.
    pub title: String,
    /// The year a Movie was released or a Show first aired, if known.
    pub year: Option<i32>,
}

/// A title a search found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// The id to fetch it by.
    pub id: ExternalId,
    /// Every id the source knows the title by, `id` among them.
    pub ids: ExternalIds,
    /// The title, in the source's language.
    pub title: String,
    /// The title in its original language.
    pub original_title: Option<String>,
    /// The year a Movie was released or a Show first aired.
    pub year: Option<i32>,
    /// The poster's path, to pass to [`TitleSource::image`].
    pub poster: Option<String>,
    /// The ISO 3166-1 alpha-2 codes of the countries a Show comes from. Empty where unknown, as
    /// for Movies.
    pub origin_country: Vec<String>,
    /// How many people rated it, a measure of how well known it is.
    pub vote_count: u32,
}

/// A Movie's details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Movie {
    /// Its ids.
    pub ids: ExternalIds,
    /// The title, in the source's language.
    pub title: String,
    /// The title in its original language.
    pub original_title: Option<String>,
    /// A summary of the plot.
    pub overview: Option<String>,
    /// When it was released, as `YYYY-MM-DD`.
    pub release_date: Option<String>,
    /// How long it runs, in minutes.
    pub runtime_minutes: Option<u32>,
    /// The poster, as a path for [`TitleSource::image`].
    pub poster: Option<String>,
    /// The backdrop, as a path for [`TitleSource::image`].
    pub backdrop: Option<String>,
}

/// A Show's details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Show {
    /// Its ids.
    pub ids: ExternalIds,
    /// The title, in the source's language.
    pub title: String,
    /// The title in its original language.
    pub original_title: Option<String>,
    /// A summary.
    pub overview: Option<String>,
    /// When it first aired, as `YYYY-MM-DD`.
    pub first_air_date: Option<String>,
    /// The poster, as a path for [`TitleSource::image`].
    pub poster: Option<String>,
    /// The backdrop, as a path for [`TitleSource::image`].
    pub backdrop: Option<String>,
    /// The numbers of its Seasons, in order. Season 0 holds specials.
    pub seasons: Vec<u32>,
}

/// One Season of a Show, with its Episodes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Season {
    /// The Season's own ids, where the source has them.
    pub ids: ExternalIds,
    /// Its number. Season 0 holds specials.
    pub number: u32,
    /// Its name, such as `Season 1` or `Specials`.
    pub title: String,
    /// A summary.
    pub overview: Option<String>,
    /// When it first aired, as `YYYY-MM-DD`.
    pub air_date: Option<String>,
    /// The poster, as a path for [`TitleSource::image`].
    pub poster: Option<String>,
    /// Its Episodes, in order.
    pub episodes: Vec<Episode>,
}

/// One Episode of a Show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Episode {
    /// The Episode's own ids. Within a [`Season`] only the source's own id may be known.
    pub ids: ExternalIds,
    /// The Season it is in.
    pub season: u32,
    /// Its number within the Season.
    pub number: u32,
    /// Its title.
    pub title: String,
    /// A summary.
    pub overview: Option<String>,
    /// When it first aired, as `YYYY-MM-DD`.
    pub air_date: Option<String>,
    /// How long it runs, in minutes.
    pub runtime_minutes: Option<u32>,
    /// A still from it, as a path for [`TitleSource::image`].
    pub still: Option<String>,
}

/// An image's bytes.
#[derive(Clone, PartialEq, Eq)]
pub struct Image {
    /// Its media type, such as `image/jpeg`.
    pub content_type: String,
    /// The encoded image.
    pub bytes: Vec<u8>,
}

impl fmt::Debug for Image {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Image")
            .field("content_type", &self.content_type)
            .field("len", &self.bytes.len())
            .finish()
    }
}

/// Why a [`TitleSource`] couldn't answer.
#[derive(Debug)]
pub enum MetadataError {
    /// There is no such title, Season, Episode or image.
    NotFound,
    /// This title source doesn't serve that method.
    Unsupported,
    /// The source can't look titles up by this kind of id.
    UnsupportedId(ExternalId),
    /// Anything else, such as the metadata source being unreachable. Worth trying again later.
    Failed(PluginError),
}

impl fmt::Display for MetadataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("not found"),
            Self::Unsupported => f.write_str("this title source doesn't serve that"),
            Self::UnsupportedId(id) => write!(f, "can't look up {id}"),
            Self::Failed(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for MetadataError {}

#[cfg(test)]
#[path = "metadata_tests.rs"]
mod tests;
