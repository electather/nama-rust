//! TMDB's v3 JSON, as far as the Plugin reads it, and how it maps onto the plugin API's types.

use nama_plugin_api::metadata::{Candidate, Episode, ExternalId, ExternalIds, Movie, Season, Show};
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
        Self {
            id: ExternalId::Tmdb(movie.id),
            year: year(movie.release_date.as_deref()),
            title: movie.title,
            original_title: present(movie.original_title),
            poster: present(movie.poster_path),
            origin_country: Vec::new(),
            vote_count: movie.vote_count,
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
        Self {
            id: ExternalId::Tmdb(show.id),
            year: year(show.first_air_date.as_deref()),
            title: show.name,
            original_title: present(show.original_name),
            poster: present(show.poster_path),
            origin_country: show.origin_country,
            vote_count: show.vote_count,
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
    fn with_tmdb(self, tmdb: i64) -> ExternalIds {
        ExternalIds {
            tmdb: Some(tmdb),
            imdb: present(self.imdb_id),
            tvdb: self.tvdb_id,
        }
    }
}

/// `/movie/{id}?append_to_response=external_ids`.
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
    #[serde(default)]
    external_ids: WireIds,
}

impl From<MovieDetails> for Movie {
    fn from(movie: MovieDetails) -> Self {
        let mut ids = movie.external_ids;
        ids.imdb_id = present(ids.imdb_id).or(movie.imdb_id);
        Self {
            ids: ids.with_tmdb(movie.id),
            title: movie.title,
            original_title: present(movie.original_title),
            overview: present(movie.overview),
            release_date: present(movie.release_date),
            runtime_minutes: movie.runtime.filter(|&minutes| minutes > 0),
            poster: present(movie.poster_path),
            backdrop: present(movie.backdrop_path),
        }
    }
}

/// `/tv/{id}?append_to_response=external_ids`.
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
}

#[derive(Debug, Deserialize)]
struct SeasonSummary {
    season_number: u32,
}

impl From<ShowDetails> for Show {
    fn from(show: ShowDetails) -> Self {
        Self {
            ids: show.external_ids.with_tmdb(show.id),
            title: show.name,
            original_title: present(show.original_name),
            overview: present(show.overview),
            first_air_date: present(show.first_air_date),
            poster: present(show.poster_path),
            backdrop: present(show.backdrop_path),
            seasons: show.seasons.iter().map(|s| s.season_number).collect(),
        }
    }
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
        }
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
