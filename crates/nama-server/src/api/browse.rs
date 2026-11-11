//! What a Library holds: `nama.v1.BrowseService`.

use std::{ffi::OsStr, path::Path};

use connectrpc::{ConnectError, RequestContext, Response, ServiceRequest, ServiceResult};
use nama_proto::nama::v1::{
    self as proto, BrowseService, GetMovieRequest, GetMovieResponse, GetShowRequest,
    GetShowResponse, ListContinueWatchingRequest, ListContinueWatchingResponse,
    ListEpisodesRequest, ListEpisodesResponse, ListMoviesRequest, ListMoviesResponse,
    ListShowsRequest, ListShowsResponse, RefusalReason,
};

use crate::{
    artwork,
    browse::{self, Browse, Started},
    contents::{Episode, Movie, Show, Version},
    libraries::Kind,
    tracks::track_to_proto,
    watch_states::WatchState,
};

use super::{caller, database_failed, external_id_to_proto, refuse, required_id};

/// `nama.v1.BrowseService`: what a Library holds, for any signed-in User, which
/// [`require_access`](super::require_access) enforces.
pub(super) struct BrowseServiceHandler {
    pub(super) browse: Browse,
}

#[expect(
    refining_impl_trait,
    reason = "plain `async fn` returning the owned response is the form the generated trait \
              documents; the refined signature isn't visible outside this crate"
)]
impl BrowseService for BrowseServiceHandler {
    async fn list_movies(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, ListMoviesRequest>,
    ) -> ServiceResult<ListMoviesResponse> {
        let user = caller(&ctx)?.user.id;
        let movies = self.browse.movies(user, request.library_id).await?;
        Response::ok(ListMoviesResponse {
            movies: movies
                .into_iter()
                .map(|(movie, state)| movie_to_proto(movie, state))
                .collect(),
            ..Default::default()
        })
    }

    async fn get_movie(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, GetMovieRequest>,
    ) -> ServiceResult<GetMovieResponse> {
        let user = caller(&ctx)?.user.id;
        let request = request.to_owned_message();
        let id = required_id(request.movie_id.into_option(), "movie_id", "Movie")?;
        let (movie, state) = self.browse.movie(user, request.library_id, &id).await?;
        Response::ok(GetMovieResponse {
            movie: movie_to_proto(movie, state).into(),
            ..Default::default()
        })
    }

    async fn list_shows(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, ListShowsRequest>,
    ) -> ServiceResult<ListShowsResponse> {
        let shows = self.browse.shows(request.library_id).await?;
        Response::ok(ListShowsResponse {
            shows: shows.iter().map(show_to_proto).collect(),
            ..Default::default()
        })
    }

    async fn get_show(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, GetShowRequest>,
    ) -> ServiceResult<GetShowResponse> {
        let request = request.to_owned_message();
        let id = required_id(request.show_id.into_option(), "show_id", "Show")?;
        let show = self.browse.show(request.library_id, &id).await?;
        let seasons = show
            .seasons
            .iter()
            .map(|(&number, season)| proto::Season {
                number: proto_number(number),
                poster_url: season
                    .poster
                    .as_deref()
                    .map(|path| artwork::url(&show.plugin, path)),
                ..Default::default()
            })
            .collect();
        Response::ok(GetShowResponse {
            show: show_to_proto(&show).into(),
            seasons,
            ..Default::default()
        })
    }

    async fn list_episodes(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, ListEpisodesRequest>,
    ) -> ServiceResult<ListEpisodesResponse> {
        let user = caller(&ctx)?.user.id;
        let request = request.to_owned_message();
        let number = request.season_number;
        let show = required_id(request.show_id.into_option(), "show_id", "Show")?;
        let episodes = self
            .browse
            .episodes(user, request.library_id, &show, i64::from(number))
            .await?;
        Response::ok(ListEpisodesResponse {
            episodes: episodes
                .into_iter()
                .map(|(episode, state)| episode_to_proto(number, episode, state))
                .collect(),
            ..Default::default()
        })
    }

    async fn list_continue_watching(
        &self,
        ctx: RequestContext,
        _request: ServiceRequest<'_, ListContinueWatchingRequest>,
    ) -> ServiceResult<ListContinueWatchingResponse> {
        let started = self.browse.continue_watching(caller(&ctx)?.user.id).await?;
        Response::ok(ListContinueWatchingResponse {
            entries: started.into_iter().map(started_to_proto).collect(),
            ..Default::default()
        })
    }
}

