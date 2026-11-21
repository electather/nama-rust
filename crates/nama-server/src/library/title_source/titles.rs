//! The Kernel's title model: what a title source tells the Kernel about Movies and Shows, apart
//! from the Plugin contract's own messages, and the trust boundary between them. Every answer,
//! from a Built-in or an Installed source, crosses here, so its ids are checked once.

use std::fmt;

use serde::{Deserialize, Serialize};

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
#[derive(Debug, Clone, PartialEq)]
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
    /// Its genres' names, in the source's language.
    pub(crate) genres: Vec<String>,
    /// Its Content Rating in the source's country, such as `R`.
    pub(crate) content_rating: Option<String>,
    /// Its Score, from 0 to 10, as the source averages it.
    pub(crate) score: Option<f64>,
    /// The ISO 639-1 code of the language it was made in, such as `ja`.
    pub(crate) original_language: Option<String>,
    /// Its Similar titles, Movies, most alike first.
    pub(crate) similar: Vec<ExternalId>,
    /// Its Trailers, at most [`MAX_TRAILERS`], in the order to show them.
    pub(crate) trailers: Vec<Trailer>,
    /// Its Cast and Crew.
    pub(crate) roles: Roles,
}

/// A Show's details.
#[derive(Debug, Clone, PartialEq)]
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
    /// As [`Movie::genres`].
    pub(crate) genres: Vec<String>,
    /// As [`Movie::content_rating`], such as `TV-MA`.
    pub(crate) content_rating: Option<String>,
    /// As [`Movie::score`].
    pub(crate) score: Option<f64>,
    /// As [`Movie::original_language`].
    pub(crate) original_language: Option<String>,
    /// As [`Movie::similar`], Shows.
    pub(crate) similar: Vec<ExternalId>,
    /// As [`Movie::trailers`].
    pub(crate) trailers: Vec<Trailer>,
    /// Its Roles over its whole run.
    pub(crate) roles: Roles,
}

/// A Trailer, as a link to where it is hosted: Nama never embeds or plays one (`GLOSSARY.md`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Trailer {
    /// What it is called, such as `Official Trailer`.
    pub(crate) name: String,
    /// Its `https` URL on `YouTube` or Vimeo.
    pub(crate) url: String,
}

/// How many Trailers a title keeps, whatever its source sends.
const MAX_TRAILERS: usize = 5;

/// A title's Roles, each list in billing order (ADR 0081).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Roles {
    pub(crate) cast: Vec<Role>,
    pub(crate) crew: Vec<Role>,
}

/// A Person's part in a title: a character they play (Cast) or a job they did (Crew).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Role {
    /// The Person's External ID.
    pub(crate) person: ExternalId,
    /// The Person's name.
    pub(crate) name: String,
    /// The character, or the job in English, such as `Director`; empty if the source doesn't say.
    pub(crate) part: String,
    /// The headshot's path, to pass to the title source's `image`. Only the first [`HEADSHOTS`]
    /// of the Cast keep one.
    pub(crate) headshot: Option<String>,
}

/// How many of the Cast, and of the Crew, the Kernel keeps of the hundreds a source may list.
const MAX_ROLES: usize = 50;

/// How many of the Cast, billed first, keep a headshot: those a title page shows.
const HEADSHOTS: usize = 15;

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

/// What a piece of a title's artwork is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArtworkKind {
    /// A poster, portrait.
    Poster,
    /// A backdrop, landscape and full screen.
    Backdrop,
    /// The Title Logo.
    Logo,
    /// Any kind the Kernel doesn't show yet, such as clear art, a banner, a thumb or disc art.
    Other,
}

/// One image of a title's artwork list.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Artwork {
    /// What kind of artwork it is.
    pub(crate) kind: ArtworkKind,
    /// Its path, for the title source's `image`.
    pub(crate) path: String,
    /// The ISO 639-1 code of the language of its text, `None` for an image without text.
    pub(crate) language: Option<String>,
    /// The source's own score for it, higher being better; always finite.
    pub(crate) rating: Option<f64>,
    /// How many people rated it.
    pub(crate) votes: Option<u32>,
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
    wire::SearchQuery {
        kind: title_kind(query.kind).into(),
        title: query.title.clone(),
        year: query.year,
        ..Default::default()
    }
}

