//! What a Library holds: `nama.v1.BrowseService`.

use std::{ffi::OsStr, path::Path};

use connectrpc::{ConnectError, RequestContext, Response, ServiceRequest, ServiceResult};
use nama_proto::nama::v1::{
    self as proto, BrowseService, GetMovieRequest, GetMovieResponse, GetShowRequest,
    GetShowResponse, ListContinueWatchingRequest, ListContinueWatchingResponse,
    ListEpisodesRequest, ListEpisodesResponse, ListMoviesRequest, ListMoviesResponse,
    ListShowsRequest, ListShowsResponse, Permission, RefusalReason, SearchLibrariesRequest,
    SearchLibrariesResponse,
};

use crate::{
    artwork,
    browse::{self, Browse, Episode, Movie, Show, Started, Version},
    contents::{FoundEpisode, FoundMovie, FoundShow},
    custom_images::{CustomImages, Role, Slot, Urls},
    libraries::Kind,
    tracks::track_to_proto,
    watch_states::WatchState,
};

use super::{
    caller, custom_image_urls, database_failed, external_id_to_proto, proto_number, refuse,
    required_id,
};

/// `nama.v1.BrowseService`: what a Library holds, for any signed-in User, which
/// [`require_access`](super::require_access) enforces.
pub(super) struct BrowseServiceHandler {
    pub(super) browse: Browse,
    pub(super) custom_images: CustomImages,
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
        let custom = custom_image_urls(&self.custom_images).await?;
        Response::ok(ListMoviesResponse {
            movies: movies
                .into_iter()
                .map(|(movie, state)| movie_to_proto(movie, state, &custom))
                .collect(),
            ..Default::default()
        })
    }

    async fn get_movie(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, GetMovieRequest>,
    ) -> ServiceResult<GetMovieResponse> {
        let user = &caller(&ctx)?.user;
        let request = request.to_owned_message();
        let id = required_id(request.movie_id.into_option(), "movie_id", "Movie")?;
        let (movie, state, read_only_folder) =
            self.browse.movie(user.id, request.library_id, &id).await?;
        let custom = custom_image_urls(&self.custom_images).await?;
        Response::ok(GetMovieResponse {
            movie: movie_to_proto(movie, state, &custom).into(),
            // Where a Library lives is shown only to those who may delete what's in it (AGENTS.md,
            // "Clients name ids"), not to every User who can browse it.
            read_only_folder: read_only_folder
                .filter(|_| user.permissions.allow(Permission::DeleteMedia)),
            ..Default::default()
        })
    }

    async fn list_shows(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, ListShowsRequest>,
    ) -> ServiceResult<ListShowsResponse> {
        let shows = self.browse.shows(request.library_id).await?;
        let custom = custom_image_urls(&self.custom_images).await?;
        Response::ok(ListShowsResponse {
            shows: shows
                .iter()
                .map(|show| show_to_proto(show, &custom))
                .collect(),
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
        let custom = custom_image_urls(&self.custom_images).await?;
        let seasons = (show.seasons.keys())
            .map(|&number| {
                let (poster_url, custom_poster) = season_poster(&show, number, &custom);
                proto::Season {
                    number: proto_number(number),
                    poster_url,
                    custom_poster,
                    ..Default::default()
                }
            })
            .collect();
        Response::ok(GetShowResponse {
            show: show_to_proto(&show, &custom).into(),
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
        let number = i64::from(request.season_number);
        let show = required_id(request.show_id.into_option(), "show_id", "Show")?;
        let episodes = self
            .browse
            .episodes(user, request.library_id, &show, number)
            .await?;
        Response::ok(ListEpisodesResponse {
            episodes: episodes
                .into_iter()
                .map(|(episode, state)| episode_to_proto(episode, state))
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
        let custom = custom_image_urls(&self.custom_images).await?;
        Response::ok(ListContinueWatchingResponse {
            entries: (started.into_iter())
                .map(|started| started_to_proto(started, &custom))
                .collect(),
            ..Default::default()
        })
    }

    async fn search_libraries(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, SearchLibrariesRequest>,
    ) -> ServiceResult<SearchLibrariesResponse> {
        let found = self.browse.search(request.query).await?;
        let custom = custom_image_urls(&self.custom_images).await?;
        Response::ok(SearchLibrariesResponse {
            movies: (found.movies.into_iter())
                .map(|movie| found_movie_to_proto(movie, &custom))
                .collect(),
            shows: (found.shows.into_iter())
                .map(|show| found_show_to_proto(show, &custom))
                .collect(),
            episodes: found
                .episodes
                .into_iter()
                .map(found_episode_to_proto)
                .collect(),
            ..Default::default()
        })
    }
}

fn found_movie_to_proto(movie: FoundMovie, custom: &Urls) -> proto::MovieInLibrary {
    let downloaded = movie.poster.map(|path| artwork::url(&movie.plugin, &path));
    proto::MovieInLibrary {
        library_id: movie.library,
        poster_url: custom
            .of(&Slot::movie(&movie.id, Role::Poster))
            .or(downloaded),
        id: external_id_to_proto(movie.id).into(),
        title: movie.title,
        release_date: movie.release_date,
        ..Default::default()
    }
}

fn found_show_to_proto(show: FoundShow, custom: &Urls) -> proto::ShowInLibrary {
    let downloaded = show.poster.map(|path| artwork::url(&show.plugin, &path));
    proto::ShowInLibrary {
        library_id: show.library,
        poster_url: custom
            .of(&Slot::show(&show.id, Role::Poster))
            .or(downloaded),
        id: external_id_to_proto(show.id).into(),
        title: show.title,
        ..Default::default()
    }
}

fn found_episode_to_proto(episode: FoundEpisode) -> proto::EpisodeInLibrary {
    proto::EpisodeInLibrary {
        library_id: episode.library,
        id: external_id_to_proto(episode.id).into(),
        show_id: external_id_to_proto(episode.show).into(),
        show_title: episode.show_title,
        season_number: proto_number(episode.season),
        episode_number: proto_number(episode.number),
        title: episode.title,
        ..Default::default()
    }
}

/// A Movie as clients see it, with its images as URLs on this Server, its Custom Images in
/// `custom` in place of the downloaded ones, and the caller's Watch State of it, if they've
/// started it.
fn movie_to_proto(movie: Movie, watch_state: Option<WatchState>, custom: &Urls) -> proto::Movie {
    let plugin = movie.plugin;
    let artwork_url = |path: String| artwork::url(&plugin, &path);
    let custom_poster = custom.of(&Slot::movie(&movie.id, Role::Poster));
    let custom_backdrop = custom.of(&Slot::movie(&movie.id, Role::Backdrop));
    proto::Movie {
        custom_poster: custom_poster.is_some(),
        custom_backdrop: custom_backdrop.is_some(),
        poster_url: custom_poster.or_else(|| movie.poster.map(artwork_url)),
        backdrop_url: custom_backdrop.or_else(|| movie.backdrop.map(artwork_url)),
        id: external_id_to_proto(movie.id).into(),
        title: movie.title,
        original_title: movie.original_title,
        overview: movie.overview,
        release_date: movie.release_date,
        runtime_minutes: movie
            .runtime_minutes
            .and_then(|minutes| u32::try_from(minutes).ok()),
        versions: movie.versions.iter().map(version_to_proto).collect(),
        watch_state: watch_state.map(watch_state_to_proto).into(),
        ..Default::default()
    }
}

/// A Continue Watching entry as clients see it, with the Custom Images in `custom`.
fn started_to_proto(started: Started, custom: &Urls) -> proto::ContinueWatchingEntry {
    let (library_id, movie_or_episode) = match started {
        Started::Movie {
            library,
            movie,
            state,
        } => (library, movie_to_proto(movie, Some(state), custom).into()),
        Started::Episode {
            library,
            show,
            episode,
            state,
        } => {
            let started_episode = proto::ContinueWatchingEpisode {
                episode: episode_to_proto(episode, Some(state)).into(),
                show: show_to_proto(&show, custom).into(),
                ..Default::default()
            };
            (library, started_episode.into())
        }
    };
    proto::ContinueWatchingEntry {
        library_id,
        movie_or_episode: Some(movie_or_episode),
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

/// A Show as clients see it, without its Seasons, with its images as URLs on this Server and
/// its Custom Images in `custom` in place of the downloaded ones. Without a Poster of its own, it
/// has its first Season's.
fn show_to_proto(show: &Show, custom: &Urls) -> proto::Show {
    let custom_poster = custom.of(&Slot::show(&show.id, Role::Poster));
    let custom_backdrop = custom.of(&Slot::show(&show.id, Role::Backdrop));
    let first_season = show.seasons.keys().next();
    let downloaded_backdrop =
        (show.backdrop.as_deref()).map(|path| artwork::url(&show.plugin, path));
    proto::Show {
        id: external_id_to_proto(show.id.clone()).into(),
        title: show.title.clone(),
        custom_poster: custom_poster.is_some(),
        custom_backdrop: custom_backdrop.is_some(),
        poster_url: custom_poster
            .or_else(|| first_season.and_then(|&number| season_poster(show, number, custom).0)),
        backdrop_url: custom_backdrop.or(downloaded_backdrop),
        ..Default::default()
    }
}

/// The URL of the Poster of `show`'s Season `number`, its Custom Image in `custom` if it has one,
/// and whether it is.
fn season_poster(show: &Show, number: i64, custom: &Urls) -> (Option<String>, bool) {
    if let Some(url) = custom.of(&Slot::season(&show.id, number)) {
        return (Some(url), true);
    }
    let downloaded = (show.seasons.get(&number)).and_then(|season| season.poster.as_deref());
    (
        downloaded.map(|path| artwork::url(&show.plugin, path)),
        false,
    )
}

/// An Episode as clients see it, with the caller's Watch State of it, if they've started it.
fn episode_to_proto(episode: Episode, watch_state: Option<WatchState>) -> proto::Episode {
    proto::Episode {
        id: external_id_to_proto(episode.id).into(),
        season_number: proto_number(episode.season),
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

/// The Connect error a client gets for a Library, title, Season or Version that Browse didn't
/// find. Database failures are logged, not sent.
impl From<browse::Error> for ConnectError {
    fn from(err: browse::Error) -> Self {
        // Every Browse call that looks up a Library names it in `library_id`, and a Version in
        // `version_id`; reporting progress looks no Library up.
        refused(err, "library_id", "version_id")
    }
}

/// The Connect error for `err`, from a request naming its Library in the field `library_field`
/// and its Version in `version_field`.
pub(super) fn refused(
    err: browse::Error,
    library_field: &str,
    version_field: &str,
) -> ConnectError {
    use RefusalReason as R;
    let message = err.to_string();
    let refused = match err {
        browse::Error::NoSuchLibrary(id) => refuse(R::REFUSAL_REASON_LIBRARY_NOT_FOUND, message)
            .field(library_field)
            .param("libraryId", &id),
        browse::Error::NothingMatched => refuse(R::REFUSAL_REASON_NOTHING_MATCHED, message),
        browse::Error::NoSuchSeason => refuse(R::REFUSAL_REASON_SEASON_NOT_FOUND, message),
        browse::Error::NoSuchVersion => {
            refuse(R::REFUSAL_REASON_VERSION_NOT_FOUND, message).field(version_field)
        }
        browse::Error::WrongKind { wanted } => {
            let kind = match wanted {
                Kind::Movies => "movies",
                Kind::Shows => "shows",
            };
            refuse(R::REFUSAL_REASON_WRONG_LIBRARY_KIND, message)
                .field(library_field)
                .param("libraryKind", &kind)
        }
        browse::Error::Database(err) => return database_failed(err),
    };
    refused.into()
}
