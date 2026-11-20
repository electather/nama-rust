//! What a Library holds: `nama.v1.BrowseService`.

use std::{ffi::OsStr, path::Path, sync::Arc};

use connectrpc::{ConnectError, RequestContext, Response, ServiceRequest, ServiceResult};
use nama_plugin_api::metadata::ExternalIds;
use nama_proto::nama::v1::{
    self as proto, BrowseService, GetMovieRequest, GetMovieResponse, GetNextEpisodeRequest,
    GetNextEpisodeResponse, GetShowRequest, GetShowResponse, GetUnmatchedVersionRequest,
    GetUnmatchedVersionResponse, ListContinueWatchingRequest, ListContinueWatchingResponse,
    ListEpisodesRequest, ListEpisodesResponse, ListFeaturedRequest, ListFeaturedResponse,
    ListGenresRequest, ListGenresResponse, ListHomeRowTitlesRequest, ListHomeRowTitlesResponse,
    ListHomeRowsRequest, ListHomeRowsResponse, ListMoviesRequest, ListMoviesResponse,
    ListShowsRequest, ListShowsResponse, ListUnmatchedRequest, ListUnmatchedResponse, MovieSort,
    Permission, SearchLibrariesRequest, SearchLibrariesResponse, ShowSort, SortDirection,
    WatchedFilter, home_row::KernelOrPlugin,
};

use crate::{
    browse::{Browse, Episode, Featured, ListedShow, Movie, Show, Started, Version, season_count},
    db::Database,
    home_rows::{HomeRows, OnServer, RowTitle},
    library::{
        Downloaded, FoundEpisode, FoundMovie, FoundShow, ImageRef, Item, LibraryMovie, Listing,
        Role, ShownImages, TitleImage, TitleKind, TitleSort, Unmatched, WatchState, Watched,
    },
    playback::track_to_proto,
    server_settings::{HomeRowId, Settings},
    users::{User, UserHomeLayout},
    watchlist,
};

use super::{
    ImageSources, caller, database_failed, external_id_to_proto, proto_number,
    requests::{missing_movie_to_proto, missing_show_to_proto},
    required_id,
};

mod refusals;
mod titles;

use refusals::home_rows_refused;
pub(super) use refusals::{Fields, not_found, refused};
use titles::{featured_to_proto, started_to_proto};

/// `nama.v1.BrowseService`: what a Library holds, for any signed-in User, which
/// [`require_access`](super::require_access) enforces.
pub(super) struct BrowseServiceHandler {
    pub(super) browse: Browse,
    pub(super) home_rows: HomeRows,
    /// Holds the default Home layout, which orders and hides the Home rows and Featured.
    pub(super) settings: Settings,
    pub(super) images: ImageSources,
    /// Holds the Watchlists that say whether a Movie or Show is on the caller's, and each User's
    /// Home layout.
    pub(super) db: Arc<Database>,
}

/// Every Home row `reader` could see now, in no layout's order: Continue Watching, then the
/// Plugins' rows.
pub(super) async fn home_rows_of(
    home_rows: &HomeRows,
    reader: &User,
) -> Result<Vec<(HomeRowId, KernelOrPlugin)>, ConnectError> {
    let plugin_rows = home_rows.list(reader).await.map_err(home_rows_refused)?;
    let plugin_rows = plugin_rows.into_iter().map(|(id, row)| {
        let row = proto::PluginHomeRow {
            plugin_id: id.plugin().map_or_default(ToString::to_string),
            key: row.key,
            heading: row.heading,
            ..Default::default()
        };
        (id, row.into())
    });
    let continue_watching = proto::KernelHomeRow::KERNEL_HOME_ROW_CONTINUE_WATCHING;
    let continue_watching = (
        HomeRowId::ContinueWatching,
        KernelOrPlugin::Kernel(continue_watching.into()),
    );
    Ok(std::iter::once(continue_watching)
        .chain(plugin_rows)
        .collect())
}

