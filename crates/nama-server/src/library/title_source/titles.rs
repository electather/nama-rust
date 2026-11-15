//! The Kernel's title model: what a title source tells the Kernel about Movies and Shows, apart
//! from the Plugin contract's own messages, and the trust boundary between them. Every answer,
//! from a Built-in or an Installed source, crosses here, so its ids are checked once.

use std::fmt;

use nama_plugin_api::{
    contract as wire,
    metadata::{ExternalId, ExternalIds, MetadataError},
};

/// Whether a title is a Movie or a Show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum TitleKind {
    /// A Movie.
    Movie,
    /// A Show.
    Show,
}

/// What to search for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SearchQuery {
    /// Movies or Shows.
    pub(crate) kind: TitleKind,
    /// The title, as a file or folder names it.
    pub(crate) title: String,
    /// The year a Movie was released or a Show first aired, if known.
    pub(crate) year: Option<i32>,
}

/// A title a search found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Candidate {
    /// The id to fetch it by.
    pub(crate) id: ExternalId,
    /// Every id the source knows the title by, `id` among them.
    pub(crate) ids: ExternalIds,
    /// The title, in the source's language.
    pub(crate) title: String,
    /// The title in its original language.
    pub(crate) original_title: Option<String>,
    /// The year a Movie was released or a Show first aired.
    pub(crate) year: Option<i32>,
    /// The poster's path, to pass to the title source's `image`.
    pub(crate) poster: Option<String>,
    /// The ISO 3166-1 alpha-2 codes of the countries a Show comes from. Empty where unknown, as
    /// for Movies.
    pub(crate) origin_country: Vec<String>,
    /// How many people rated it, a measure of how well known it is.
    pub(crate) vote_count: u32,
}

/// A Movie's details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Movie {
    /// Its ids.
    pub(crate) ids: ExternalIds,
    /// The title, in the source's language.
    pub(crate) title: String,
    /// The title in its original language.
    pub(crate) original_title: Option<String>,
    /// A summary of the plot.
    pub(crate) overview: Option<String>,
    /// When it was released, as `YYYY-MM-DD`.
    pub(crate) release_date: Option<String>,
    /// How long it runs, in minutes.
    pub(crate) runtime_minutes: Option<u32>,
    /// The poster, as a path for the title source's `image`.
    pub(crate) poster: Option<String>,
    /// The backdrop, as a path for the title source's `image`.
    pub(crate) backdrop: Option<String>,
}

/// A Show's details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Show {
    /// Its ids.
    pub(crate) ids: ExternalIds,
    /// The title, in the source's language.
    pub(crate) title: String,
    /// The title in its original language.
    pub(crate) original_title: Option<String>,
    /// A summary.
    pub(crate) overview: Option<String>,
    /// When it first aired, as `YYYY-MM-DD`.
    pub(crate) first_air_date: Option<String>,
    /// The poster, as a path for the title source's `image`.
    pub(crate) poster: Option<String>,
    /// The backdrop, as a path for the title source's `image`.
    pub(crate) backdrop: Option<String>,
    /// The numbers of its Seasons, in order. Season 0 holds specials.
    pub(crate) seasons: Vec<u32>,
}

/// One Season of a Show, with its Episodes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Season {
    /// The Season's own ids, where the source has them.
    pub(crate) ids: ExternalIds,
    /// Its number. Season 0 holds specials.
    pub(crate) number: u32,
    /// Its name, such as `Season 1` or `Specials`.
    pub(crate) title: String,
    /// A summary.
    pub(crate) overview: Option<String>,
    /// When it first aired, as `YYYY-MM-DD`.
    pub(crate) air_date: Option<String>,
    /// The poster, as a path for the title source's `image`.
    pub(crate) poster: Option<String>,
    /// Its Episodes, in order.
    pub(crate) episodes: Vec<Episode>,
}

/// One Episode of a Show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Episode {
    /// The Episode's own ids. Within a [`Season`] only the source's own id may be known.
    pub(crate) ids: ExternalIds,
    /// The Season it is in.
    pub(crate) season: u32,
    /// Its number within the Season.
    pub(crate) number: u32,
    /// Its title.
    pub(crate) title: String,
    /// A summary.
    pub(crate) overview: Option<String>,
    /// When it first aired, as `YYYY-MM-DD`.
    pub(crate) air_date: Option<String>,
    /// How long it runs, in minutes.
    pub(crate) runtime_minutes: Option<u32>,
    /// A still from it, as a path for the title source's `image`.
    pub(crate) still: Option<String>,
}

