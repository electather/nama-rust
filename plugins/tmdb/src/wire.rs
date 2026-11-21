//! TMDB's v3 JSON, as far as the Plugin reads it, and how it maps onto the contract's messages.

use nama_plugin_api::{
    contract::{Artwork, ArtworkKind, Candidate, Episode, Movie, Season, Show, TitleId},
    metadata::{ExternalId, IMDB},
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(crate) struct Page<T> {
    pub(crate) results: Vec<T>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct MovieResult {
    id: i64,
    title: String,
    original_title: Option<String>,
    release_date: Option<String>,
    poster_path: Option<String>,
    #[serde(default)]
    vote_count: u32,
}

impl From<MovieResult> for Candidate {
    fn from(movie: MovieResult) -> Self {
        let id = title_id(&ExternalId::tmdb(movie.id));
        Self {
            ids: vec![id.clone()],
            id: id.into(),
            year: year(movie.release_date.as_deref()),
            title: movie.title,
            original_title: present(movie.original_title),
            poster: present(movie.poster_path),
            origin_country: Vec::new(),
            vote_count: movie.vote_count,
            ..Default::default()
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct ShowResult {
    id: i64,
    name: String,
    original_name: Option<String>,
    first_air_date: Option<String>,
    poster_path: Option<String>,
    #[serde(default)]
    origin_country: Vec<String>,
    #[serde(default)]
    vote_count: u32,
}

impl From<ShowResult> for Candidate {
    fn from(show: ShowResult) -> Self {
        let id = title_id(&ExternalId::tmdb(show.id));
        Self {
            ids: vec![id.clone()],
            id: id.into(),
            year: year(show.first_air_date.as_deref()),
            title: show.name,
            original_title: present(show.original_name),
            poster: present(show.poster_path),
            origin_country: show.origin_country,
            vote_count: show.vote_count,
            ..Default::default()
        }
    }
}

/// `/find/{id}`: the titles another source's id names, by kind.
#[derive(Debug, Deserialize)]
pub(crate) struct Found {
    #[serde(default)]
    pub(crate) movie_results: Vec<FoundTitle>,
    #[serde(default)]
    pub(crate) tv_results: Vec<FoundTitle>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct FoundTitle {
    pub(crate) id: i64,
}

/// `external_ids`, appended to a details response.
#[derive(Debug, Default, Deserialize)]
pub(crate) struct WireIds {
    imdb_id: Option<String>,
    tvdb_id: Option<i64>,
}

impl WireIds {
    fn with_tmdb(self, tmdb: i64) -> Vec<TitleId> {
        let imdb = present(self.imdb_id).and_then(|imdb| ExternalId::new(IMDB, imdb).ok());
        [
            Some(ExternalId::tmdb(tmdb)),
            imdb,
            self.tvdb_id.map(ExternalId::tvdb),
        ]
        .iter()
        .flatten()
        .map(title_id)
        .collect()
    }
}

/// `/movie/{id}?append_to_response=external_ids,release_dates`.
#[derive(Debug, Deserialize)]
pub(crate) struct MovieDetails {
    id: i64,
    title: String,
    original_title: Option<String>,
    overview: Option<String>,
    release_date: Option<String>,
    runtime: Option<u32>,
    poster_path: Option<String>,
    backdrop_path: Option<String>,
    imdb_id: Option<String>,
    original_language: Option<String>,
    #[serde(flatten)]
    votes: Votes,
    #[serde(default)]
    external_ids: WireIds,
    #[serde(default)]
    genres: Vec<Genre>,
    #[serde(default)]
    release_dates: ReleaseDates,
}

impl MovieDetails {
    /// The Movie, with its Content Rating in `country`.
    pub(crate) fn into_movie(self, country: Option<&str>) -> Movie {
        let mut wire = self.external_ids;
        wire.imdb_id = present(wire.imdb_id).or(self.imdb_id);
        let releases = in_country(self.release_dates.results, country, |it| &it.iso_3166_1);
        Movie {
            ids: wire.with_tmdb(self.id),
            title: self.title,
            original_title: present(self.original_title),
            overview: present(self.overview),
            release_date: present(self.release_date),
            runtime_minutes: self.runtime.filter(|&minutes| minutes > 0),
            poster: present(self.poster_path),
            backdrop: present(self.backdrop_path),
            genres: genre_names(self.genres),
            content_rating: releases.and_then(|releases| {
                (releases.release_dates.into_iter())
                    .find_map(|release| present(release.certification))
            }),
            score: self.votes.score(),
            original_language: original_language(self.original_language),
            ..Default::default()
        }
    }
}

/// `/tv/{id}?append_to_response=external_ids,content_ratings`.
#[derive(Debug, Deserialize)]
pub(crate) struct ShowDetails {
    id: i64,
    name: String,
    original_name: Option<String>,
    overview: Option<String>,
    first_air_date: Option<String>,
    poster_path: Option<String>,
    backdrop_path: Option<String>,
    #[serde(default)]
    seasons: Vec<SeasonSummary>,
    #[serde(default)]
    external_ids: WireIds,
    #[serde(default)]
    genres: Vec<Genre>,
    #[serde(default)]
    content_ratings: ContentRatings,
    original_language: Option<String>,
    #[serde(flatten)]
    votes: Votes,
}

#[derive(Debug, Deserialize)]
struct SeasonSummary {
    season_number: u32,
}

impl ShowDetails {
    /// The Show, with its Content Rating in `country`.
    pub(crate) fn into_show(self, country: Option<&str>) -> Show {
        let rated = in_country(self.content_ratings.results, country, |it| &it.iso_3166_1);
        Show {
            ids: self.external_ids.with_tmdb(self.id),
            title: self.name,
            original_title: present(self.original_name),
            overview: present(self.overview),
            first_air_date: present(self.first_air_date),
            poster: present(self.poster_path),
            backdrop: present(self.backdrop_path),
            seasons: self.seasons.iter().map(|s| s.season_number).collect(),
            genres: genre_names(self.genres),
            content_rating: rated.and_then(|rated| present(rated.rating)),
            score: self.votes.score(),
            original_language: original_language(self.original_language),
            ..Default::default()
        }
    }
}

/// The language a title was made in, which TMDB gives as `xx` when it has none, such as a silent film.
fn original_language(code: Option<String>) -> Option<String> {
    present(code).filter(|code| code != "xx")
}

/// A title's `vote_average` and `vote_count`.
#[derive(Debug, Deserialize)]
struct Votes {
    vote_average: Option<f64>,
    #[serde(default)]
    vote_count: u32,
}

impl Votes {
    /// The Score, which TMDB gives as 0.0 when nobody has voted.
    fn score(&self) -> Option<f64> {
        self.vote_average.filter(|_| self.vote_count > 0)
    }
}

#[derive(Debug, Deserialize)]
struct Genre {
    name: String,
}

fn genre_names(genres: Vec<Genre>) -> Vec<String> {
    genres.into_iter().map(|genre| genre.name).collect()
}

/// `release_dates`, appended to a Movie's details: its releases by country.
#[derive(Debug, Default, Deserialize)]
struct ReleaseDates {
    #[serde(default)]
    results: Vec<CountryReleases>,
}

#[derive(Debug, Deserialize)]
struct CountryReleases {
    iso_3166_1: String,
    #[serde(default)]
    release_dates: Vec<Release>,
}

#[derive(Debug, Deserialize)]
struct Release {
    certification: Option<String>,
}

/// `content_ratings`, appended to a Show's details: its rating by country.
#[derive(Debug, Default, Deserialize)]
struct ContentRatings {
    #[serde(default)]
    results: Vec<CountryRating>,
}

#[derive(Debug, Deserialize)]
struct CountryRating {
    iso_3166_1: String,
    rating: Option<String>,
}

/// The entry for `country` among `entries`, each of which `code` gives the ISO 3166-1 code of.
/// `None` without a country: never another country's.
fn in_country<T>(entries: Vec<T>, country: Option<&str>, code: fn(&T) -> &str) -> Option<T> {
    let country = country?;
    (entries.into_iter()).find(|entry| code(entry).eq_ignore_ascii_case(country))
}

/// `/tv/{id}/season/{n}?append_to_response=external_ids`.
#[derive(Debug, Deserialize)]
pub(crate) struct SeasonDetails {
    id: i64,
    name: Option<String>,
    overview: Option<String>,
    air_date: Option<String>,
    poster_path: Option<String>,
    season_number: u32,
    #[serde(default)]
    episodes: Vec<EpisodeDetails>,
    #[serde(default)]
    external_ids: WireIds,
}

impl From<SeasonDetails> for Season {
    fn from(season: SeasonDetails) -> Self {
        Self {
            ids: season.external_ids.with_tmdb(season.id),
            number: season.season_number,
            title: present(season.name)
                .unwrap_or_else(|| format!("Season {}", season.season_number)),
            overview: present(season.overview),
            air_date: present(season.air_date),
            poster: present(season.poster_path),
            episodes: season.episodes.into_iter().map(Episode::from).collect(),
            ..Default::default()
        }
    }
}

/// An Episode, in a Season or from `/tv/{id}/season/{n}/episode/{e}`.
#[derive(Debug, Deserialize)]
pub(crate) struct EpisodeDetails {
    id: i64,
    name: Option<String>,
    overview: Option<String>,
    air_date: Option<String>,
    season_number: u32,
    episode_number: u32,
    runtime: Option<u32>,
    still_path: Option<String>,
    #[serde(default)]
    external_ids: WireIds,
}

impl From<EpisodeDetails> for Episode {
    fn from(episode: EpisodeDetails) -> Self {
        Self {
            ids: episode.external_ids.with_tmdb(episode.id),
            season: episode.season_number,
            number: episode.episode_number,
            title: present(episode.name)
                .unwrap_or_else(|| format!("Episode {}", episode.episode_number)),
            overview: present(episode.overview),
            air_date: present(episode.air_date),
            runtime_minutes: episode.runtime.filter(|&minutes| minutes > 0),
            still: present(episode.still_path),
            ..Default::default()
        }
    }
}

/// `/movie/{id}/images` or `/tv/{id}/images`, asked in every language.
#[derive(Debug, Deserialize)]
pub(crate) struct Images {
    #[serde(default)]
    posters: Vec<ImageDetails>,
    #[serde(default)]
    backdrops: Vec<ImageDetails>,
    #[serde(default)]
    logos: Vec<ImageDetails>,
}

#[derive(Debug, Deserialize)]
struct ImageDetails {
    file_path: String,
    iso_639_1: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    vote_average: Option<f64>,
    vote_count: Option<u32>,
}

impl From<Images> for Vec<Artwork> {
    fn from(images: Images) -> Self {
        [
            (ArtworkKind::ARTWORK_KIND_POSTER, images.posters),
            (ArtworkKind::ARTWORK_KIND_BACKDROP, images.backdrops),
            (ArtworkKind::ARTWORK_KIND_LOGO, images.logos),
        ]
        .into_iter()
        .flat_map(|(kind, list)| list.into_iter().map(move |image| (kind, image)))
        .filter(|(_, image)| !image.file_path.trim().is_empty())
        .map(|(kind, image)| Artwork {
            kind: kind.into(),
            path: image.file_path,
            language: present(image.iso_639_1),
            width: image.width,
            height: image.height,
            rating: image.vote_average,
            votes: image.vote_count,
            ..Default::default()
        })
        .collect()
    }
}

/// `id` as the contract carries it.
pub(crate) fn title_id(id: &ExternalId) -> TitleId {
    TitleId {
        namespace: id.namespace().to_owned(),
        value: id.value().to_owned(),
        ..Default::default()
    }
}

/// `text`, unless TMDB left it empty, as it does for unknown dates and overviews.
fn present(text: Option<String>) -> Option<String> {
    text.filter(|text| !text.trim().is_empty())
}

/// The year of a `YYYY-MM-DD` date.
fn year(date: Option<&str>) -> Option<i32> {
    date?.get(..4)?.parse().ok()
}

#[cfg(test)]
#[path = "wire_tests.rs"]
mod tests;
