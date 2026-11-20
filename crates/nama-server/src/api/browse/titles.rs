//! Continue Watching and Featured entries as clients see them.

use nama_proto::nama::v1 as proto;

use crate::{
    browse::{Featured, Started},
    library::ShownImages,
};

use super::{episode_to_proto, movie_to_proto, show_to_proto};

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