impl BrowseServiceHandler {
    /// Whether the title of `kind` known by `ids` is on the User `user`'s Watchlist.
    async fn on_watchlist(
        &self,
        user: i64,
        kind: TitleKind,
        ids: &ExternalIds,
    ) -> Result<bool, ConnectError> {
        let on = watchlist::has(&self.db, user, kind, ids).await;
        on.map_err(database_failed)
    }
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
        let images = self.images.shown().await.map_err(database_failed)?;
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
        let on_watchlist = (self.on_watchlist(user.id, TitleKind::Movie, &movie.ids)).await?;
        let images = self.images.shown().await.map_err(database_failed)?;
        Response::ok(GetMovieResponse {
            movie: proto::Movie {
                on_watchlist,
                ..movie_to_proto(movie, state, &images)
            }
            .into(),
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
        let images = self.images.shown().await.map_err(database_failed)?;
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
        ctx: RequestContext,
        request: ServiceRequest<'_, GetShowRequest>,
    ) -> ServiceResult<GetShowResponse> {
        let user = caller(&ctx)?.user.id;
        let request = request.to_owned_message();
        let id = required_id(request.show_id.into_option(), "show_id", "Show")?;
        let show = self.browse.show(request.library_id, &id).await;
        let show = show.map_err(refused(Fields {
            title: "show_id",
            ..Fields::LIBRARY
        }))?;
        let on_watchlist = (self.on_watchlist(user, TitleKind::Show, &show.ids)).await?;
        let images = self.images.shown().await.map_err(database_failed)?;
        let seasons = (show.seasons.iter())
            .map(|(&number, season)| {
                let downloaded = season.poster.as_ref().map(Downloaded::stored);
                let poster = TitleImage::SeasonPoster(number);
                let poster = images.of_title(&show.id, &show.ids, poster, downloaded);
                proto::Season {
                    number: proto_number(number),
                    poster_url: poster.url,
                    poster_thumbhash: poster.thumbhash,
                    custom_poster: poster.custom,
                    ..Default::default()
                }
            })
            .collect();
        Response::ok(GetShowResponse {
            show: proto::Show {
                on_watchlist,
                ..show_to_proto(&show, &images)
            }
            .into(),
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
        let images = self.images.shown().await.map_err(database_failed)?;
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
        let images = self.images.shown().await.map_err(database_failed)?;
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
        let images = self.images.shown().await.map_err(database_failed)?;
        let mut entries = Vec::with_capacity(started.len());
        // ponytail: one query per entry, on a page of at most 100; ask for the page's at once if
        // Continue Watching gets slow.
        for entry in started {
            let (kind, ids) = match &entry {
                Started::Movie { movie, .. } => (TitleKind::Movie, &movie.ids),
                Started::Episode { show, .. } => (TitleKind::Show, &show.ids),
            };
            let on_watchlist = self.on_watchlist(user, kind, ids).await?;
            entries.push(started_to_proto(entry, on_watchlist, &images));
        }
        Response::ok(ListContinueWatchingResponse {
            entries,
            page: page.into(),
            ..Default::default()
        })
    }

    async fn list_featured(
        &self,
        ctx: RequestContext,
        _request: ServiceRequest<'_, ListFeaturedRequest>,
    ) -> ServiceResult<ListFeaturedResponse> {
        let user = caller(&ctx)?.user.id;
        let own = UserHomeLayout::of(&self.db, user).await;
        let own = own.map_err(database_failed)?;
        if !own.shows_featured(&self.settings.now().default_home_layout) {
            return Response::ok(ListFeaturedResponse::default());
        }
        let suggested = self.home_rows.featured(user).await;
        let suggested = suggested.map_err(home_rows_refused)?;
        let featured = self.browse.featured(user, &suggested).await;
        let featured = featured.map_err(refused(Fields::NONE))?;
        let images = self.images.shown().await.map_err(database_failed)?;
        let mut titles = Vec::with_capacity(featured.len());
        // ponytail: one query per title, as Continue Watching asks; Featured lists a handful.
        for title in featured {
            let (kind, ids) = match &title {
                Featured::Movie { movie, .. } => (TitleKind::Movie, &movie.ids),
                Featured::Show { show, .. } => (TitleKind::Show, &show.ids),
            };
            let on_watchlist = self.on_watchlist(user, kind, ids).await?;
            titles.push(featured_to_proto(title, on_watchlist, &images));
        }
        Response::ok(ListFeaturedResponse {
            titles,
            ..Default::default()
        })
    }

    async fn list_home_rows(
        &self,
        ctx: RequestContext,
        _request: ServiceRequest<'_, ListHomeRowsRequest>,
    ) -> ServiceResult<ListHomeRowsResponse> {
        let reader = &caller(&ctx)?.user;
        let rows = home_rows_of(&self.home_rows, reader).await?;
        let own = UserHomeLayout::of(&self.db, reader.id).await;
        let own = own.map_err(database_failed)?;
        let rows = own.arrange(&self.settings.now().default_home_layout, rows);
        Response::ok(ListHomeRowsResponse {
            rows: (rows.into_iter())
                .filter(|&(_, shown)| shown)
                .map(|((_, row), _)| proto::HomeRow {
                    kernel_or_plugin: Some(row),
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
        let images = self.images.shown().await.map_err(database_failed)?;
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
                            missing_movie_to_proto(movie, open, &images).into()
                        }
                        RowTitle::MissingShow(show) => missing_show_to_proto(show, &images).into(),
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
        let images = self.images.shown().await.map_err(database_failed)?;
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

const MOVIE_POSTER: TitleImage = TitleImage::Movie(Role::Poster);
const SHOW_POSTER: TitleImage = TitleImage::Show(Role::Poster);

/// A Movie as a Library's grid shows it, with its Poster as `images` gives it and the caller's
/// Watch State of it, if they've started it.
fn movie_summary(
    movie: LibraryMovie,
    watch_state: Option<WatchState>,
    images: &ShownImages,
) -> proto::MovieSummary {
    let downloaded = movie.poster.as_ref().map(Downloaded::stored);
    let poster = images.of_title(&movie.id, &movie.ids, MOVIE_POSTER, downloaded);
    proto::MovieSummary {
        id: external_id_to_proto(&movie.id).into(),
        title: movie.title,
        release_date: movie.release_date,
        poster_url: poster.url,
        poster_thumbhash: poster.thumbhash,
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
    let downloaded = movie.poster.as_ref().map(Downloaded::stored);
    let poster = images.of_title(&movie.id, &movie.ids, MOVIE_POSTER, downloaded);
    proto::MovieInLibrary {
        library_id: movie.library,
        poster_url: poster.url,
        poster_thumbhash: poster.thumbhash,
        id: external_id_to_proto(&movie.id).into(),
        title: movie.title,
        release_date: movie.release_date,
        ..Default::default()
    }
}

fn found_show_to_proto(show: FoundShow, images: &ShownImages) -> proto::ShowInLibrary {
    let downloaded = show.poster.as_ref().map(Downloaded::stored);
    let poster = images.of_title(&show.id, &show.ids, SHOW_POSTER, downloaded);
    proto::ShowInLibrary {
        library_id: show.library,
        poster_url: poster.url,
        poster_thumbhash: poster.thumbhash,
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
    let shown = |role, image: Option<&ImageRef>| {
        let downloaded = image.map(Downloaded::stored);
        images.of_title(&movie.id, &movie.ids, TitleImage::Movie(role), downloaded)
    };
    let downloaded = |image: Option<&ImageRef>| images.downloaded(image.map(Downloaded::stored));
    let poster = shown(Role::Poster, movie.poster.as_ref());
    let backdrop = shown(Role::Backdrop, movie.backdrop.as_ref());
    let titled_backdrop = downloaded(movie.titled_backdrop.as_ref());
    proto::Movie {
        title_logo_url: downloaded(movie.logo.as_ref()).url,
        genres: movie.genres,
        content_rating: movie.content_rating,
        custom_poster: poster.custom,
        custom_backdrop: backdrop.custom,
        poster_url: poster.url,
        poster_thumbhash: poster.thumbhash,
        backdrop_url: backdrop.url,
        backdrop_thumbhash: backdrop.thumbhash,
        titled_backdrop_url: titled_backdrop.url,
        titled_backdrop_thumbhash: titled_backdrop.thumbhash,
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
    let shown = |role, image: Option<&ImageRef>| {
        let downloaded = image.map(Downloaded::stored);
        images.of_title(&show.id, &show.ids, TitleImage::Show(role), downloaded)
    };
    let downloaded = |image: Option<&ImageRef>| images.downloaded(image.map(Downloaded::stored));
    let poster = shown(Role::Poster, show.poster());
    let backdrop = shown(Role::Backdrop, show.backdrop.as_ref());
    let titled_backdrop = downloaded(show.titled_backdrop.as_ref());
    proto::Show {
        id: external_id_to_proto(&show.id).into(),
        title: show.title.clone(),
        custom_poster: poster.custom,
        custom_backdrop: backdrop.custom,
        poster_url: poster.url,
        poster_thumbhash: poster.thumbhash,
        backdrop_url: backdrop.url,
        backdrop_thumbhash: backdrop.thumbhash,
        titled_backdrop_url: titled_backdrop.url,
        titled_backdrop_thumbhash: titled_backdrop.thumbhash,
        title_logo_url: downloaded(show.logo.as_ref()).url,
        genres: show.genres.clone(),
        content_rating: show.content_rating.clone(),
        overview: show.overview.clone(),
        first_air_date: show.first_air_date.clone(),
        season_count: u32::try_from(season_count(show)).unwrap_or(u32::MAX),
        ..Default::default()
    }
}

/// An Episode as clients see it, with its Still as a URL on this Server and its thumbhash from
/// `images`, and the caller's Watch State of it, if they've started it.
fn episode_to_proto(
    episode: Episode,
    watch_state: Option<WatchState>,
    images: &ShownImages,
) -> proto::Episode {
    let plugin = &episode.plugin;
    let still = (episode.still.as_deref()).map(|path| Downloaded::Stored { plugin, path });
    let still = images.downloaded(still);
    proto::Episode {
        still_url: still.url,
        still_thumbhash: still.thumbhash,
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
#[path = "browse/browse_tests.rs"]
mod tests;
