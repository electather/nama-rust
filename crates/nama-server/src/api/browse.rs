//! What a Library holds: `nama.v1.BrowseService`.

use std::{ffi::OsStr, path::Path};

use connectrpc::{RequestContext, Response, ServiceRequest, ServiceResult};
use nama_proto::nama::v1::{
    self as proto, BrowseService, GetMovieRequest, GetMovieResponse, GetNextEpisodeRequest,
    GetNextEpisodeResponse, GetShowRequest, GetShowResponse, GetUnmatchedVersionRequest,
    GetUnmatchedVersionResponse, ListContinueWatchingRequest, ListContinueWatchingResponse,
    ListEpisodesRequest, ListEpisodesResponse, ListGenresRequest, ListGenresResponse,
    ListHomeRowTitlesRequest, ListHomeRowTitlesResponse, ListHomeRowsRequest, ListHomeRowsResponse,
    ListMoviesRequest, ListMoviesResponse, ListShowsRequest, ListShowsResponse,
    ListUnmatchedRequest, ListUnmatchedResponse, MovieSort, Permission, SearchLibrariesRequest,
    SearchLibrariesResponse, ShowSort, SortDirection, WatchedFilter,
};

use crate::{
    browse::{Browse, Episode, ListedShow, Movie, Show, Started, Version},
    home_rows::{HomeRows, OnServer, RowTitle},
    library::{
        Artwork, FoundEpisode, FoundMovie, FoundShow, ImageRef, Item, LibraryMovie, Listing, Role,
        Slot, TitleSort, Unmatched, WatchState, Watched, artwork_url,
    },
    playback::track_to_proto,
};

use super::{
    ImageSources, ShownImages, caller, external_id_to_proto, proto_number,
    requests::{missing_movie_to_proto, missing_show_to_proto},
    required_id,
};

mod refusals;

use refusals::home_rows_refused;
pub(super) use refusals::{Fields, not_found, refused};

