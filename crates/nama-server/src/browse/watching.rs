//! Watch State through Browse: what a User reports, marks and has started.

use std::collections::HashMap;

use super::{Browse, Episode, Error, Movie, Show, Target, Titles, finding_failed};
use crate::library::{
    contents::{self, MatchedTo},
    watch_states::{MovieOrEpisode, Progress, WatchState},
};

/// A Movie or Episode a User has started and not played, in the Library Continue Watching lists
/// it under.
#[derive(Debug)]
pub(crate) enum Started {
    Movie {
        library: i64,
        movie: Movie,
        state: WatchState,
    },
    Episode {
        library: i64,
        show: Show,
        episode: Episode,
        state: WatchState,
    },
}

impl Browse {
    /// Records, as of now, that the User `user` has got as far as `progress` through the Version
    /// `version`: through its Movie, or split across each Episode its file holds. An Unmatched
    /// Item has no Watch State, so progress on it stores nothing (ADR 0013).
    pub(crate) async fn report(
        &self,
        user: i64,
        version: i64,
        progress: Progress,
    ) -> Result<(), Error> {
        let titles = match self.contents.matched_to(version).await? {
            MatchedTo::NoSuchVersion => return Err(Error::NoSuchVersion),
            MatchedTo::Nothing => return Ok(()),
            MatchedTo::Movie(movie) => vec![Some(MovieOrEpisode::movie(&movie))],
            // An Item that isn't Matched keeps its share of the file, so the others' shares don't
            // move.
            MatchedTo::Episodes { episodes, .. } => (episodes.iter())
                .map(|episode| episode.as_ref().map(MovieOrEpisode::episode))
                .collect(),
        };
        Ok(self.watch_states.report(user, titles, progress).await?)
    }

    /// Marks, as of now, `target` of the Library `library` played or unplayed for the User
    /// `user`: a Season or Show is each of its Episodes. Fails as browsing to it does if the
    /// Library has no such title.
    pub(crate) async fn mark(
        &self,
        user: i64,
        library: i64,
        target: Target,
        played: bool,
    ) -> Result<(), Error> {
        let titles = self.titles(&self.library(library).await?).await?;
        let episode = |episode: &contents::Episode| MovieOrEpisode::episode(&episode.id);
        let marked = match target {
            Target::Movie(id) => vec![MovieOrEpisode::movie(&titles.movie(&id)?.id)],
            Target::Episode(id) => vec![episode(&titles.episode(&id)?)],
            Target::Season { show, number } => (titles.season(&show, number)?.episodes.iter())
                .map(episode)
                .collect(),
            Target::Show(id) => titles.show(&id)?.episodes().map(episode).collect(),
        };
        Ok(self.watch_states.mark(user, marked, played).await?)
    }

    /// The User `user`'s Continue Watching: the Movies and Episodes they've started and not
    /// played, most recent first. Each is in the oldest Library that has it, and one no Library
    /// has is left out.
    pub(crate) async fn continue_watching(&self, user: i64) -> Result<Vec<Started>, Error> {
        let started = self.watch_states.continue_watching(user).await?;
        if started.is_empty() {
            return Ok(Vec::new());
        }
        let mut unfound: HashMap<MovieOrEpisode, WatchState> = started.iter().cloned().collect();
        let mut found = HashMap::new();
        // Oldest first, so a title in two Libraries is found in the older.
        for library in self.libraries.list().await.map_err(finding_failed)? {
            found.extend(match self.read(&library).await? {
                Titles::Movies(movies) => started_movies(library.id, movies, &mut unfound),
                Titles::Shows(shows) => started_episodes(library.id, &shows, &mut unfound),
            });
        }
        Ok(started
            .into_iter()
            .filter_map(|(watched, _)| found.remove(&watched))
            .collect())
    }
}

/// The `movies` of the Library `library` that `unfound` has the Watch State of, each taken out
/// of `unfound`.
fn started_movies(
    library: i64,
    movies: Vec<Movie>,
    unfound: &mut HashMap<MovieOrEpisode, WatchState>,
) -> Vec<(MovieOrEpisode, Started)> {
    movies
        .into_iter()
        .filter_map(|movie| {
            let watched = MovieOrEpisode::movie(&movie.id);
            let state = unfound.remove(&watched)?;
            Some((
                watched,
                Started::Movie {
                    library,
                    movie,
                    state,
                },
            ))
        })
        .collect()
}

/// The Episodes of `shows`, of the Library `library`, that `unfound` has the Watch State of, each
/// taken out of `unfound`.
fn started_episodes(
    library: i64,
    shows: &[Show],
    unfound: &mut HashMap<MovieOrEpisode, WatchState>,
) -> Vec<(MovieOrEpisode, Started)> {
    let mut started = Vec::new();
    for show in shows {
        for episode in show.episodes() {
            let watched = MovieOrEpisode::episode(&episode.id);
            let Some(state) = unfound.remove(&watched) else {
                continue;
            };
            // ponytail: a Show cloned per started Episode; `Arc<Show>` if a profile ever
            // points here.
            let episode = Started::Episode {
                library,
                show: show.clone(),
                episode: episode.clone(),
                state,
            };
            started.push((watched, episode));
        }
    }
    started
}
