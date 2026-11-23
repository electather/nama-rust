//! Continue Watching and Featured entries, the Episodes either side of one, a Home row's titles,
//! and a title's Roles and Trailers, as clients see them.

use connectrpc::ConnectError;
use nama_proto::nama::v1 as proto;

use crate::{
    api::{
        ImageSources, database_failed, external_id_to_proto,
        requests::{missing_movie_to_proto, missing_show_to_proto},
    },
    browse::{self, Episode, Featured, Started},
    home_rows::RowTitle,
    library::{OnServer, Roles, ShownImages, Trailer, WatchState},
};

use super::{
    Fields, episode_to_proto, found_movie_to_proto, found_show_to_proto, movie_to_proto, refused,
    show_to_proto,
};

/// The Episode `GetNextEpisode` or `GetPreviousEpisode` `found` beside the one asked about, with
/// its images as `images` shows them, or the Refusal for the request.
pub(super) async fn episode_beside_to_proto(
    found: Result<Option<(Episode, Option<WatchState>)>, browse::Error>,
    images: &ImageSources,
) -> Result<Option<proto::Episode>, ConnectError> {
    let found = found.map_err(refused(Fields {
        title: "show_id",
        season: "season_number",
        episode: "episode_number",
        ..Fields::LIBRARY
    }))?;
    let images = images.shown().await.map_err(database_failed)?;
    Ok(found.map(|(episode, state)| episode_to_proto(episode, state, &images)))
}

/// A Continue Watching entry as clients see it, with its images as `images` gives them, its Movie
/// or Show `on_watchlist` or not.
pub(super) fn started_to_proto(
    started: Started,
    on_watchlist: bool,
    images: &ShownImages,
) -> proto::ContinueWatchingEntry {
    let (library_id, movie_or_episode) = match started {
        Started::Movie {
            library,
            movie,
            state,
        } => {
            let movie = proto::Movie {
                on_watchlist,
                ..movie_to_proto(movie, Some(state), images)
            };
            (library, movie.into())
        }
        Started::Episode {
            library,
            show,
            episode,
            state,
        } => {
            let started_episode = proto::ContinueWatchingEpisode {
                episode: episode_to_proto(episode, state, images).into(),
                show: proto::Show {
                    on_watchlist,
                    ..show_to_proto(&show, images)
                }
                .into(),
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

/// A Featured title as clients see it, with its images as `images` gives them, its Movie or Show
/// `on_watchlist` or not.
pub(super) fn featured_to_proto(
    featured: Featured,
    on_watchlist: bool,
    images: &ShownImages,
) -> proto::FeaturedTitle {
    let (library_id, movie_or_show, next_episode) = match featured {
        Featured::Movie {
            library,
            movie,
            state,
        } => {
            let movie = proto::Movie {
                on_watchlist,
                ..movie_to_proto(movie, state, images)
            };
            (library, movie.into(), None)
        }
        Featured::Show {
            library,
            show,
            next,
        } => {
            let next = next.map(|(episode, state)| episode_to_proto(episode, state, images));
            let show = proto::Show {
                on_watchlist,
                ..show_to_proto(&show, images)
            };
            (library, show.into(), next)
        }
    };
    proto::FeaturedTitle {
        library_id,
        movie_or_show: Some(movie_or_show),
        next_episode: next_episode.into(),
        ..Default::default()
    }
}

/// A title's Cast and Crew as clients see them, each headshot at `headshot_url(its path)`.
pub(in crate::api) fn roles_to_proto(
    roles: Roles,
    headshot_url: impl Fn(&str) -> Option<String>,
) -> (Vec<proto::Role>, Vec<proto::Role>) {
    let [cast, crew] = [roles.cast, roles.crew].map(|roles| {
        (roles.into_iter())
            .map(|role| proto::Role {
                headshot_url: role.headshot.as_deref().and_then(&headshot_url),
                person_id: external_id_to_proto(&role.person).into(),
                name: role.name,
                part: role.part,
                ..Default::default()
            })
            .collect()
    });
    (cast, crew)
}

/// A title's Trailers, as clients see them.
pub(in crate::api) fn trailers_to_proto(trailers: Vec<Trailer>) -> Vec<proto::Trailer> {
    (trailers.into_iter())
        .map(|trailer| proto::Trailer {
            name: trailer.name,
            url: trailer.url,
            ..Default::default()
        })
        .collect()
}

/// A Home row's title, or a Box Set's Movie, as clients see it.
pub(super) fn row_title_to_proto(title: RowTitle, images: &ShownImages) -> proto::HomeRowTitle {
    proto::HomeRowTitle {
        movie_or_show: Some(match title {
            RowTitle::OnServer(OnServer::Movie(movie)) => {
                found_movie_to_proto(movie, images).into()
            }
            RowTitle::OnServer(OnServer::Show(show)) => found_show_to_proto(show, images).into(),
            RowTitle::MissingMovie(movie, open) => {
                missing_movie_to_proto(movie, open, images).into()
            }
            RowTitle::MissingShow(show) => missing_show_to_proto(show, images).into(),
        }),
        ..Default::default()
    }
}
