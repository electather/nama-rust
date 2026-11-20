//! Featured: the Movies and Shows at the top of a User's home page, picked by weight from the
//! Kernel's suggestions (ADR 0072).

use std::collections::HashSet;

use nama_plugin_api::metadata::ExternalId;

use super::{
    Browse, Episode, Error, Movie, Show, Started, Titles, episode_ids, finding_failed, movie_ids,
    watching::{WatchedEpisode, is_aired, show_entry},
    with_tracks,
};
use crate::{
    db,
    library::{LibraryEpisode, LibraryMovie, WatchState},
};

// ponytail: the Kernel's weights, and the 14 days they fade over, are fixed (ADR 0072); make them
// settings if Admins ask to tune Featured.
/// A Continue Watching entry's weight the moment its Watch State changes.
const CONTINUE_WATCHING: f64 = 0.8;
/// A title's weight the moment it is added.
const ADDED: f64 = 0.5;
/// How long a title counts as added, and how long a suggestion takes to fade to half its
/// weight, in seconds.
const FADES_OVER: i32 = 14 * 24 * 60 * 60;
/// How many titles Featured holds.
const FEATURED: usize = 5;

/// A Movie or Show in a User's Featured, in the Library it was suggested from.
#[derive(Debug)]
pub(crate) enum Featured {
    Movie {
        library: i64,
        movie: Movie,
        /// `None` if the User hasn't started it.
        state: Option<WatchState>,
    },
    Show {
        library: i64,
        show: Show,
        /// The Episode Play or Resume starts, with the User's Watch State of it: see
        /// [`plays_first`].
        next: Option<(Episode, Option<WatchState>)>,
    },
}

/// A Movie or Show suggested for Featured, without Tracks.
enum Title {
    Movie(LibraryMovie),
    Show(Show),
}

impl Title {
    /// What makes two suggestions the same title: a Movie and a Show may share an id.
    fn key(&self) -> (bool, ExternalId) {
        match self {
            Self::Movie(movie) => (false, movie.id.clone()),
            Self::Show(show) => (true, show.id.clone()),
        }
    }

    const fn added_at(&self) -> i64 {
        match self {
            Self::Movie(movie) => movie.added_at,
            Self::Show(show) => show.added_at,
        }
    }
}

/// A title suggested for Featured, from the Library `library`.
struct Suggestion {
    library: i64,
    title: Title,
    weight: f64,
}

impl Browse {
    /// The User `user`'s Featured, heaviest first, those of equal weight the most recently added
    /// first: their Continue Watching entries and the titles added in the last 14 days, each
    /// once, at the heavier of its weights and in the Library that suggestion came from.
    pub(crate) async fn featured(&self, user: i64) -> Result<Vec<Featured>, Error> {
        let mut featured = Vec::with_capacity(FEATURED);
        for suggestion in heaviest(self.suggestions(user).await?) {
            featured.push(self.with_what_plays(user, suggestion).await?);
        }
        Ok(featured)
    }

    /// The Kernel's suggestions for the User `user`, each weighed as of now: a title may be
    /// suggested more than once.
    async fn suggestions(&self, user: i64) -> Result<Vec<Suggestion>, Error> {
        let now = db::now();
        let mut suggested: Vec<Suggestion> = (self.all_started(user).await?.into_iter())
            .map(|(started, (changed_at, _))| {
                let (library, title) = match started {
                    Started::Movie { library, movie, .. } => (library, Title::Movie(movie)),
                    Started::Episode { library, show, .. } => (library, Title::Show(show)),
                };
                let weight = faded(CONTINUE_WATCHING, now - changed_at);
                Suggestion {
                    library,
                    title,
                    weight,
                }
            })
            .collect();
        for library in self.libraries.list().await.map_err(finding_failed)? {
            // ponytail: whole-Library read, a second one after Continue Watching's.
            let titles: Vec<Title> = match self.titles(&library).await? {
                Titles::Movies(movies) => movies.into_iter().map(Title::Movie).collect(),
                Titles::Shows(shows) => shows.into_iter().map(Title::Show).collect(),
            };
            let added = (titles.into_iter())
                .filter(|title| now - title.added_at() < i64::from(FADES_OVER))
                .map(|title| Suggestion {
                    library: library.id,
                    weight: faded(ADDED, now - title.added_at()),
                    title,
                });
            suggested.extend(added);
        }
        Ok(suggested)
    }

    /// `suggestion` with the User `user`'s Watch State of its Movie, or its Show's Episode that
    /// Play or Resume starts, and that Movie's or Episode's Tracks.
    async fn with_what_plays(&self, user: i64, suggestion: Suggestion) -> Result<Featured, Error> {
        let library = suggestion.library;
        Ok(match suggestion.title {
            Title::Movie(movie) => {
                let state = self.states_of(user, vec![movie_ids(&movie)]).await?.pop();
                let probes = self.probes(movie.versions.iter()).await?;
                Featured::Movie {
                    library,
                    movie: movie.map_versions(with_tracks(&probes)),
                    state: state.flatten(),
                }
            }
            Title::Show(show) => {
                let titles = show.episodes().map(episode_ids).collect();
                let states = self.states_of(user, titles).await?;
                // `show_entry` breaks ties by row id, which these states lack: of two started in
                // the same second, the later aired plays.
                let watched: Vec<WatchedEpisode<'_, _>> = (show.episodes().zip(states))
                    .map(|(episode, state)| (episode, state.map(|state| (state, 0))))
                    .collect();
                let next = match plays_first(&watched) {
                    Some((episode, state)) => {
                        let probes = self.probes(episode.versions.iter()).await?;
                        Some((episode.clone().map_versions(with_tracks(&probes)), state))
                    }
                    None => None,
                };
                Featured::Show {
                    library,
                    show,
                    next,
                }
            }
        })
    }
}

/// The [`FEATURED`] heaviest of `suggested`, heaviest first, those of equal weight the most
/// recently added first, each title once at the heaviest of its suggestions.
fn heaviest(mut suggested: Vec<Suggestion>) -> Vec<Suggestion> {
    suggested.sort_by(|a, b| {
        (b.weight.total_cmp(&a.weight)).then(b.title.added_at().cmp(&a.title.added_at()))
    });
    let mut seen = HashSet::new();
    suggested.retain(|suggestion| seen.insert(suggestion.title.key()));
    suggested.truncate(FEATURED);
    suggested
}

/// `weight`, halved for each [`FADES_OVER`] of `age` seconds.
fn faded(weight: f64, age: i64) -> f64 {
    let age = f64::from(i32::try_from(age.max(0)).unwrap_or(i32::MAX));
    weight * 0.5_f64.powf(age / f64::from(FADES_OVER))
}

/// The Episode of a Show that Play or Resume starts, from each of its Episodes in aired order
/// with the User's Watch State of it: its Continue Watching entry's, as `GetNextEpisode` picks
/// one, or else the first the User hasn't played, Specials aside. `None` if every one is played.
fn plays_first<'a, V>(
    watched: &[WatchedEpisode<'a, V>],
) -> Option<(&'a LibraryEpisode<V>, Option<WatchState>)> {
    show_entry(watched).or_else(|| {
        (watched.iter())
            .filter(|&&(episode, _)| is_aired(episode))
            .find(|&&(_, taken)| taken.is_none_or(|(state, _)| state.played_at.is_none()))
            .map(|&(episode, taken)| (episode, taken.map(|(state, _)| state)))
    })
}

#[cfg(test)]
#[path = "featured_tests.rs"]
mod tests;
