//! Watch State through Browse: what a User reports, marks and has started.

use std::collections::HashMap;

use nama_plugin_api::metadata::{ExternalId, ExternalIds};

use super::{Browse, Episode, Error, Movie, Show, Target, Titles, finding_failed};
use crate::library::{
    LibraryEpisode, MovieOrEpisode, Progress, TitlesIn, WatchState, WatchStateKind,
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
    /// `version`: through its Movie, or split across each Episode its file holds, under each one's
    /// preferred id. An Unmatched Item has no Watch State, so progress on it stores nothing (ADR
    /// 0013).
    pub(crate) async fn report(
        &self,
        user: i64,
        version: i64,
        progress: Progress,
    ) -> Result<(), Error> {
        let titles = match self.contents.titles_in(version).await? {
            TitlesIn::NoSuchVersion => return Err(Error::NoSuchVersion),
            TitlesIn::Movie(None) => return Ok(()),
            TitlesIn::Movie(Some((id, ids))) => {
                vec![Some(MovieOrEpisode::movie(&id).known_as(&ids))]
            }
            // An Item that isn't Matched keeps its share of the file, so the others' shares don't
            // move.
            TitlesIn::Episodes(episodes) => (episodes.into_iter())
                .map(|episode| episode.map(|(id, ids)| MovieOrEpisode::episode(&id).known_as(&ids)))
                .collect(),
        };
        Ok(self.watch_states.report(user, titles, progress).await?)
    }

    /// Marks, as of now, `target` of the Library `library` played or unplayed for the User
    /// `user`: a Season or Show is each of its Episodes. Played is written under each one's
    /// preferred id; unplayed forgets it under every id. Fails as browsing to it does if the
    /// Library has no such title.
    pub(crate) async fn mark(
        &self,
        user: i64,
        library: i64,
        target: Target,
        played: bool,
    ) -> Result<(), Error> {
        let titles = self.titles(&self.library(library).await?).await?;
        let episode =
            |episode: &LibraryEpisode| MovieOrEpisode::episode(&episode.id).known_as(&episode.ids);
        let marked = match target {
            Target::Movie(id) => {
                let movie = titles.movie(&id)?;
                vec![MovieOrEpisode::movie(&movie.id).known_as(&movie.ids)]
            }
            Target::Episode(id) => vec![episode(&titles.episode(&id)?)],
            Target::Season { show, number } => (titles.season(&show, number)?.episodes.iter())
                .map(episode)
                .collect(),
            Target::Show(id) => titles.show(&id)?.episodes().map(episode).collect(),
        };
        Ok(self.watch_states.mark(user, marked, played).await?)
    }

    /// The User `user`'s Continue Watching: the Movies and Episodes they've started and not
    /// played, most recent first. Each is in the oldest Library that has it under any of its ids,
    /// listed once with its most recently updated Watch State among them, so a newer played one
    /// under one id hides an older started one under another (ADR 0042). One no Library has is
    /// left out.
    pub(crate) async fn continue_watching(&self, user: i64) -> Result<Vec<Started>, Error> {
        let started = self.watch_states.continue_watching(user).await?;
        if started.is_empty() {
            return Ok(Vec::new());
        }
        let mut found = Found {
            unfound: started.iter().cloned().collect(),
            place: HashMap::new(),
            titles: Vec::new(),
        };
        // Oldest first, so a title in two Libraries is found in the older.
        for library in self.libraries.list().await.map_err(finding_failed)? {
            match self.read(&library).await? {
                Titles::Movies(movies) => found.movies(library.id, movies),
                Titles::Shows(shows) => found.episodes(library.id, &shows),
            }
        }
        let (asked, mut titles): (Vec<_>, Vec<_>) = (found.titles.into_iter())
            .map(|(asked, title)| (asked, Some(title)))
            .unzip();
        let newest = self.watch_states.of_titles(Some(user), &asked).await?;
        Ok(started
            .into_iter()
            .filter_map(|(watched, _)| {
                let place = *found.place.get(&watched)?;
                // Taken under the first of its ids listed, so a title is listed once.
                let title = titles.get_mut(place)?.take()?;
                let &(_, state) = newest.get(place)?.first()?;
                (state.position_ms > 0 && !state.played).then(|| title.with_state(state))
            })
            .collect())
    }
}

impl Started {
    /// The same title with the Watch State `state`.
    fn with_state(mut self, state: WatchState) -> Self {
        match &mut self {
            Self::Movie { state: had, .. } | Self::Episode { state: had, .. } => *had = state,
        }
        self
    }
}

/// The titles of the Libraries read so far that a User has started under any of their ids.
struct Found {
    /// The User's started Watch States under ids no Library read so far has.
    unfound: HashMap<MovieOrEpisode, WatchState>,
    /// Which of `titles` each id the User started a title under is of.
    place: HashMap<MovieOrEpisode, usize>,
    /// Each title found, with every id of it, and one of the Watch States it was started with.
    titles: Vec<((WatchStateKind, ExternalIds), Started)>,
}

impl Found {
    /// Adds `title(state)`, a title of kind `kind` with the ids `ids`, if the User started it
    /// under any of them, with `state` one they started it with, and takes its ids as its.
    fn add(
        &mut self,
        kind: WatchStateKind,
        ids: &ExternalIds,
        watched: impl Fn(&ExternalId) -> MovieOrEpisode,
        title: impl FnOnce(WatchState) -> Started,
    ) {
        let place = self.titles.len();
        let mut started = None;
        for id in ids.iter() {
            let watched = watched(&id);
            if let Some(state) = self.unfound.remove(&watched) {
                self.place.insert(watched, place);
                started = started.or(Some(state));
            }
        }
        if let Some(state) = started {
            self.titles.push(((kind, ids.clone()), title(state)));
        }
    }

    /// The `movies` of the Library `library`.
    fn movies(&mut self, library: i64, movies: Vec<Movie>) {
        for movie in movies {
            let ids = movie.ids.clone();
            let started = |state| Started::Movie {
                library,
                movie,
                state,
            };
            self.add(WatchStateKind::Movie, &ids, MovieOrEpisode::movie, started);
        }
    }

    /// The Episodes of `shows`, of the Library `library`.
    fn episodes(&mut self, library: i64, shows: &[Show]) {
        for show in shows {
            for episode in show.episodes() {
                // ponytail: a Show cloned per started Episode; `Arc<Show>` if a profile ever
                // points here.
                let started = |state| Started::Episode {
                    library,
                    show: show.clone(),
                    episode: episode.clone(),
                    state,
                };
                let kind = WatchStateKind::Episode;
                self.add(kind, &episode.ids, MovieOrEpisode::episode, started);
            }
        }
    }
}