/// A Movie as clients see it, with its images as URLs on this Server, and the caller's Watch
/// State of it, if they've started it.
fn movie_to_proto(movie: Movie, watch_state: Option<WatchState>) -> proto::Movie {
    let plugin = movie.plugin;
    let artwork_url = |path: String| artwork::url(&plugin, &path);
    proto::Movie {
        id: external_id_to_proto(movie.id).into(),
        title: movie.title,
        original_title: movie.original_title,
        overview: movie.overview,
        release_date: movie.release_date,
        runtime_minutes: movie
            .runtime_minutes
            .and_then(|minutes| u32::try_from(minutes).ok()),
        poster_url: movie.poster.map(artwork_url),
        backdrop_url: movie.backdrop.map(artwork_url),
        versions: movie.versions.iter().map(version_to_proto).collect(),
        watch_state: watch_state.map(watch_state_to_proto).into(),
        ..Default::default()
    }
}

/// A Continue Watching entry as clients see it.
fn started_to_proto(started: Started) -> proto::ContinueWatchingEntry {
    let (library_id, movie_or_episode, show) = match started {
        Started::Movie {
            library,
            movie,
            state,
        } => (library, movie_to_proto(movie, Some(state)).into(), None),
        Started::Episode {
            library,
            show,
            season,
            episode,
            state,
        } => {
            let episode = episode_to_proto(proto_number(season), episode, Some(state));
            (library, episode.into(), Some(show_to_proto(&show)))
        }
    };
    proto::ContinueWatchingEntry {
        library_id,
        movie_or_episode: Some(movie_or_episode),
        show: show.into(),
        ..Default::default()
    }
}

fn watch_state_to_proto(state: WatchState) -> proto::WatchState {
    // Stored positions and durations are never negative.
    let unsigned = |ms: i64| u64::try_from(ms).unwrap_or_default();
    proto::WatchState {
        position_ms: unsigned(state.position_ms),
        duration_ms: unsigned(state.duration_ms),
        played: state.played,
        updated_at: state.updated_at,
        ..Default::default()
    }
}

/// A Show as clients see it, without its Seasons, with its images as URLs on this Server.
fn show_to_proto(show: &Show) -> proto::Show {
    let artwork_url = |path: &str| artwork::url(&show.plugin, path);
    proto::Show {
        id: external_id_to_proto(show.id.clone()).into(),
        title: show.title.clone(),
        poster_url: show.poster().map(artwork_url),
        backdrop_url: show.backdrop.as_deref().map(artwork_url),
        ..Default::default()
    }
}

/// An Episode of Season `season` as clients see it, with the caller's Watch State of it, if
/// they've started it.
fn episode_to_proto(
    season: u32,
    episode: Episode,
    watch_state: Option<WatchState>,
) -> proto::Episode {
    proto::Episode {
        id: external_id_to_proto(episode.id).into(),
        season_number: season,
        episode_number: proto_number(episode.number),
        title: episode.title,
        overview: episode.overview,
        air_date: episode.air_date,
        runtime_minutes: episode
            .runtime_minutes
            .and_then(|minutes| u32::try_from(minutes).ok()),
        versions: episode.versions.iter().map(version_to_proto).collect(),
        watch_state: watch_state.map(watch_state_to_proto).into(),
        ..Default::default()
    }
}

/// A season or episode number as a proto `uint32`. A scan parses them from digits, so they are
/// never negative.
fn proto_number(n: i64) -> u32 {
    u32::try_from(n).unwrap_or_default()
}

/// A Version as clients see it: its file's name, not the folders it is in.
fn version_to_proto(version: &Version) -> proto::Version {
    // A stored path came from a `String`, so its name is UTF-8.
    let file_name = Path::new(&version.path)
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or_default()
        .to_owned();
    proto::Version {
        id: version.file.id,
        file_name,
        // A size is never negative.
        size: u64::try_from(version.file.size).unwrap_or_default(),
        tracks: version.tracks.iter().map(track_to_proto).collect(),
        ..Default::default()
    }
}

/// The Connect error a client gets for a Library, title or Season that Browse didn't find. Database
/// failures are logged, not sent.
impl From<browse::Error> for ConnectError {
    fn from(err: browse::Error) -> Self {
        use RefusalReason as R;
        let message = err.to_string();
        let refused = match err {
            // Every Browse and Watch State call names its Library in `library_id`.
            browse::Error::NoSuchLibrary(id) => {
                refuse(R::REFUSAL_REASON_LIBRARY_NOT_FOUND, message)
                    .field("library_id")
                    .param("libraryId", &id)
            }
            browse::Error::NothingMatched => refuse(R::REFUSAL_REASON_NOTHING_MATCHED, message),
            browse::Error::NoSuchSeason => refuse(R::REFUSAL_REASON_SEASON_NOT_FOUND, message),
            browse::Error::WrongKind { wanted } => {
                let kind = match wanted {
                    Kind::Movies => "movies",
                    Kind::Shows => "shows",
                };
                refuse(R::REFUSAL_REASON_WRONG_LIBRARY_KIND, message)
                    .field("library_id")
                    .param("libraryKind", &kind)
            }
            browse::Error::Database(err) => return database_failed(err),
        };
        refused.into()
    }
}