/// An image's bytes.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Image {
    /// Its media type, such as `image/jpeg`.
    pub(crate) content_type: String,
    /// The encoded image.
    pub(crate) bytes: Vec<u8>,
}

impl fmt::Debug for Image {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Image")
            .field("content_type", &self.content_type)
            .field("len", &self.bytes.len())
            .finish()
    }
}

/// `query` as the contract asks it.
pub(super) fn search_query(query: &SearchQuery) -> wire::SearchQuery {
    let kind = match query.kind {
        TitleKind::Movie => wire::TitleKind::TITLE_KIND_MOVIE,
        TitleKind::Show => wire::TitleKind::TITLE_KIND_SHOW,
    };
    wire::SearchQuery {
        kind: kind.into(),
        title: query.title.clone(),
        year: query.year,
        ..Default::default()
    }
}

/// `id` as the contract carries it.
pub(super) fn title_id(id: &ExternalId) -> wire::TitleId {
    wire::TitleId {
        namespace: id.namespace().to_owned(),
        value: id.value().to_owned(),
        ..Default::default()
    }
}

/// `id` as the Kernel's, or why the source's answer is no good: a source's id is checked here,
/// at the trust boundary, since the Kernel stores and links by it.
pub(super) fn external_id(id: &wire::TitleId) -> Result<ExternalId, MetadataError> {
    ExternalId::new(&id.namespace, id.value.clone()).map_err(|err| {
        let named = format!("{}-{}", id.namespace, id.value);
        MetadataError::Failed(
            format!("the title source answered with the id {named:?}: {err}").into(),
        )
    })
}

fn external_ids(ids: &[wire::TitleId]) -> Result<ExternalIds, MetadataError> {
    ids.iter().map(external_id).collect()
}

pub(super) fn candidate(candidate: wire::Candidate) -> Result<Candidate, MetadataError> {
    let id = candidate.id.as_option().ok_or_else(|| {
        MetadataError::Failed(
            format!("the title source found {:?} without an id", candidate.title).into(),
        )
    })?;
    Ok(Candidate {
        id: external_id(id)?,
        ids: external_ids(&candidate.ids)?,
        title: candidate.title,
        original_title: candidate.original_title,
        year: candidate.year,
        poster: candidate.poster,
        origin_country: candidate.origin_country,
        vote_count: candidate.vote_count,
    })
}

pub(super) fn movie(movie: wire::Movie) -> Result<Movie, MetadataError> {
    Ok(Movie {
        ids: external_ids(&movie.ids)?,
        title: movie.title,
        original_title: movie.original_title,
        overview: movie.overview,
        release_date: movie.release_date,
        runtime_minutes: movie.runtime_minutes,
        poster: movie.poster,
        backdrop: movie.backdrop,
    })
}

pub(super) fn show(show: wire::Show) -> Result<Show, MetadataError> {
    Ok(Show {
        ids: external_ids(&show.ids)?,
        title: show.title,
        original_title: show.original_title,
        overview: show.overview,
        first_air_date: show.first_air_date,
        poster: show.poster,
        backdrop: show.backdrop,
        seasons: show.seasons,
    })
}

pub(super) fn season(season: wire::Season) -> Result<Season, MetadataError> {
    Ok(Season {
        ids: external_ids(&season.ids)?,
        number: season.number,
        title: season.title,
        overview: season.overview,
        air_date: season.air_date,
        poster: season.poster,
        episodes: season
            .episodes
            .into_iter()
            .map(episode)
            .collect::<Result<_, _>>()?,
    })
}

pub(super) fn episode(episode: wire::Episode) -> Result<Episode, MetadataError> {
    Ok(Episode {
        ids: external_ids(&episode.ids)?,
        season: episode.season,
        number: episode.number,
        title: episode.title,
        overview: episode.overview,
        air_date: episode.air_date,
        runtime_minutes: episode.runtime_minutes,
        still: episode.still,
    })
}

pub(super) fn image(image: wire::Image) -> Image {
    Image {
        content_type: image.content_type,
        bytes: image.bytes,
    }
}

#[cfg(test)]
#[path = "titles_tests.rs"]
mod tests;