/// `nama.v1.BrowseService`: what a Library holds, for any signed-in User, which
/// [`require_access`](super::require_access) enforces.
pub(super) struct BrowseServiceHandler {
    pub(super) browse: Browse,
    pub(super) home_rows: HomeRows,
    /// Serves the posters of the missing titles Home rows show, which no Match stores.
    pub(super) artwork: Artwork,
    pub(super) images: ImageSources,
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
        let request = request.to_owned_message();
        let sort = match request.sort.as_known() {
            Some(MovieSort::Added) => TitleSort::Added,
            Some(MovieSort::Released) => TitleSort::Released,
            _ => TitleSort::Title,
        };
        let filter = request.filter.into_option().unwrap_or_default();
        let listing = listing(
            Filter {
                genres: filter.genres,
                min_year: filter.min_year,
                max_year: filter.max_year,
                watched: filter.watched.as_known(),
            },
            sort,
            request.direction.as_known(),
        );
        let page = (self.browse)
            .movie_page(user, request.library_id, &listing, &request.page)
            .await;
        let (movies, page) = page.map_err(refused(Fields::LIBRARY))?;
        let images = self.images.shown().await?;
        Response::ok(ListMoviesResponse {
            movies: movies
                .into_iter()
                .map(|(movie, state)| movie_summary(movie, state, &images))
                .collect(),
            page: page.into(),
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
        let found = self.browse.movie(user.id, request.library_id, &id).await;
        let (movie, state, read_only_folder) = found.map_err(refused(Fields {
            title: "movie_id",
            ..Fields::LIBRARY
        }))?;
        let images = self.images.shown().await?;
        Response::ok(GetMovieResponse {
            movie: movie_to_proto(movie, state, &images).into(),
            // Where a Library lives is shown only to those who may delete what's in it (AGENTS.md,
            // "Clients name ids"), not to every User who can browse it.
            read_only_folder: read_only_folder
                .filter(|_| user.permissions.allow(Permission::DeleteMedia)),
            ..Default::default()
        })
    }

    async fn list_shows(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, ListShowsRequest>,
    ) -> ServiceResult<ListShowsResponse> {
        let user = caller(&ctx)?.user.id;
        let request = request.to_owned_message();
        let sort = match request.sort.as_known() {
            Some(ShowSort::Added) => TitleSort::Added,
            Some(ShowSort::Released) => TitleSort::Released,
            _ => TitleSort::Title,
        };
        let filter = request.filter.into_option().unwrap_or_default();
        let listing = listing(
            Filter {
                genres: filter.genres,
                min_year: filter.min_year,
                max_year: filter.max_year,
                watched: filter.watched.as_known(),
            },
            sort,
            request.direction.as_known(),
        );
        let page = (self.browse)
            .show_page(user, request.library_id, &listing, &request.page)
            .await;
        let (shows, page) = page.map_err(refused(Fields::LIBRARY))?;
        let images = self.images.shown().await?;
        Response::ok(ListShowsResponse {
            shows: shows
                .into_iter()
                .map(|listed| show_summary(&listed, &images))
                .collect(),
            page: page.into(),
            ..Default::default()
        })
    }

    async fn list_genres(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, ListGenresRequest>,
    ) -> ServiceResult<ListGenresResponse> {
        let genres = self.browse.genres(request.library_id).await;
        Response::ok(ListGenresResponse {
            genres: genres.map_err(refused(Fields::LIBRARY))?,
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
        let show = self.browse.show(request.library_id, &id).await;
        let show = show.map_err(refused(Fields {
            title: "show_id",
            ..Fields::LIBRARY
        }))?;
        let images = self.images.shown().await?;
        let seasons = (show.seasons.keys())
            .map(|&number| {
                let (poster_url, custom_poster) = season_poster(&show, number, &images);
                proto::Season {
                    number: proto_number(number),
                    poster_thumbhash: images.thumbhash(poster_url.as_deref()),
                    poster_url,
                    custom_poster,
                    ..Default::default()
                }
            })
            .collect();
        Response::ok(GetShowResponse {
            show: show_to_proto(&show, &images).into(),
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
            .await
            .map_err(refused(Fields {
                title: "show_id",
                season: "season_number",
                ..Fields::LIBRARY
            }))?;
        let images = self.images.shown().await?;
        Response::ok(ListEpisodesResponse {
            episodes: episodes
                .into_iter()
                .map(|(episode, state)| episode_to_proto(episode, state, &images))
                .collect(),
            ..Default::default()
        })
    }

    async fn get_next_episode(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, GetNextEpisodeRequest>,
    ) -> ServiceResult<GetNextEpisodeResponse> {
        let user = caller(&ctx)?.user.id;
        let request = request.to_owned_message();
        let show = required_id(request.show_id.into_option(), "show_id", "Show")?;
        let numbers = (
            i64::from(request.season_number),
            i64::from(request.episode_number),
        );
        let next = self
            .browse
            .episode_after(user, request.library_id, &show, numbers)
            .await
            .map_err(refused(Fields {
                title: "show_id",
                season: "season_number",
                episode: "episode_number",
                ..Fields::LIBRARY
            }))?;
        let images = self.images.shown().await?;
        Response::ok(GetNextEpisodeResponse {
            next: next
                .map(|(episode, state)| episode_to_proto(episode, state, &images))
                .into(),
            ..Default::default()
        })
    }

    async fn list_continue_watching(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, ListContinueWatchingRequest>,
    ) -> ServiceResult<ListContinueWatchingResponse> {
        let user = caller(&ctx)?.user.id;
        let asked = request.to_owned_message().page;
        let listed = self.browse.continue_watching(user, &asked).await;
        let (started, page) = listed.map_err(refused(Fields::NONE))?;
        let images = self.images.shown().await?;
        Response::ok(ListContinueWatchingResponse {
            entries: (started.into_iter())
                .map(|started| started_to_proto(started, &images))
                .collect(),
            page: page.into(),
            ..Default::default()
        })
    }

    async fn list_home_rows(
        &self,
        ctx: RequestContext,
        _request: ServiceRequest<'_, ListHomeRowsRequest>,
    ) -> ServiceResult<ListHomeRowsResponse> {
        let reader = &caller(&ctx)?.user;
        let rows = self.home_rows.list(reader).await;
        let rows = rows.map_err(home_rows_refused)?;
        Response::ok(ListHomeRowsResponse {
            rows: (rows.into_iter())
                .map(|(plugin, row)| proto::HomeRow {
                    plugin_id: plugin.as_str().to_owned(),
                    key: row.key,
                    heading: row.heading,
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        })
    }

    async fn list_home_row_titles(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, ListHomeRowTitlesRequest>,
    ) -> ServiceResult<ListHomeRowTitlesResponse> {
        let reader = &caller(&ctx)?.user;
        let request = request.to_owned_message();
        let plugin = request.plugin_id.into();
        let listed = (self.home_rows)
            .titles(reader, &plugin, &request.key, &request.page)
            .await;
        let (titles, page) = listed.map_err(home_rows_refused)?;
        let images = self.images.shown().await?;
        let artwork = &self.artwork;
        Response::ok(ListHomeRowTitlesResponse {
            titles: (titles.into_iter())
                .map(|title| proto::HomeRowTitle {
                    movie_or_show: Some(match title {
                        RowTitle::OnServer(OnServer::Movie(movie)) => {
                            found_movie_to_proto(movie, &images).into()
                        }
                        RowTitle::OnServer(OnServer::Show(show)) => {
                            found_show_to_proto(show, &images).into()
                        }
                        RowTitle::MissingMovie(movie, open) => {
                            missing_movie_to_proto(artwork, movie, open, &images).into()
                        }
                        RowTitle::MissingShow(show) => {
                            missing_show_to_proto(artwork, show, &images).into()
                        }
                    }),
                    ..Default::default()
                })
                .collect(),
            page: page.into(),
            ..Default::default()
        })
    }

    async fn search_libraries(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, SearchLibrariesRequest>,
    ) -> ServiceResult<SearchLibrariesResponse> {
        let request = request.to_owned_message();
        let found = self.browse.search(&request.query, &request.page).await;
        let (found, page) = found.map_err(refused(Fields::NONE))?;
        let images = self.images.shown().await?;
        Response::ok(SearchLibrariesResponse {
            movies: (found.movies.into_iter())
                .map(|movie| found_movie_to_proto(movie, &images))
                .collect(),
            shows: (found.shows.into_iter())
                .map(|show| found_show_to_proto(show, &images))
                .collect(),
            episodes: found
                .episodes
                .into_iter()
                .map(found_episode_to_proto)
                .collect(),
            page: page.into(),
            ..Default::default()
        })
    }

    async fn list_unmatched(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, ListUnmatchedRequest>,
    ) -> ServiceResult<ListUnmatchedResponse> {
        let request = request.to_owned_message();
        let listed = (self.browse)
            .unmatched(request.library_id, &request.page)
            .await;
        let (unmatched, page) = listed.map_err(refused(Fields::LIBRARY))?;
        Response::ok(ListUnmatchedResponse {
            items: (unmatched.into_iter()).map(unmatched_summary).collect(),
            page: page.into(),
            ..Default::default()
        })
    }

    async fn get_unmatched_version(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, GetUnmatchedVersionRequest>,
    ) -> ServiceResult<GetUnmatchedVersionResponse> {
        let found = (self.browse)
            .unmatched_version(request.library_id, request.version_id)
            .await;
        let (unmatched, version) = found.map_err(refused(Fields {
            version: "version_id",
            ..Fields::LIBRARY
        }))?;
        Response::ok(GetUnmatchedVersionResponse {
            item: unmatched_summary(unmatched).into(),
            version: version_to_proto(&version).into(),
            ..Default::default()
        })
    }
}

/// A `MovieFilter` or `ShowFilter`, which differ only in their type.
struct Filter {
    genres: Vec<String>,
    min_year: Option<u32>,
    max_year: Option<u32>,
    /// `None` for a value this Server doesn't know, which keeps every title, as unspecified does.
    watched: Option<WatchedFilter>,
}

/// What `filter`, `sort` and `direction` ask of a page of Movies or Shows. A direction this
/// Server doesn't know runs the sort's own way: ascending by title, else most recent first.
fn listing(filter: Filter, sort: TitleSort, direction: Option<SortDirection>) -> Listing {
    let descending = match direction {
        Some(SortDirection::Ascending) => false,
        Some(SortDirection::Descending) => true,
        _ => sort != TitleSort::Title,
    };
    Listing {
        genres: filter.genres,
        min_year: filter.min_year,
        max_year: filter.max_year,
        watched: match filter.watched {
            Some(WatchedFilter::NotStarted) => Some(Watched::NotStarted),
            Some(WatchedFilter::Started) => Some(Watched::Started),
            Some(WatchedFilter::Played) => Some(Watched::Played),
            _ => None,
        },
        sort,
        descending,
    }
}

/// A Movie as a Library's grid shows it, with its Poster as `images` gives it and the caller's
/// Watch State of it, if they've started it.
fn movie_summary(
    movie: LibraryMovie,
    watch_state: Option<WatchState>,
    images: &ShownImages,
) -> proto::MovieSummary {
    let downloaded = (movie.poster).map(|image| artwork_url(image.plugin.as_str(), &image.path));
    let poster_url = (images.movie_image(&movie.id, &movie.ids, Role::Poster)).or(downloaded);
    proto::MovieSummary {
        id: external_id_to_proto(&movie.id).into(),
        title: movie.title,
        release_date: movie.release_date,
        poster_thumbhash: images.thumbhash(poster_url.as_deref()),
        poster_url,
        watch_state: watch_state.map(watch_state_to_proto).into(),
        ..Default::default()
    }
}

/// A Show as a Library's grid shows it, with its Poster as `images` gives it.
fn show_summary(listed: &ListedShow, images: &ShownImages) -> proto::ShowSummary {
    let show = show_to_proto(&listed.show, images);
    proto::ShowSummary {
        id: show.id,
        title: show.title,
        poster_url: show.poster_url,
        poster_thumbhash: show.poster_thumbhash,
        first_air_date: show.first_air_date,
        episode_count: listed.episodes,
        played_episode_count: listed.played,
        ..Default::default()
    }
}

fn found_movie_to_proto(movie: FoundMovie, images: &ShownImages) -> proto::MovieInLibrary {
    let downloaded = (movie.poster).map(|image| artwork_url(image.plugin.as_str(), &image.path));
    let poster_url = (images.movie_image(&movie.id, &movie.ids, Role::Poster)).or(downloaded);
    proto::MovieInLibrary {
        library_id: movie.library,
        poster_thumbhash: images.thumbhash(poster_url.as_deref()),
        poster_url,
        id: external_id_to_proto(&movie.id).into(),
        title: movie.title,
        release_date: movie.release_date,
        ..Default::default()
    }
}

fn found_show_to_proto(show: FoundShow, images: &ShownImages) -> proto::ShowInLibrary {
    let downloaded = (show.poster).map(|image| artwork_url(image.plugin.as_str(), &image.path));
    let poster_url = (images.show_image(&show.id, &show.ids, Role::Poster)).or(downloaded);
    proto::ShowInLibrary {
        library_id: show.library,
        poster_thumbhash: images.thumbhash(poster_url.as_deref()),
        poster_url,
        id: external_id_to_proto(&show.id).into(),
        title: show.title,
        ..Default::default()
    }
}

fn found_episode_to_proto(episode: FoundEpisode) -> proto::EpisodeInLibrary {
    proto::EpisodeInLibrary {
        library_id: episode.library,
        id: external_id_to_proto(&episode.id).into(),
        show_id: external_id_to_proto(&episode.show).into(),
        show_title: episode.show_title,
        season_number: proto_number(episode.season),
        episode_number: proto_number(episode.number),
        title: episode.title,
        ..Default::default()
    }
}

/// A Movie as clients see it, with its images as URLs on this Server and their thumbhashes, the
/// ones `images` shows in place of the downloaded ones, and the caller's Watch State of it,
/// if they've started it.
fn movie_to_proto(
    movie: Movie,
    watch_state: Option<WatchState>,
    images: &ShownImages,
) -> proto::Movie {
    let artwork_url = |image: ImageRef| artwork_url(image.plugin.as_str(), &image.path);
    let is_custom = |role| {
        (images.custom)
            .of_title(&movie.id, &movie.ids, |id| Slot::movie(id, role))
            .is_some()
    };
    let shown = |role| images.movie_image(&movie.id, &movie.ids, role);
    let poster_url = shown(Role::Poster).or_else(|| movie.poster.map(artwork_url));
    let backdrop_url = shown(Role::Backdrop).or_else(|| movie.backdrop.map(artwork_url));
    proto::Movie {
        title_logo_url: movie.logo.map(artwork_url),
        genres: movie.genres,
        content_rating: movie.content_rating,
        custom_poster: is_custom(Role::Poster),
        custom_backdrop: is_custom(Role::Backdrop),
        poster_thumbhash: images.thumbhash(poster_url.as_deref()),
        backdrop_thumbhash: images.thumbhash(backdrop_url.as_deref()),
        poster_url,
        backdrop_url,
        id: external_id_to_proto(&movie.id).into(),
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

/// A Continue Watching entry as clients see it, with its images as `images` gives them.
fn started_to_proto(started: Started, images: &ShownImages) -> proto::ContinueWatchingEntry {
    let (library_id, movie_or_episode) = match started {
        Started::Movie {
            library,
            movie,
            state,
        } => (library, movie_to_proto(movie, Some(state), images).into()),
        Started::Episode {
            library,
            show,
            episode,
            state,
        } => {
            let started_episode = proto::ContinueWatchingEpisode {
                episode: episode_to_proto(episode, state, images).into(),
                show: show_to_proto(&show, images).into(),
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

/// A Show as clients see it, without its Seasons, with its images as URLs on this Server and their
/// thumbhashes, the ones `images` shows in place of the downloaded ones.
fn show_to_proto(show: &Show, images: &ShownImages) -> proto::Show {
    let artwork_url = |image: &ImageRef| artwork_url(image.plugin.as_str(), &image.path);
    let is_custom = |role| {
        (images.custom)
            .of_title(&show.id, &show.ids, |id| Slot::show(id, role))
            .is_some()
    };
    let shown = |role| images.show_image(&show.id, &show.ids, role);
    let poster_url = shown(Role::Poster).or_else(|| show.poster().map(artwork_url));
    let backdrop_url = shown(Role::Backdrop).or_else(|| show.backdrop.as_ref().map(artwork_url));
    proto::Show {
        id: external_id_to_proto(&show.id).into(),
        title: show.title.clone(),
        custom_poster: is_custom(Role::Poster),
        custom_backdrop: is_custom(Role::Backdrop),
        poster_thumbhash: images.thumbhash(poster_url.as_deref()),
        backdrop_thumbhash: images.thumbhash(backdrop_url.as_deref()),
        poster_url,
        backdrop_url,
        title_logo_url: show.logo.as_ref().map(artwork_url),
        genres: show.genres.clone(),
        content_rating: show.content_rating.clone(),
        overview: show.overview.clone(),
        first_air_date: show.first_air_date.clone(),
        ..Default::default()
    }
}

/// The URL of the Poster of `show`'s Season `number`, its Custom Image in `images` if it has one,
/// and whether it is; else the image beside its media, else the downloaded one.
fn season_poster(show: &Show, number: i64, images: &ShownImages) -> (Option<String>, bool) {
    if let Some(url) = (images.custom).of_title(&show.id, &show.ids, |id| Slot::season(id, number))
    {
        return (Some(url), true);
    }
    let local = images.local.of_season(&show.id, &show.ids, number);
    let downloaded = (show.seasons.get(&number)).and_then(|season| season.poster.as_ref());
    let url =
        local.or_else(|| downloaded.map(|image| artwork_url(image.plugin.as_str(), &image.path)));
    (url, false)
}

/// An Episode as clients see it, with its Still as a URL on this Server and its thumbhash from
/// `images`, and the caller's Watch State of it, if they've started it.
fn episode_to_proto(
    episode: Episode,
    watch_state: Option<WatchState>,
    images: &ShownImages,
) -> proto::Episode {
    let still_url = (episode.still).map(|path| artwork_url(&episode.plugin, &path));
    proto::Episode {
        still_thumbhash: images.thumbhash(still_url.as_deref()),
        still_url,
        id: external_id_to_proto(&episode.id).into(),
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

/// `unmatched` as any User sees it: its ids and what its file's name was parsed as, never where the
/// file is (ADR 0069).
pub(super) fn unmatched_summary(unmatched: Unmatched) -> proto::UnmatchedItemSummary {
    let (kind, title, year, numbers) = match unmatched.item {
        Item::Movie { title, year, .. } => (proto::TitleKind::TITLE_KIND_MOVIE, title, year, None),
        Item::Episode {
            show,
            year,
            season,
            episode,
            ..
        } => (
            proto::TitleKind::TITLE_KIND_SHOW,
            show,
            year,
            Some((season, episode)),
        ),
    };
    proto::UnmatchedItemSummary {
        item_id: unmatched.id,
        version_id: unmatched.version_id,
        kind: kind.into(),
        title,
        year: year.and_then(int32),
        season: numbers.and_then(|(season, _)| int32(season)),
        episode: numbers.and_then(|(_, episode)| int32(episode)),
        ..Default::default()
    }
}

/// `n` as a proto `int32`, or `None` if it doesn't fit, which no name a scan parses gives.
fn int32(n: i64) -> Option<i32> {
    i32::try_from(n).ok()
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

#[cfg(test)]
#[path = "browse_tests.rs"]
mod tests;