/// `kind` as the contract carries it.
pub(super) const fn title_kind(kind: TitleKind) -> wire::TitleKind {
    match kind {
        TitleKind::Movie => wire::TitleKind::TITLE_KIND_MOVIE,
        TitleKind::Show => wire::TitleKind::TITLE_KIND_SHOW,
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
        genres: movie.genres,
        content_rating: movie.content_rating,
        score: valid_score(movie.score),
        original_language: movie.original_language,
        similar: similar(&movie.similar),
        trailers: trailers(movie.trailers),
        roles: roles(movie.cast, movie.crew),
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
        genres: show.genres,
        content_rating: show.content_rating,
        score: valid_score(show.score),
        original_language: show.original_language,
        similar: similar(&show.similar),
        trailers: trailers(show.trailers),
        roles: roles(show.cast, show.crew),
    })
}

/// `score`, unless it's outside 0 to 10, where the Kernel stores none rather than failing the
/// Match's write on it.
fn valid_score(score: Option<f64>) -> Option<f64> {
    score.filter(|score| (0.0..=10.0).contains(score))
}

/// The most Similar titles the Kernel keeps of one title, about a row of posters.
const MAX_SIMILAR: usize = 20;

/// The first [`MAX_SIMILAR`] well-formed ids of `given`. A malformed one is dropped rather than
/// failing the title, which is whole without them.
fn similar(given: &[wire::TitleId]) -> Vec<ExternalId> {
    (given.iter())
        .filter_map(|id| ExternalId::new(&id.namespace, id.value.clone()).ok())
        .take(MAX_SIMILAR)
        .collect()
}

/// The Roles the Kernel keeps of `cast` and `crew`: at most [`MAX_ROLES`] of each, the Crew
/// without headshots.
fn roles(cast: Vec<wire::Role>, crew: Vec<wire::Role>) -> Roles {
    let kept = |roles: Vec<wire::Role>| roles.into_iter().filter_map(role).take(MAX_ROLES);
    Roles {
        cast: (kept(cast).enumerate())
            .map(|(billed, role)| Role {
                headshot: role.headshot.filter(|_| billed < HEADSHOTS),
                ..role
            })
            .collect(),
        crew: kept(crew)
            .map(|role| Role {
                headshot: None,
                ..role
            })
            .collect(),
    }
}

/// `role`, unless its Person has no valid id: one bad Role is dropped rather than failing the
/// title.
fn role(role: wire::Role) -> Option<Role> {
    Some(Role {
        person: external_id(role.person.as_option()?).ok()?,
        name: role.name,
        part: role.part,
        headshot: role.headshot,
    })
}

/// The first [`MAX_TRAILERS`] of `trailers` the Kernel can link to.
fn trailers(trailers: Vec<wire::Trailer>) -> Vec<Trailer> {
    (trailers.into_iter().filter_map(trailer))
        .take(MAX_TRAILERS)
        .collect()
}

/// `trailer` as a link to its site, unless the Kernel doesn't link to that site or its key could
/// make the link point anywhere else.
fn trailer(trailer: wire::Trailer) -> Option<Trailer> {
    let key = trailer.key;
    let safe = !key.is_empty()
        && (key.bytes()).all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_');
    let url = match trailer.site.as_str() {
        "YouTube" if safe => format!("https://www.youtube.com/watch?v={key}"),
        "Vimeo" if safe => format!("https://vimeo.com/{key}"),
        _ => return None,
    };
    let name = if trailer.name.trim().is_empty() {
        "Trailer".to_owned()
    } else {
        trailer.name
    };
    Some(Trailer { name, url })
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

/// The Kernel's twins of an artwork list. One malformed item, with no path or kind, is dropped
/// alone, so it never costs the source the rest of its list.
pub(super) fn artwork(list: Vec<wire::Artwork>) -> Vec<Artwork> {
    use wire::ArtworkKind as Wire;
    let item = |art: wire::Artwork| {
        let kind = match art.kind.as_known()? {
            Wire::ARTWORK_KIND_POSTER => ArtworkKind::Poster,
            Wire::ARTWORK_KIND_BACKDROP => ArtworkKind::Backdrop,
            Wire::ARTWORK_KIND_LOGO => ArtworkKind::Logo,
            Wire::ARTWORK_KIND_CLEAR_ART
            | Wire::ARTWORK_KIND_BANNER
            | Wire::ARTWORK_KIND_THUMB
            | Wire::ARTWORK_KIND_DISC => ArtworkKind::Other,
            Wire::ARTWORK_KIND_UNSPECIFIED => return None,
        };
        if art.path.is_empty() {
            return None;
        }
        Some(Artwork {
            kind,
            path: art.path,
            language: art.language,
            // A NaN would outrank every rating when ranked.
            rating: art.rating.filter(|rating| rating.is_finite()),
            votes: art.votes,
        })
    };
    list.into_iter().filter_map(item).collect()
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
