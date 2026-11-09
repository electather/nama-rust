//! Provide: metadata about Movies and Shows, which the Kernel uses to Match the files in a
//! Library to titles.
//!
//! A Plugin provides it by registering an `Arc<dyn MetadataProvider>`:
//!
//! ```ignore
//! registrar.provide::<dyn MetadataProvider>(Priority::DEFAULT, Arc::new(my_provider));
//! ```

use std::fmt;

use crate::{BoxFuture, PluginError};

/// A Provider of metadata about Movies and Shows.
pub trait MetadataProvider: Send + Sync + 'static {
    /// The titles of `query.kind` that best fit `query`, best first.
    fn search<'a>(
        &'a self,
        query: &'a SearchQuery,
    ) -> BoxFuture<'a, Result<Vec<Candidate>, MetadataError>>;

    /// The Movie with this id.
    fn movie<'a>(&'a self, id: &'a ExternalId) -> BoxFuture<'a, Result<Movie, MetadataError>>;

    /// The Show with this id.
    fn show<'a>(&'a self, id: &'a ExternalId) -> BoxFuture<'a, Result<Show, MetadataError>>;

    /// One Season of the Show with this id, with its Episodes. Season 0 holds specials.
    fn season<'a>(
        &'a self,
        show: &'a ExternalId,
        season: u32,
    ) -> BoxFuture<'a, Result<Season, MetadataError>>;

    /// One Episode of the Show with this id.
    fn episode<'a>(
        &'a self,
        show: &'a ExternalId,
        season: u32,
        episode: u32,
    ) -> BoxFuture<'a, Result<Episode, MetadataError>>;

    /// The image at `path`, as this Provider gave it in a poster, backdrop or still.
    fn image<'a>(&'a self, path: &'a str) -> BoxFuture<'a, Result<Image, MetadataError>>;
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

/// An id of a title in a metadata source.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ExternalId {
    /// A TMDB id. Movies and Shows have separate id spaces.
    Tmdb(i64),
    /// An IMDb id, such as `tt0078748`.
    Imdb(String),
    /// A TVDB id.
    Tvdb(i64),
}

impl fmt::Display for ExternalId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tmdb(id) => write!(f, "tmdb-{id}"),
            Self::Imdb(id) => write!(f, "imdb-{id}"),
            Self::Tvdb(id) => write!(f, "tvdb-{id}"),
        }
    }
}

/// Every id a title has, in the metadata sources a Provider knows about. Watch State syncs by
/// these (ADR 0008).
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct ExternalIds {
    /// The TMDB id.
    pub tmdb: Option<i64>,
    /// The IMDb id, such as `tt0078748`.
    pub imdb: Option<String>,
    /// The TVDB id.
    pub tvdb: Option<i64>,
}

/// A title a search found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// The id to fetch it by.
    pub id: ExternalId,
    /// The title, in the Provider's language.
    pub title: String,
    /// The title in its original language.
    pub original_title: Option<String>,
    /// The year a Movie was released or a Show first aired.
    pub year: Option<i32>,
    /// The poster's path, to pass to [`MetadataProvider::image`].
    pub poster: Option<String>,
    /// The ISO 3166-1 alpha-2 codes of the countries a Show comes from. Empty where unknown, as
    /// for Movies.
    pub origin_country: Vec<String>,
}

/// A Movie's details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Movie {
    /// Its ids.
    pub ids: ExternalIds,
    /// The title, in the Provider's language.
    pub title: String,
    /// The title in its original language.
    pub original_title: Option<String>,
    /// A summary of the plot.
    pub overview: Option<String>,
    /// When it was released, as `YYYY-MM-DD`.
    pub release_date: Option<String>,
    /// How long it runs, in minutes.
    pub runtime_minutes: Option<u32>,
    /// The poster, as a path for [`MetadataProvider::image`].
    pub poster: Option<String>,
    /// The backdrop, as a path for [`MetadataProvider::image`].
    pub backdrop: Option<String>,
}

/// A Show's details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Show {
    /// Its ids.
    pub ids: ExternalIds,
    /// The title, in the Provider's language.
    pub title: String,
    /// The title in its original language.
    pub original_title: Option<String>,
    /// A summary.
    pub overview: Option<String>,
    /// When it first aired, as `YYYY-MM-DD`.
    pub first_air_date: Option<String>,
    /// The poster, as a path for [`MetadataProvider::image`].
    pub poster: Option<String>,
    /// The backdrop, as a path for [`MetadataProvider::image`].
    pub backdrop: Option<String>,
    /// The numbers of its Seasons, in order. Season 0 holds specials.
    pub seasons: Vec<u32>,
}

/// One Season of a Show, with its Episodes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Season {
    /// The Season's own ids, where the Provider has them.
    pub ids: ExternalIds,
    /// Its number. Season 0 holds specials.
    pub number: u32,
    /// Its name, such as `Season 1` or `Specials`.
    pub title: String,
    /// A summary.
    pub overview: Option<String>,
    /// When it first aired, as `YYYY-MM-DD`.
    pub air_date: Option<String>,
    /// The poster, as a path for [`MetadataProvider::image`].
    pub poster: Option<String>,
    /// Its Episodes, in order.
    pub episodes: Vec<Episode>,
}

/// One Episode of a Show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Episode {
    /// The Episode's own ids. Within a [`Season`] only the Provider's own id may be known.
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
    /// A still from it, as a path for [`MetadataProvider::image`].
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

/// Why a [`MetadataProvider`] couldn't answer.
#[derive(Debug)]
pub enum MetadataError {
    /// There is no such title, Season, Episode or image.
    NotFound,
    /// The Provider can't look titles up by this kind of id.
    UnsupportedId(ExternalId),
    /// Anything else, such as the metadata source being unreachable. Worth trying again later.
    Failed(PluginError),
}

impl fmt::Display for MetadataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("not found"),
            Self::UnsupportedId(id) => write!(f, "can't look up {id}"),
            Self::Failed(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for MetadataError {}
